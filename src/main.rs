#[macro_use]
extern crate rocket;

use rocket_okapi::openapi_get_routes;
use rocket_okapi::swagger_ui::{make_swagger_ui, SwaggerUIConfig};

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
                routes::dayly_leaders::get_dayly_leaders_by_date,
            ]
        )
}