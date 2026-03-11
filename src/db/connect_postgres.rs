//! # PostgreSQL Connection Handler
//!
//! This module provides functionality for establishing and managing
//! the connection pool to the PostgreSQL database.

use anyhow::{Context, Result};
use sqlx::postgres::PgPoolOptions;
use std::time::Duration;

/// Establishes a connection to the PostgreSQL database.
///
/// # Arguments
/// - `database_url`: The full connection string (e.g., `postgres://user:pass@host:port/dbname`).
/// - `max_connections`: Maximum number of concurrent connections in the pool.
/// - `acquire_timeout_secs`: Timeout in seconds for acquiring a connection from the pool.
///
/// # Errors
/// Returns an error if the connection fails, providing a detailed troubleshooting guide.
pub async fn connect_pg(
    database_url: String,
    max_connections: u32,
    acquire_timeout_secs: u64,
) -> Result<sqlx::PgPool> {
    let pool = PgPoolOptions::new()
        .max_connections(max_connections)
        .acquire_timeout(Duration::from_secs(acquire_timeout_secs))
        .connect(&database_url)
        .await;

    match pool {
        Ok(p) => Ok(p),
        Err(e) => {
            let redacted_url = match url::Url::parse(&database_url) {
                Ok(mut u) => {
                    let _ = u.set_password(None::<&str>);
                    let _ = u.set_username("...");
                    u.to_string()
                }
                Err(_) => "INVALID_DB_URL".to_string(),
            };

            Err(anyhow::anyhow!(
                "
                CRITICAL DATABASE CONNECTION ERROR:
                -------------------------------------------------
                Error: {}
                URL: {}
                -------------------------------------------------
                Please verify:
                1. Is Postgres running?
                2. Is the connection URL correct?
                3. Are the credentials valid?
                4. Is the network allowing connection to the configured port?
                -------------------------------------------------
                ",
                e,
                redacted_url
            ))
            .context("DATABASE CONNECTION FAILED")
        }
    }
}
