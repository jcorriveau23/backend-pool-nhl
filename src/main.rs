#[macro_use]
extern crate rocket;

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
            routes![
                routes::daily_leaders::get_daily_leaders_by_date,
                routes::user::get_user_by_name,
                routes::user::get_users,
                routes::auth::register_user,
                routes::auth::login_user,
                routes::auth::wallet_login_user,
                routes::auth::validate_token
            ],
        )
}