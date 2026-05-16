use crate::{
    error::{AppError, Result},
    middleware::AuthUser,
    models::*,
    AppState,
};
use axum::{
    extract::{Path, State},
    routing::{delete, get, post},
    Json, Router,
};
use uuid::Uuid;

pub fn router() -> Router<AppState> {
    Router::new()
        // List all tags visible to the user (their own + system tags)
        .route("/tags",                             get(list_tags))
        // Tag a flashcard
        .route("/flashcards/:id/tags",              get(list_card_tags).post(add_tag))
        .route("/flashcards/:id/tags/:tag_id",      delete(remove_tag))
}

/// GET /tags  — returns user's tags + system tags
async fn list_tags(
    State(state): State<AppState>,
    AuthUser(uid): AuthUser,
) -> Result<Json<Vec<Tag>>> {
    let rows = sqlx::query_as::<_, Tag>(
        "SELECT * FROM tags WHERE user_id = $1 OR user_id IS NULL ORDER BY name",
    )
    .bind(uid)
    .fetch_all(state.db.as_ref())
    .await?;
    Ok(Json(rows))
}

/// GET /flashcards/:id/tags  — tags this user added to the card
async fn list_card_tags(
    State(state): State<AppState>,
    AuthUser(uid): AuthUser,
    Path(card_id): Path<Uuid>,
) -> Result<Json<Vec<Tag>>> {
    let rows = sqlx::query_as::<_, Tag>(
        r#"
        SELECT t.* FROM tags t
        JOIN flashcard_tags ft ON ft.tag_id = t.id
        WHERE ft.flashcard_id = $1 AND ft.user_id = $2
        ORDER BY t.name
        "#,
    )
    .bind(card_id)
    .bind(uid)
    .fetch_all(state.db.as_ref())
    .await?;
    Ok(Json(rows))
}

/// POST /flashcards/:id/tags  — create tag if needed, then attach to card
async fn add_tag(
    State(state): State<AppState>,
    AuthUser(uid): AuthUser,
    Path(card_id): Path<Uuid>,
    Json(body): Json<CreateTagRequest>,
) -> Result<Json<Tag>> {
    let name = body.name.trim().to_lowercase();
    if name.is_empty() {
        return Err(AppError::BadRequest("Tag name cannot be empty".into()));
    }

    // Upsert tag for this user
    let tag = sqlx::query_as::<_, Tag>(
        r#"
        INSERT INTO tags (name, user_id)
        VALUES ($1, $2)
        ON CONFLICT (name, user_id) DO UPDATE SET name = EXCLUDED.name
        RETURNING *
        "#,
    )
    .bind(&name)
    .bind(uid)
    .fetch_one(state.db.as_ref())
    .await?;

    // Get word for denormalized field
    let word: Option<String> =
        sqlx::query_scalar("SELECT word FROM flashcards WHERE id=$1")
            .bind(card_id)
            .fetch_optional(state.db.as_ref())
            .await?;

    // Attach tag to card
    sqlx::query(
        r#"
        INSERT INTO flashcard_tags (flashcard_id, tag_id, user_id, word)
        VALUES ($1, $2, $3, $4)
        ON CONFLICT DO NOTHING
        "#,
    )
    .bind(card_id)
    .bind(tag.id)
    .bind(uid)
    .bind(word)
    .execute(state.db.as_ref())
    .await?;

    Ok(Json(tag))
}

/// DELETE /flashcards/:id/tags/:tag_id
async fn remove_tag(
    State(state): State<AppState>,
    AuthUser(uid): AuthUser,
    Path((card_id, tag_id)): Path<(Uuid, Uuid)>,
) -> Result<Json<serde_json::Value>> {
    sqlx::query(
        "DELETE FROM flashcard_tags WHERE flashcard_id=$1 AND tag_id=$2 AND user_id=$3",
    )
    .bind(card_id)
    .bind(tag_id)
    .bind(uid)
    .execute(state.db.as_ref())
    .await?;

    Ok(Json(serde_json::json!({ "ok": true })))
}
