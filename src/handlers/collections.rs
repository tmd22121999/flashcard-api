use crate::{
    error::{AppError, Result},
    middleware::{AuthUser, MaybeUser},
    models::*,
    AppState,
};
use axum::{
    extract::{Path, State},
    routing::{delete, get, post, put},
    Json, Router,
};
use uuid::Uuid;

pub fn router() -> Router<AppState> {
    Router::new()
        // Guests can only read public collections
        .route("/collections",              get(list).post(create))
        .route("/collections/:id",          get(get_one).put(update).delete(delete_one))
        .route("/collections/:id/cards",    get(list_cards).post(add_card))
        .route("/collections/:id/cards/:card_id", delete(remove_card))
}

/// GET /collections — guests see public ones; authed users see their own + public
async fn list(
    State(state): State<AppState>,
    MaybeUser(user_id): MaybeUser,
) -> Result<Json<Vec<Collection>>> {
    let rows = match user_id {
        Some(uid) => sqlx::query_as::<_, Collection>(
            "SELECT * FROM collections WHERE user_id = $1 OR is_public = TRUE ORDER BY created_at DESC",
        )
        .bind(uid)
        .fetch_all(state.db.as_ref())
        .await?,
        None => sqlx::query_as::<_, Collection>(
            "SELECT * FROM collections WHERE is_public = TRUE ORDER BY created_at DESC",
        )
        .fetch_all(state.db.as_ref())
        .await?,
    };
    Ok(Json(rows))
}

/// GET /collections/:id
async fn get_one(
    State(state): State<AppState>,
    MaybeUser(user_id): MaybeUser,
    Path(id): Path<Uuid>,
) -> Result<Json<Collection>> {
    let col = sqlx::query_as::<_, Collection>("SELECT * FROM collections WHERE id = $1")
        .bind(id)
        .fetch_optional(state.db.as_ref())
        .await?
        .ok_or_else(|| AppError::NotFound("Collection not found".into()))?;

    // Access check: must be public or owned by caller
    if !col.is_public && col.user_id != user_id {
        return Err(AppError::Forbidden);
    }
    Ok(Json(col))
}

/// POST /collections
async fn create(
    State(state): State<AppState>,
    AuthUser(uid): AuthUser,
    Json(body): Json<CreateCollectionRequest>,
) -> Result<Json<Collection>> {
    let col = sqlx::query_as::<_, Collection>(
        r#"
        INSERT INTO collections (user_id, name, description, is_public)
        VALUES ($1, $2, $3, $4) RETURNING *
        "#,
    )
    .bind(uid)
    .bind(&body.name)
    .bind(&body.description)
    .bind(body.is_public.unwrap_or(false))
    .fetch_one(state.db.as_ref())
    .await?;
    Ok(Json(col))
}

/// PUT /collections/:id
async fn update(
    State(state): State<AppState>,
    AuthUser(uid): AuthUser,
    Path(id): Path<Uuid>,
    Json(body): Json<CreateCollectionRequest>,
) -> Result<Json<Collection>> {
    let col = sqlx::query_as::<_, Collection>(
        r#"
        UPDATE collections
        SET name=$1, description=$2, is_public=$3, updated_at=NOW()
        WHERE id=$4 AND user_id=$5
        RETURNING *
        "#,
    )
    .bind(&body.name)
    .bind(&body.description)
    .bind(body.is_public.unwrap_or(false))
    .bind(id)
    .bind(uid)
    .fetch_optional(state.db.as_ref())
    .await?
    .ok_or_else(|| AppError::NotFound("Collection not found or not yours".into()))?;
    Ok(Json(col))
}

/// DELETE /collections/:id
async fn delete_one(
    State(state): State<AppState>,
    AuthUser(uid): AuthUser,
    Path(id): Path<Uuid>,
) -> Result<Json<serde_json::Value>> {
    let rows = sqlx::query("DELETE FROM collections WHERE id=$1 AND user_id=$2")
        .bind(id)
        .bind(uid)
        .execute(state.db.as_ref())
        .await?
        .rows_affected();
    if rows == 0 {
        return Err(AppError::NotFound("Collection not found or not yours".into()));
    }
    Ok(Json(serde_json::json!({ "ok": true })))
}

/// GET /collections/:id/cards
async fn list_cards(
    State(state): State<AppState>,
    MaybeUser(user_id): MaybeUser,
    Path(id): Path<Uuid>,
) -> Result<Json<Vec<FlashcardView>>> {
    // Access check
    let col = sqlx::query_as::<_, Collection>("SELECT * FROM collections WHERE id=$1")
        .bind(id)
        .fetch_optional(state.db.as_ref())
        .await?
        .ok_or_else(|| AppError::NotFound("Collection not found".into()))?;

    if !col.is_public && col.user_id != user_id {
        return Err(AppError::Forbidden);
    }

    let rows = sqlx::query_as::<_, FlashcardView>(
        r#"
        SELECT
            f.id, f.word,
            COALESCE(o.front_text, f.front_text) AS front_text,
            COALESCE(o.back_text,  f.back_text)  AS back_text,
            f.lang,
            COALESCE(o.level, f.level)            AS level,
            o.star, o.learned, o.encounter_count,
            f.created_at, f.updated_at
        FROM collection_cards cc
        JOIN flashcards f ON f.id = cc.flashcard_id
        LEFT JOIN user_card_overrides o ON o.flashcard_id = f.id AND o.user_id = $2
        WHERE cc.collection_id = $1
        ORDER BY cc.sort_order, cc.added_at
        "#,
    )
    .bind(id)
    .bind(user_id)
    .fetch_all(state.db.as_ref())
    .await?;

    Ok(Json(rows))
}

/// POST /collections/:id/cards
async fn add_card(
    State(state): State<AppState>,
    AuthUser(uid): AuthUser,
    Path(id): Path<Uuid>,
    Json(body): Json<AddCardToCollectionRequest>,
) -> Result<Json<serde_json::Value>> {
    // Must own collection
    let owned: bool = sqlx::query_scalar(
        "SELECT EXISTS(SELECT 1 FROM collections WHERE id=$1 AND user_id=$2)",
    )
    .bind(id)
    .bind(uid)
    .fetch_one(state.db.as_ref())
    .await?;

    if !owned {
        return Err(AppError::Forbidden);
    }

    sqlx::query(
        r#"
        INSERT INTO collection_cards (collection_id, flashcard_id, sort_order)
        VALUES ($1, $2, $3)
        ON CONFLICT DO NOTHING
        "#,
    )
    .bind(id)
    .bind(body.flashcard_id)
    .bind(body.sort_order.unwrap_or(0))
    .execute(state.db.as_ref())
    .await?;

    Ok(Json(serde_json::json!({ "ok": true })))
}

/// DELETE /collections/:id/cards/:card_id
async fn remove_card(
    State(state): State<AppState>,
    AuthUser(uid): AuthUser,
    Path((col_id, card_id)): Path<(Uuid, Uuid)>,
) -> Result<Json<serde_json::Value>> {
    let owned: bool = sqlx::query_scalar(
        "SELECT EXISTS(SELECT 1 FROM collections WHERE id=$1 AND user_id=$2)",
    )
    .bind(col_id)
    .bind(uid)
    .fetch_one(state.db.as_ref())
    .await?;

    if !owned {
        return Err(AppError::Forbidden);
    }

    sqlx::query("DELETE FROM collection_cards WHERE collection_id=$1 AND flashcard_id=$2")
        .bind(col_id)
        .bind(card_id)
        .execute(state.db.as_ref())
        .await?;

    Ok(Json(serde_json::json!({ "ok": true })))
}
