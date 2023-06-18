use crate::db::daily_leaders;
use crate::errors::response::Result;
use crate::models::daily_leaders::DailyLeaders;

use crate::database::CONNECTION;

use axum::{extract::Path, routing::get, Json, Router};

pub fn create_route() -> Router {
    Router::new().route("/daily_leaser/:date", get(get_daily_leaders_by_date))
}

/// get dailyLeaders document by _date
async fn get_daily_leaders_by_date(Path(_date): Path<String>) -> Result<Json<DailyLeaders>> {
    daily_leaders::find_daily_leaders(CONNECTION.get().await, _date)
        .await
        .map(Json)
}
