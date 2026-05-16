use crate::{
    error::{AppError, Result},
    middleware::make_token,
    models::*,
    AppState,
};
use axum::{extract::State, routing::post, Json, Router};
use bcrypt::{hash, verify, DEFAULT_COST};
use uuid::Uuid;

pub fn router() -> Router<AppState> {
    Router::new()
        .route("/auth/register", post(register))
        .route("/auth/login",    post(login))
}

async fn register(
    State(state): State<AppState>,
    Json(body): Json<RegisterRequest>,
) -> Result<Json<AuthResponse>> {
    if body.username.trim().is_empty() || body.password.len() < 6 {
        return Err(AppError::BadRequest("Username required, password min 6 chars".into()));
    }

    let hashed = hash(&body.password, DEFAULT_COST)?;

    let user = sqlx::query_as::<_, User>(
        "INSERT INTO users (username, email, password_hash) VALUES ($1, $2, $3) RETURNING *",
    )
    .bind(&body.username)
    .bind(&body.email)
    .bind(&hashed)
    .fetch_one(state.db.as_ref())
    .await
    .map_err(|e| {
        if e.to_string().contains("unique") {
            AppError::Conflict("Email or username already exists".into())
        } else {
            AppError::Sqlx(e)
        }
    })?;

    let token = make_token(user.id, &user.username, &state.jwt_secret)?;
    Ok(Json(AuthResponse { token, user: user.into() }))
}

async fn login(
    State(state): State<AppState>,
    Json(body): Json<LoginRequest>,
) -> Result<Json<AuthResponse>> {
    let user = sqlx::query_as::<_, User>("SELECT * FROM users WHERE email = $1")
        .bind(&body.email)
        .fetch_optional(state.db.as_ref())
        .await?
        .ok_or(AppError::Unauthorized)?;

    if !verify(&body.password, &user.password_hash)? {
        return Err(AppError::Unauthorized);
    }

    let token = make_token(user.id, &user.username, &state.jwt_secret)?;
    Ok(Json(AuthResponse { token, user: user.into() }))
}
