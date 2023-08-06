// This module setup the logger level.

use std::env;

pub fn setup(logger_level: &str) {
    if env::var_os("RUST_LOG").is_none() {
        let env = format!("rustapi={logger_level},tower_http={logger_level}");

        env::set_var("RUST_LOG", env);
    }

    tracing_subscriber::fmt::init();
}
