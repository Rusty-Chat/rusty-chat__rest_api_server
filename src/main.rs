use anyhow::{Context, Result};
use axum::{Router, middleware};
use sqlx::PgPool;
use std::net::SocketAddr;
use std::sync::Arc;
use tokio::net::lookup_host;

// AWS S3
use crate::utils::file_upload_handler::S3AppState;
use aws_config::Region;
use aws_credential_types::Credentials;
use aws_sdk_s3::Client;

// logging init with the tracing crate
use percent_encoding::{NON_ALPHANUMERIC, utf8_percent_encode};
use tracing::error;
use tracing::info;
use tracing_subscriber::fmt::time::SystemTime;

// utils import
mod utils;
use crate::utils::load_config::{AppConfig, load_config};
use crate::utils::load_env::load_env;
// db import
mod db;
use db::connect_postgres::connect_pg;

// controllers import
mod domains;
use crate::domains::admin::router::admin_routes;
use crate::domains::messages::router::messages_routes;
use crate::domains::rooms::router::rooms_routes;
use crate::domains::user::router::user_routes;

mod middlewares;
use crate::middlewares::logging_middleware::logging_middleware;
use crate::middlewares::request_timeout_middleware::timeout_middleware;

#[derive(Clone, Debug)]
pub struct AppState {
    pub config: Arc<AppConfig>,
    pub db: PgPool,
    pub s3: S3AppState,
}

fn initialize_logging() {
    tracing_subscriber::fmt()
        .json()
        .with_timer(SystemTime)
        // .with_thread_ids(true)
        .with_level(true)
        .init();
}

#[tokio::main]
async fn main() {
    load_env();
    initialize_logging();

    let clean_config = match load_config() {
        Ok(config) => {
            if let Err(e) = config.validate() {
                error!(
                    "SERVER START-UP ERROR: FAILED TO LOAD SERVER CONFIGURATIONS, {}!",
                    e
                );
                std::process::exit(1);
            }

            config
        }
        Err(e) => {
            error!(
                "SERVER START-UP ERROR: FAILED TO LOAD SERVER CONFIGURATIONS, {}!",
                e
            );
            std::process::exit(1);
        }
    };

    let aws_config_ref = match clean_config.aws.as_ref() {
        Some(config) => config,
        None => {
            error!("SERVER START-UP ERROR: AWS CONFIGURATION IS MISSING!");
            std::process::exit(1);
        }
    };

    // note here that the "None" is in place of a session token
    let s3_credentials = Credentials::from_keys(
        aws_config_ref.access_key.clone(),
        aws_config_ref.secret_access_key.clone(),
        None,
    );

    let s3_config = aws_config::from_env()
        .region(Region::new(aws_config_ref.s3_bucket_region.clone()))
        .credentials_provider(s3_credentials)
        .load()
        .await;

    let s3_client = Client::new(&s3_config);

    let s3_state = S3AppState {
        s3_client,
        bucket_name: aws_config_ref.s3_bucket_name.clone(),
        bucket_url: aws_config_ref.bucket_url.clone(),
    };

    let db_config = match clean_config.database.as_ref() {
        Some(config) => config,
        None => {
            error!("SERVER START-UP ERROR: DATABASE CONFIGURATION IS MISSING!");
            std::process::exit(1);
        }
    };

    let db_user = match db_config.user.as_deref() {
        Some(user) => user,
        None => {
            error!("SERVER START-UP ERROR: DATABASE USER IS MISSING!");
            std::process::exit(1);
        }
    };

    let db_password = match db_config.password.as_deref() {
        Some(password) => password,
        None => {
            error!("SERVER START-UP ERROR: DATABASE PASSWORD IS MISSING!");
            std::process::exit(1);
        }
    };

    let encoded_user = utf8_percent_encode(db_user, NON_ALPHANUMERIC).to_string();
    let encoded_password = utf8_percent_encode(db_password, NON_ALPHANUMERIC).to_string();

    let database_url = format!(
        "postgres://{}:{}@{}:{}/{}",
        encoded_user, encoded_password, db_config.host, db_config.port, db_config.name
    );
    let db_host = db_config.host.clone();
    let db_port = db_config.port;

    let db_pool = match connect_pg(
        database_url.clone(),
        db_config.max_connections,
        db_config.connect_timeout_secs,
    )
    .await
    {
        Ok(pool) => pool,
        Err(e) => {
            error!("SERVER START-UP ERROR: DATABASE CONNECTION FAILED: {}!", e);
            std::process::exit(1);
        }
    };

    let server_config = match clean_config.server.as_ref() {
        Some(config) => config,
        None => {
            error!("SERVER START-UP ERROR: SERVER CONFIGURATION IS MISSING!");
            std::process::exit(1);
        }
    };

    let host = server_config.host.clone();
    let port = server_config.port;
    let environment = clean_config
        .app
        .environment
        .clone()
        .unwrap_or("development".to_string());

    let state = AppState {
        config: Arc::new(clean_config),
        db: db_pool,
        s3: s3_state,
    };

    // fn verify_config_loading(state: &AppState) {
    //     println!(
    //         "Config loaded successfully: {} is running on {}:{}",
    //         state.config.app.name,
    //         state.config.server.as_ref().unwrap().host,
    //         state.config.server.as_ref().unwrap().port
    //     );
    // }

    // verify_config_loading(&state);

    let app = Router::new()
        .nest("/api/v1/user", user_routes(&state))
        .nest("/api/v1/admin", admin_routes(&state))
        .nest("/api/v1/rooms", rooms_routes(&state))
        .nest("/api/v1/messages", messages_routes(&state))
        .layer(middleware::from_fn(logging_middleware))
        .layer(middleware::from_fn(timeout_middleware))
        .with_state(state);

    // .layer(Extension(db_pool));

    async fn resolve_bind_address(host: &str, port: u16) -> Result<SocketAddr> {
        let bind_addr = format!("{host}:{port}");

        let mut resolved_addrs = lookup_host(&bind_addr)
            .await
            .with_context(|| format!("Failed to resolve bind address: {bind_addr}"))?;

        let addr = resolved_addrs
            .next()
            .ok_or_else(|| anyhow::anyhow!("No socket addresses resolved for {bind_addr}"))?;

        Ok(addr)
    }

    fn build_database_url(db_host: &str, db_port: u16) -> String {
        format!("postgres://...@{db_host}:{db_port}/..")
    }

    let addr = match resolve_bind_address(&host, port).await {
        Ok(addr) => addr,
        Err(err) => {
            error!(
                "SERVER START-UP ERROR: Failed to resolve bind address ({}:{}): {:?}",
                host, port, err
            );
            std::process::exit(1);
        }
    };

    let slice_db_url = build_database_url(&host, db_port);

    // Start server
    let listener = match tokio::net::TcpListener::bind(addr).await {
        Ok(listener) => {
            print!(
                "
                .................................................
                Connected to DB: {}
                Environment: {}
                Status: DB connected successfully
                .................................................

                Server running on http://{}
                ",
                slice_db_url, environment, addr
            );

            listener
        }
        Err(e) => {
            error!("SERVER INITIALIZATION ERROR: {}!", e);
            std::process::exit(1);
        }
    };

    let server_result = axum::serve(listener, app).await;

    match server_result {
        Ok(_) => {
            info!("Graceful server shutdown!");
        }
        Err(e) => {
            error!("SERVER SHUTDOWN ERROR: {}!", e);
        }
    }
}
