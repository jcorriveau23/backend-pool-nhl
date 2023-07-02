use crate::db::daily_leaders;
use crate::errors::response::Result;
use crate::models::daily_leaders::DailyLeaders;

use crate::AppState;
use axum::{
    extract::{Path, State},
    routing::get,
    Json, Router,
};

pub fn create_route() -> Router<AppState> {
    Router::new().route("/daily_leaser/:date", get(get_daily_leaders_by_date))
}

/// get dailyLeaders document by _date
async fn get_daily_leaders_by_date(
    state: State<AppState>,
    Path(_date): Path<String>,
) -> Result<Json<DailyLeaders>> {
    daily_leaders::find_daily_leaders(&state.db, _date)
        .await
        .map(Json)
}
