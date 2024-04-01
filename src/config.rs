use crate::error::KnawledgeError;
use clap::Parser;
use serde::Deserialize;
use std::{collections::HashMap, fs, path::Path};

#[derive(Debug, Clone, Parser)]
pub struct StartArgs {
    #[arg(short, long, default_value = "config.json")]
    pub config_path: String,

    #[arg(short, long, default_value = "127.0.0.1")]
    pub address: String,

    #[arg(short, long, default_value = "3030")]
    pub port: u16,

    #[arg(short, long, default_value = "INFO")]
    pub log_level: tracing::Level,
}

#[derive(Debug, Clone, Deserialize)]
pub struct Config {
    /// The document title for the front end
    pub title: Option<String>,

    /// The list of directories to initially include for the public page.
    /// Maps names to directory paths.
    pub directories: HashMap<String, String>,

    pub admin: Option<AdminConfig>,
}

impl Config {
    pub fn read(path: impl AsRef<Path>) -> Result<Self, KnawledgeError> {
        let config = fs::read_to_string(path)?;
        Ok(serde_json::from_str(&config)?)
    }
}

#[derive(Debug, Clone, Deserialize)]
pub struct AdminConfig {
    pub cookie_domain: String,
    #[serde(alias = "password_hash")]
    pub pw_hash: String,
}
