use config::{Config, ConfigError, Environment, File};
use serde::Deserialize;
use std::fmt;

#[derive(Debug, Clone, Deserialize)]
pub struct Server {
    // Interface to bind to. Defaults to 0.0.0.0 so the server is reachable when
    // running inside a container (127.0.0.1 would only bind the container's own
    // loopback, which published-port forwarding can't reach).
    #[serde(default = "default_host")]
    pub host: String,
    pub port: u16,
}

fn default_host() -> String {
    "0.0.0.0".to_string()
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
pub struct Redis {
    pub uri: String,
}

#[derive(Debug, Clone, Deserialize)]
pub struct Auth {
    // The endpoint hosted by hanko where is stored the JWKS to validate the jwt token.
    pub jwks_url: String,

    // The token audience to be able to validate the token (i.g., slapshot.xyz).
    pub token_audience: String,
}

#[derive(Debug, Clone, Deserialize)]
pub struct Settings {
    pub environment: String,
    pub server: Server,
    pub logger: Logger,
    pub database: Database,
    pub redis: Redis,
    pub auth: Auth,
}

impl Settings {
    pub fn new() -> Result<Self, ConfigError> {
        let config = if cfg!(debug_assertions) {
            "debug"
        } else {
            "release"
        };

        let builder = Config::builder()
            .add_source(File::with_name(&format!("config/{config}")))
            // Env vars override file values, e.g. APP_DATABASE__URI, APP_AUTH__JWKS_URL.
            // Lets secrets be injected at deploy time instead of living in the config file.
            .add_source(
                Environment::with_prefix("APP")
                    .prefix_separator("_")
                    .separator("__"),
            );

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
