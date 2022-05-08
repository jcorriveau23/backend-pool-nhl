#[macro_use]
extern crate rocket;

use rocket_okapi::openapi_get_routes;

mod db;
mod models;
mod routes;
mod errors;


#[launch]
async fn rocket() -> _ {
    rocket::build()
        .attach(db::init())
        .mount(
            "/api-rust",
            openapi_get_routes![
                routes::daily_leaders::get_daily_leaders_by_date,
                routes::user::get_user_by_name,
                routes::user::get_users,
            ]
        )
}