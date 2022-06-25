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
                routes::auth::set_username,
                routes::auth::validate_token,
                routes::pool::get_pool_by_name,
                routes::pool::get_pools,
                routes::pool::create_pool,
                routes::pool::delete_pool,
                routes::pool::start_draft,
                routes::pool::select_player,
                routes::pool::create_trade,
                routes::pool::cancel_trade,
                routes::pool::respond_trade,
                routes::pool::fill_spot,
                routes::pool::protect_players,
            ],
        )
}