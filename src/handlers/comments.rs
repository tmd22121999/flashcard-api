use crate::{
    error::{AppError, Result},
    middleware::{AuthUser, MaybeUser},
    models::*,
    AppState,
};
use axum::{
    extract::{Path, State},
    routing::{delete, get, patch, post},
    Json, Router,
};
use uuid::Uuid;

pub fn router() -> Router<AppState> {
    Router::new()
        .route("/flashcards/:id/comments",      get(list).post(create))
        .route("/flashcards/:id/comments/:cid", patch(update).delete(delete_one))
}

/// GET /flashcards/:id/comments
/// Guests see only is_public=true; authed users see public + their own private
async fn list(
    State(state): State<AppState>,
    MaybeUser(user_id): MaybeUser,
    Path(card_id): Path<Uuid>,
) -> Result<Json<Vec<Comment>>> {
    let rows = match user_id {
        Some(uid) => sqlx::query_as::<_, Comment>(
            r#"SELECT * FROM comments
               WHERE flashcard_id = $1
                 AND (is_public = TRUE OR user_id = $2)
               ORDER BY created_at DESC"#,
        )
        .bind(card_id)
        .bind(uid)
        .fetch_all(state.db.as_ref())
        .await?,
        None => sqlx::query_as::<_, Comment>(
            "SELECT * FROM comments WHERE flashcard_id=$1 AND is_public=TRUE ORDER BY created_at DESC",
        )
        .bind(card_id)
        .fetch_all(state.db.as_ref())
        .await?,
    };
    Ok(Json(rows))
}

/// POST /flashcards/:id/comments
async fn create(
    State(state): State<AppState>,
    AuthUser(uid): AuthUser,
    Path(card_id): Path<Uuid>,
    Json(body): Json<CreateCommentRequest>,
) -> Result<Json<Comment>> {
    if body.body.trim().is_empty() {
        return Err(AppError::BadRequest("Comment body cannot be empty".into()));
    }

    // Get word for denormalized search field
    let word: Option<String> =
        sqlx::query_scalar("SELECT word FROM flashcards WHERE id=$1")
            .bind(card_id)
            .fetch_optional(state.db.as_ref())
            .await?;

    let comment = sqlx::query_as::<_, Comment>(
        r#"
        INSERT INTO comments (flashcard_id, user_id, word, body, is_public)
        VALUES ($1, $2, $3, $4, $5)
        RETURNING *
        "#,
    )
    .bind(card_id)
    .bind(uid)
    .bind(word)
    .bind(&body.body)
    .bind(body.is_public.unwrap_or(false))
    .fetch_one(state.db.as_ref())
    .await?;

    Ok(Json(comment))
}

/// PATCH /flashcards/:id/comments/:cid
async fn update(
    State(state): State<AppState>,
    AuthUser(uid): AuthUser,
    Path((_, cid)): Path<(Uuid, Uuid)>,
    Json(body): Json<UpdateCommentRequest>,
) -> Result<Json<Comment>> {
    let comment = sqlx::query_as::<_, Comment>(
        r#"
        UPDATE comments
        SET
            body      = COALESCE($1, body),
            is_public = COALESCE($2, is_public),
            updated_at = NOW()
        WHERE id=$3 AND user_id=$4
        RETURNING *
        "#,
    )
    .bind(&body.body)
    .bind(body.is_public)
    .bind(cid)
    .bind(uid)
    .fetch_optional(state.db.as_ref())
    .await?
    .ok_or_else(|| AppError::NotFound("Comment not found or not yours".into()))?;

    Ok(Json(comment))
}

/// DELETE /flashcards/:id/comments/:cid
async fn delete_one(
    State(state): State<AppState>,
    AuthUser(uid): AuthUser,
    Path((_, cid)): Path<(Uuid, Uuid)>,
) -> Result<Json<serde_json::Value>> {
    let rows = sqlx::query("DELETE FROM comments WHERE id=$1 AND user_id=$2")
        .bind(cid)
        .bind(uid)
        .execute(state.db.as_ref())
        .await?
        .rows_affected();

    if rows == 0 {
        return Err(AppError::NotFound("Comment not found or not yours".into()));
    }

    Ok(Json(serde_json::json!({ "ok": true })))
}
