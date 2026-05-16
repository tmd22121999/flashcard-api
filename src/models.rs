use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use uuid::Uuid;

// ── User ────────────────────────────────────────────────────────────────────

#[derive(Debug, Serialize, Deserialize, sqlx::FromRow)]
pub struct User {
    pub id: Uuid,
    pub username: String,
    pub email: String,
    #[serde(skip_serializing)]
    pub password_hash: String,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
}

#[derive(Debug, Deserialize)]
pub struct RegisterRequest {
    pub username: String,
    pub email: String,
    pub password: String,
}

#[derive(Debug, Deserialize)]
pub struct LoginRequest {
    pub email: String,
    pub password: String,
}

#[derive(Debug, Serialize)]
pub struct AuthResponse {
    pub token: String,
    pub user: UserPublic,
}

#[derive(Debug, Serialize)]
pub struct UserPublic {
    pub id: Uuid,
    pub username: String,
    pub email: String,
}

impl From<User> for UserPublic {
    fn from(u: User) -> Self {
        Self { id: u.id, username: u.username, email: u.email }
    }
}

// ── Flashcard ────────────────────────────────────────────────────────────────

/// Full card view — canonical fields merged with per-user overrides (if logged in)
#[derive(Debug, Serialize, sqlx::FromRow)]
pub struct FlashcardView {
    pub id: Uuid,
    pub word: String,
    pub front_text: String,     // override or canonical
    pub back_text: String,      // override or canonical
    pub lang: String,
    pub level: Option<String>,  // override or canonical
    // per-user fields (null for guests)
    pub star: Option<bool>,
    pub learned: Option<bool>,
    pub encounter_count: Option<i32>,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
}

#[derive(Debug, Deserialize)]
pub struct CreateFlashcardRequest {
    pub word: String,
    pub front_text: String,
    pub back_text: String,
    pub lang: Option<String>,
    pub level: Option<String>,
}

#[derive(Debug, Deserialize)]
pub struct UpdateOverrideRequest {
    pub front_text: Option<String>,
    pub back_text: Option<String>,
    pub level: Option<String>,
    pub star: Option<bool>,
    pub learned: Option<bool>,
}

#[derive(Debug, Deserialize)]
pub struct FlashcardQuery {
    pub lang: Option<String>,
    pub level: Option<String>,
    pub word: Option<String>,   // search
    pub tag: Option<String>,    // filter by tag name
    pub starred: Option<bool>,
    pub learned: Option<bool>,
    pub page: Option<i64>,
    pub per_page: Option<i64>,
}

// ── Collection ────────────────────────────────────────────────────────────────

#[derive(Debug, Serialize, sqlx::FromRow)]
pub struct Collection {
    pub id: Uuid,
    pub user_id: Option<Uuid>,
    pub name: String,
    pub description: Option<String>,
    pub is_public: bool,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
}

#[derive(Debug, Deserialize)]
pub struct CreateCollectionRequest {
    pub name: String,
    pub description: Option<String>,
    pub is_public: Option<bool>,
}

#[derive(Debug, Deserialize)]
pub struct AddCardToCollectionRequest {
    pub flashcard_id: Uuid,
    pub sort_order: Option<i32>,
}

// ── Comment ────────────────────────────────────────────────────────────────

#[derive(Debug, Serialize, sqlx::FromRow)]
pub struct Comment {
    pub id: Uuid,
    pub flashcard_id: Uuid,
    pub user_id: Uuid,
    pub word: Option<String>,
    pub body: String,
    pub is_public: bool,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
}

#[derive(Debug, Deserialize)]
pub struct CreateCommentRequest {
    pub body: String,
    pub is_public: Option<bool>,
}

#[derive(Debug, Deserialize)]
pub struct UpdateCommentRequest {
    pub body: Option<String>,
    pub is_public: Option<bool>,
}

// ── Tag ────────────────────────────────────────────────────────────────────

#[derive(Debug, Serialize, sqlx::FromRow)]
pub struct Tag {
    pub id: Uuid,
    pub name: String,
    pub user_id: Option<Uuid>,
    pub created_at: DateTime<Utc>,
}

#[derive(Debug, Deserialize)]
pub struct CreateTagRequest {
    pub name: String,
}

// ── Level ────────────────────────────────────────────────────────────────────

#[derive(Debug, Serialize, sqlx::FromRow)]
pub struct Level {
    pub id: i32,
    pub code: String,
    pub lang: String,
    pub description: Option<String>,
    pub sort_order: i32,
}

// ── JWT Claims ────────────────────────────────────────────────────────────────

#[derive(Debug, Serialize, Deserialize)]
pub struct Claims {
    pub sub: Uuid,      // user id
    pub username: String,
    pub exp: usize,
}

// ── Pagination ────────────────────────────────────────────────────────────────

#[derive(Debug, Serialize)]
pub struct Page<T: Serialize> {
    pub data: Vec<T>,
    pub total: i64,
    pub page: i64,
    pub per_page: i64,
}
