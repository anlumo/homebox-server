use std::{error::Error, fs::read_to_string, net::SocketAddr, path::Path};

use serde::{Deserialize, Serialize};

#[derive(Serialize, Deserialize, Debug)]
pub struct Server {
    pub address: Option<SocketAddr>,
}

#[derive(Serialize, Deserialize, Debug)]
pub struct Database {
    pub file: String,
    pub metadata: String,
}

#[derive(Serialize, Deserialize, Debug)]
pub struct Auth {
    pub password: String,
}

#[derive(Deserialize)]
pub struct Config {
    pub logging: log4rs::config::RawConfig,
    pub server: Server,
    pub database: Database,
    pub auth: Auth,
}

impl Config {
    pub fn parse<P: AsRef<Path>>(
        path: P,
    ) -> Result<(Config, log4rs::config::Config), Box<dyn Error>> {
        let config = read_to_string(path)?;

        let config: Config = serde_yaml::from_str(&config)?;

        let config_deserializers = log4rs::config::Deserializers::new();
        let (appenders, mut errors) = config.logging.appenders_lossy(&config_deserializers);
        errors.handle();
        let (log4rs_config, mut errors) = log4rs::config::Config::builder()
            .appenders(appenders)
            .loggers(config.logging.loggers())
            .build_lossy(config.logging.root());
        errors.handle();

        Ok((config, log4rs_config))
    }
}
