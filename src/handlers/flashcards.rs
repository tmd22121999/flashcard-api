use crate::{
    db::flashcards as db,
    error::{AppError, Result},
    middleware::{AuthUser, MaybeUser},
    models::*,
    AppState,
};
use axum::{
    extract::{Path, Query, State},
    routing::{get, patch, post},
    Json, Router,
};
use uuid::Uuid;

pub fn router() -> Router<AppState> {
    Router::new()
        // Public (guests can read)
        .route("/flashcards",          get(list).post(create))
        .route("/flashcards/:id",      get(get_one))
        // Authenticated only
        .route("/flashcards/:id/encounter", post(encounter))
        .route("/flashcards/:id/override",  patch(update_override))
}

/// GET /flashcards?lang=zh&level=HSK1&word=你好&tag=grammar&starred=true&page=1&per_page=20
async fn list(
    State(state): State<AppState>,
    MaybeUser(user_id): MaybeUser,
    Query(q): Query<FlashcardQuery>,
) -> Result<Json<Page<FlashcardView>>> {
    let (data, total) = db::list_flashcards(&state.db, user_id, &q).await?;
    let page = q.page.unwrap_or(1);
    let per_page = q.per_page.unwrap_or(20);
    Ok(Json(Page { data, total, page, per_page }))
}

/// GET /flashcards/:id
async fn get_one(
    State(state): State<AppState>,
    MaybeUser(user_id): MaybeUser,
    Path(id): Path<Uuid>,
) -> Result<Json<FlashcardView>> {
    let card = db::get_flashcard(&state.db, id, user_id).await?;
    Ok(Json(card))
}

/// POST /flashcards  (auth required — admin use; in prod add role check)
async fn create(
    State(state): State<AppState>,
    AuthUser(_uid): AuthUser,
    Json(body): Json<CreateFlashcardRequest>,
) -> Result<Json<FlashcardView>> {
    let row = sqlx::query_as::<_, FlashcardView>(
        r#"
        INSERT INTO flashcards (word, front_text, back_text, lang, level)
        VALUES ($1, $2, $3, $4, $5)
        RETURNING
            id, word, front_text, back_text, lang, level,
            NULL::BOOL AS star,
            NULL::BOOL AS learned,
            NULL::INT  AS encounter_count,
            created_at, updated_at
        "#,
    )
    .bind(&body.word)
    .bind(&body.front_text)
    .bind(&body.back_text)
    .bind(body.lang.as_deref().unwrap_or("zh"))
    .bind(&body.level)
    .fetch_one(state.db.as_ref())
    .await?;

    Ok(Json(row))
}

/// POST /flashcards/:id/encounter  — increment encounter_count
async fn encounter(
    State(state): State<AppState>,
    AuthUser(uid): AuthUser,
    Path(id): Path<Uuid>,
) -> Result<Json<serde_json::Value>> {
    db::increment_encounter(&state.db, id, uid).await?;
    Ok(Json(serde_json::json!({ "ok": true })))
}

/// PATCH /flashcards/:id/override  — user overrides front/back/level/star/learned
async fn update_override(
    State(state): State<AppState>,
    AuthUser(uid): AuthUser,
    Path(id): Path<Uuid>,
) -> Result<Json<serde_json::Value>> {
    // verify card exists
    sqlx::query_scalar::<_, bool>("SELECT EXISTS(SELECT 1 FROM flashcards WHERE id=$1)")
        .bind(id)
        .fetch_one(state.db.as_ref())
        .await?
        .then_some(())
        .ok_or_else(|| AppError::NotFound("Flashcard not found".into()))?;

    sqlx::query(
        r#"
        INSERT INTO user_card_overrides (user_id, flashcard_id)
        VALUES ($1, $2)
        ON CONFLICT (user_id, flashcard_id) DO NOTHING
        "#,
    )
    .bind(uid)
    .bind(id)
    .execute(state.db.as_ref())
    .await?;

    Ok(Json(serde_json::json!({ "ok": true })))
}

/// PATCH /flashcards/:id/override  — proper version with body
pub async fn patch_override(
    State(state): State<AppState>,
    AuthUser(uid): AuthUser,
    Path(card_id): Path<Uuid>,
    Json(body): Json<UpdateOverrideRequest>,
) -> Result<Json<serde_json::Value>> {
    sqlx::query(
        r#"
        INSERT INTO user_card_overrides
            (user_id, flashcard_id, front_text, back_text, level, star, learned)
        VALUES ($1, $2, $3, $4, $5, $6, $7)
        ON CONFLICT (user_id, flashcard_id) DO UPDATE SET
            front_text      = COALESCE(EXCLUDED.front_text, user_card_overrides.front_text),
            back_text       = COALESCE(EXCLUDED.back_text,  user_card_overrides.back_text),
            level           = COALESCE(EXCLUDED.level,      user_card_overrides.level),
            star            = COALESCE(EXCLUDED.star,       user_card_overrides.star),
            learned         = COALESCE(EXCLUDED.learned,    user_card_overrides.learned),
            updated_at      = NOW()
        "#,
    )
    .bind(uid)
    .bind(card_id)
    .bind(&body.front_text)
    .bind(&body.back_text)
    .bind(&body.level)
    .bind(body.star)
    .bind(body.learned)
    .execute(state.db.as_ref())
    .await?;

    Ok(Json(serde_json::json!({ "ok": true })))
}
