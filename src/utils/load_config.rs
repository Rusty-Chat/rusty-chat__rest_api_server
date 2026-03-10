//! # Configuration Management
//!
//! This module handles loading and validating the application configuration from
//! multiple sources: base TOML files, environment-specific overrides, local
//! overrides, and environment variables.

use anyhow::{Context, Result};
use config::{Config, Environment, File};
use serde::Deserialize;
use std::fmt;

/// Application-specific metadata section.
#[derive(Debug, Deserialize)]
pub struct AppSection {
    /// The name of the application.
    pub name: String,
    /// The current environment (e.g., development, production).
    pub environment: Option<String>,
}

#[derive(Debug, Deserialize)]
pub struct ClientIntegrationsSection {
    #[serde(default)]
    pub allow_access_middleware: bool,

    #[serde(default)]
    pub allow_sessions_middleware: bool,

    #[serde(default)]
    pub allow_logging_middleware: bool,

    #[serde(default)]
    pub allow_request_timeout_middleware: bool,

    #[serde(default)]
    pub allow_admin_routes_protector_middleware: bool,
}

#[derive(Debug, Deserialize)]
pub struct ObservabilitySection {
    pub enable_tracing: bool,
    pub enable_metrics: bool,
}

#[derive(Debug, Deserialize)]
pub struct ServerSection {
    pub host: String,
    pub port: u16,
    // pub graceful_shutdown_secs: u64,
}

#[derive(Debug, Deserialize)]
pub struct DatabaseSection {
    pub engine: String,
    pub host: String,
    pub port: u16,
    pub user: Option<String>,
    pub password: Option<String>,
    pub name: String,
    pub max_connections: u32,
    pub connect_timeout_secs: u64,
}

#[derive(Debug, Deserialize)]
pub struct AuthSection {
    pub jwt_secret: String,
    pub jwt_access_expiration_time_in_hours: u64,
    pub jwt_session_expiration_time_in_hours: u64,
    pub jwt_one_time_password_lifetime_in_minutes: u64,
}

#[derive(Debug, Deserialize)]
pub struct AwsSection {
    pub access_key: String,
    pub secret_access_key: String,
    pub bucket_url: String,
    pub s3_bucket_region: String,
    pub s3_bucket_name: String,
}

// #[derive(Debug, Deserialize)]
// pub struct SecuritySection {
//     pub bcrypt_cost: u32,
//     pub rate_limit_per_minute: u32,
// }

/// Root configuration structure containing all application settings.
#[derive(Debug, Deserialize)]
pub struct AppConfig {
    pub app: AppSection,
    pub client_integrations: ClientIntegrationsSection,
    pub observability: ObservabilitySection,

    // Optional / currently commented-out sections
    pub server: Option<ServerSection>,
    pub database: Option<DatabaseSection>,
    pub auth: Option<AuthSection>,
    pub aws: Option<AwsSection>,
    // pub security: Option<SecuritySection>,
}

/// Loads the application configuration.
///
/// Order of precedence (highest to lowest):
/// 1. Environment variables (prefixed with `APP__`) - overrides every other configuration setup
/// 2. `config/local.toml` - overrides `config/{APP__ENV}.toml` and `config/base.toml`
/// 3. `config/{APP__ENV}.toml` - overrides `config/base.toml`
/// 4. `config/base.toml` - default values
pub fn load_config() -> Result<AppConfig> {
    // Determine environment
    let env = std::env::var("APP__ENV")
        .context("APP__ENV environment variable is not set! Please set it to 'development', 'production', etc.")?;

    // Build configuration
    let builder = Config::builder()
        // Base config is required
        .add_source(File::with_name("config/base").required(true))
        // Environment-specific overrides (optional)
        .add_source(File::with_name(&format!("config/{}", env)).required(false))
        // Local overrides (optional, for dev machines)
        .add_source(File::with_name("config/local").required(false))
        // Environment variable overrides
        .add_source(
            Environment::default()
                .separator("__") // maps APP__SECTION__FIELD → section.field
                .prefix("APP") // all vars must start with APP__
                .try_parsing(true), // parse numbers/booleans automatically
        );

    /**************** EXPLAINING THE MAPPING RULE FOR THE [ABOVE] FINAL ENV OVERRIDES ****************
    # Mapping Rule (exact)

    APP__<SECTION>__<FIELD>=value - E.g. APP__SERVER__PORT=9000

    Lowercase / uppercase differences are normalized (handled without manual intervention).

    So this TOML:

    [server]
    port = 8080

    will be overridden by:

    APP__SERVER__PORT=9000

    If the names don't align, nothing happens.

    Example (❌ no override):

    SERVER_PORT=9000

    This does nothing unless you explicitly read it in code.

    **************** EXPLAINING THE MAPPING RULE FOR THE [ABOVE] FINAL ENV OVERRIDES ****************/

    builder
        .build()
        .context("Failed to build config")?
        .try_deserialize()
        .context("Invalid config shape")
}

#[derive(Debug)]
pub enum ConfigError {
    MissingAppName,
    InvalidServerPort,
    MissingServerSection,
    MissingDatabaseSection,
    MissingDatabaseName,
    MissingDatabaseUser,
    MissingDatabasePassword,
    MissingAuthSection,
    MissingJwtSecret,
    MissingAwsSection,
    MissingAwsAccessKey,
    MissingAwsSecretAccessKey,
    MissingAwsBucketUrl,
    MissingAwsS3BucketRegion,
    MissingAwsS3BucketName,
}

impl fmt::Display for ConfigError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            ConfigError::MissingAppName => write!(f, "app.name cannot be empty"),
            ConfigError::InvalidServerPort => write!(f, "server.port cannot be 0"),
            ConfigError::MissingServerSection => write!(f, "server section is missing"),
            ConfigError::MissingDatabaseSection => write!(f, "database section is missing"),
            ConfigError::MissingDatabaseName => write!(f, "database.name cannot be empty"),
            ConfigError::MissingDatabaseUser => write!(f, "database.user cannot be empty"),
            ConfigError::MissingDatabasePassword => write!(f, "database.password cannot be empty"),
            ConfigError::MissingAuthSection => write!(f, "auth section is missing"),
            ConfigError::MissingJwtSecret => write!(f, "auth.jwt_secret cannot be empty"),
            ConfigError::MissingAwsSection => write!(f, "aws section is missing"),
            ConfigError::MissingAwsAccessKey => write!(f, "aws.access_key cannot be empty"),
            ConfigError::MissingAwsSecretAccessKey => {
                write!(f, "aws.secret_access_key cannot be empty")
            }
            ConfigError::MissingAwsBucketUrl => write!(f, "aws.bucket_url cannot be empty"),
            ConfigError::MissingAwsS3BucketRegion => {
                write!(f, "aws.s3_bucket_region cannot be empty")
            }
            ConfigError::MissingAwsS3BucketName => {
                write!(f, "aws.s3_bucket_name cannot be empty")
            }
        }
    }
}

impl std::error::Error for ConfigError {}

impl AppConfig {
    pub fn validate(&self) -> std::result::Result<(), ConfigError> {
        // Check app name
        if self.app.name.trim().is_empty() {
            return Err(ConfigError::MissingAppName);
        }

        // Check server
        let server = self
            .server
            .as_ref()
            .ok_or(ConfigError::MissingServerSection)?;
        if server.port == 0 {
            return Err(ConfigError::InvalidServerPort);
        }

        // Check database
        let database = self
            .database
            .as_ref()
            .ok_or(ConfigError::MissingDatabaseSection)?;
        if database.name.trim().is_empty() {
            return Err(ConfigError::MissingDatabaseName);
        }
        if database
            .user
            .as_ref()
            .map(|s| s.trim().is_empty())
            .unwrap_or(true)
        {
            return Err(ConfigError::MissingDatabaseUser);
        }
        if database
            .password
            .as_ref()
            .map(|s| s.trim().is_empty())
            .unwrap_or(true)
        {
            return Err(ConfigError::MissingDatabasePassword);
        }

        // Check auth
        let auth = self.auth.as_ref().ok_or(ConfigError::MissingAuthSection)?;
        if auth.jwt_secret.trim().is_empty() {
            return Err(ConfigError::MissingJwtSecret);
        }

        // Check AWS
        let aws = self.aws.as_ref().ok_or(ConfigError::MissingAwsSection)?;
        if aws.access_key.trim().is_empty() {
            return Err(ConfigError::MissingAwsAccessKey);
        }
        if aws.secret_access_key.trim().is_empty() {
            return Err(ConfigError::MissingAwsSecretAccessKey);
        }
        if aws.bucket_url.trim().is_empty() {
            return Err(ConfigError::MissingAwsBucketUrl);
        }
        if aws.s3_bucket_region.trim().is_empty() {
            return Err(ConfigError::MissingAwsS3BucketRegion);
        }
        if aws.s3_bucket_name.trim().is_empty() {
            return Err(ConfigError::MissingAwsS3BucketName);
        }

        Ok(())
    }
}
