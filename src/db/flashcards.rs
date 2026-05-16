use crate::{error::Result, models::*};
use sqlx::PgPool;
use uuid::Uuid;

/// Fetch paginated flashcards with per-user overrides merged in.
/// For guests (user_id = None) canonical fields are returned and
/// star/learned/encounter_count are NULL.
pub async fn list_flashcards(
    db: &PgPool,
    user_id: Option<Uuid>,
    q: &FlashcardQuery,
) -> Result<(Vec<FlashcardView>, i64)> {
    let page = q.page.unwrap_or(1).max(1);
    let per_page = q.per_page.unwrap_or(20).clamp(1, 100);
    let offset = (page - 1) * per_page;

    // Build dynamic query
    let rows = sqlx::query_as::<_, FlashcardView>(
        r#"
        SELECT
            f.id,
            f.word,
            COALESCE(o.front_text, f.front_text)  AS front_text,
            COALESCE(o.back_text,  f.back_text)   AS back_text,
            f.lang,
            COALESCE(o.level, f.level)             AS level,
            o.star,
            o.learned,
            o.encounter_count,
            f.created_at,
            f.updated_at
        FROM flashcards f
        LEFT JOIN user_card_overrides o
            ON o.flashcard_id = f.id AND o.user_id = $1
        LEFT JOIN flashcard_tags ft
            ON ft.flashcard_id = f.id AND ft.user_id = $1
        LEFT JOIN tags t
            ON t.id = ft.tag_id
        WHERE ($2::TEXT IS NULL OR f.lang  = $2)
          AND ($3::TEXT IS NULL OR COALESCE(o.level, f.level) = $3)
          AND ($4::TEXT IS NULL OR f.word ILIKE '%' || $4 || '%')
          AND ($5::TEXT IS NULL OR t.name = $5)
          AND ($6::BOOL IS NULL OR o.star = $6)
          AND ($7::BOOL IS NULL OR o.learned = $7)
        GROUP BY f.id, o.front_text, o.back_text, o.level,
                 o.star, o.learned, o.encounter_count
        ORDER BY f.word
        LIMIT $8 OFFSET $9
        "#,
    )
    .bind(user_id)
    .bind(&q.lang)
    .bind(&q.level)
    .bind(&q.word)
    .bind(&q.tag)
    .bind(q.starred)
    .bind(q.learned)
    .bind(per_page)
    .bind(offset)
    .fetch_all(db)
    .await?;

    let total: i64 = sqlx::query_scalar(
        r#"
        SELECT COUNT(DISTINCT f.id)
        FROM flashcards f
        LEFT JOIN user_card_overrides o ON o.flashcard_id = f.id AND o.user_id = $1
        LEFT JOIN flashcard_tags ft ON ft.flashcard_id = f.id AND ft.user_id = $1
        LEFT JOIN tags t ON t.id = ft.tag_id
        WHERE ($2::TEXT IS NULL OR f.lang  = $2)
          AND ($3::TEXT IS NULL OR COALESCE(o.level, f.level) = $3)
          AND ($4::TEXT IS NULL OR f.word ILIKE '%' || $4 || '%')
          AND ($5::TEXT IS NULL OR t.name = $5)
          AND ($6::BOOL IS NULL OR o.star = $6)
          AND ($7::BOOL IS NULL OR o.learned = $7)
        "#,
    )
    .bind(user_id)
    .bind(&q.lang)
    .bind(&q.level)
    .bind(&q.word)
    .bind(&q.tag)
    .bind(q.starred)
    .bind(q.learned)
    .fetch_one(db)
    .await?;

    Ok((rows, total))
}

pub async fn get_flashcard(
    db: &PgPool,
    card_id: Uuid,
    user_id: Option<Uuid>,
) -> Result<FlashcardView> {
    sqlx::query_as::<_, FlashcardView>(
        r#"
        SELECT
            f.id, f.word,
            COALESCE(o.front_text, f.front_text) AS front_text,
            COALESCE(o.back_text,  f.back_text)  AS back_text,
            f.lang,
            COALESCE(o.level, f.level)            AS level,
            o.star, o.learned, o.encounter_count,
            f.created_at, f.updated_at
        FROM flashcards f
        LEFT JOIN user_card_overrides o
            ON o.flashcard_id = f.id AND o.user_id = $2
        WHERE f.id = $1
        "#,
    )
    .bind(card_id)
    .bind(user_id)
    .fetch_optional(db)
    .await?
    .ok_or_else(|| crate::error::AppError::NotFound("Flashcard not found".into()))
}

/// Increment encounter_count for a user (upsert override row)
pub async fn increment_encounter(db: &PgPool, card_id: Uuid, user_id: Uuid) -> Result<()> {
    sqlx::query(
        r#"
        INSERT INTO user_card_overrides (user_id, flashcard_id, encounter_count)
        VALUES ($1, $2, 1)
        ON CONFLICT (user_id, flashcard_id)
        DO UPDATE SET
            encounter_count = user_card_overrides.encounter_count + 1,
            updated_at = NOW()
        "#,
    )
    .bind(user_id)
    .bind(card_id)
    .execute(db)
    .await?;
    Ok(())
}
