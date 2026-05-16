use crate::{error::Result, models::Level, AppState};
use axum::{extract::{Query, State}, routing::get, Json, Router};
use serde::Deserialize;

pub fn router() -> Router<AppState> {
    Router::new().route("/levels", get(list))
}

#[derive(Deserialize)]
struct LevelQuery {
    lang: Option<String>,
}

async fn list(
    State(state): State<AppState>,
    Query(q): Query<LevelQuery>,
) -> Result<Json<Vec<Level>>> {
    let rows = sqlx::query_as::<_, Level>(
        "SELECT * FROM levels WHERE ($1::TEXT IS NULL OR lang=$1) ORDER BY lang, sort_order",
    )
    .bind(&q.lang)
    .fetch_all(state.db.as_ref())
    .await?;
    Ok(Json(rows))
}
