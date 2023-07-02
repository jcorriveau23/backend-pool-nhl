use config::{Config, ConfigError, File};
use lazy_static::lazy_static;
use serde::Deserialize;
use std::fmt;

lazy_static! {
    pub static ref SETTINGS: Settings = Settings::new().expect("Failed to setup settings");
}

#[derive(Debug, Clone, Deserialize)]
pub struct Server {
    pub port: u16,
}

#[derive(Debug, Clone, Deserialize)]
pub struct Logger {
    pub level: String,
}

#[derive(Debug, Clone, Deserialize)]
pub struct Database {
    pub uri: String,
    pub name: String,
}

#[derive(Debug, Clone, Deserialize)]
pub struct Auth {
    pub secret: String,
}

#[derive(Debug, Clone, Deserialize)]
pub struct Settings {
    pub environment: String,
    pub server: Server,
    pub logger: Logger,
    pub database: Database,
    pub auth: Auth,
}

impl Settings {
    pub fn new() -> Result<Self, ConfigError> {
        let config = if cfg!(debug_assertions) {
            "debug"
        } else {
            "release"
        };

        let builder = Config::builder().add_source(File::with_name(&format!("config/{config}")));

        builder
            .build()?
            // Deserialize (and thus freeze) the entire configuration.
            .try_deserialize()
    }
}

impl fmt::Display for Server {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        write!(f, "http://localhost:{}", &self.port)
    }
}
