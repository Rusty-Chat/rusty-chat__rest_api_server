use crate::middlewares::auth_sessions_middleware::{SessionsMiddlewareOutput, UserProfile};
use crate::utils::generate_tokens::{User, generate_tokens};
use axum::{
    Json,
    extract::{Request, State},
    http::{StatusCode, header},
    middleware::Next,
    response::IntoResponse,
};
use jsonwebtoken::{DecodingKey, Validation, decode};
use serde::{Deserialize, Serialize};
use tower_cookies::Cookies;
use tracing::error;

// ============================================================================
// Types/Structures
// ============================================================================

#[derive(Debug, Serialize, Deserialize)]
pub struct JwtClaims {
    pub id: i64,
    pub email: String,
    pub exp: usize,
    pub iat: usize,
}

#[derive(Clone)]
pub struct MiddlewareState {
    pub jwt_secret: String,
    pub cookie_name: String,
}

#[derive(Clone, Debug)]
pub struct SessionInfo {
    pub user: User,
    pub new_access_token: String,
    pub new_refresh_token: String,
    pub session_status: String,
}

// #[derive(Debug, Serialize, sqlx::FromRow, Clone)]
// pub struct UserProfile {
//     pub id: i64,
//     pub full_name: String,
//     pub email: String,
//     pub profile_image_url: Option<String>,
//     pub access_token: Option<String>,
//     pub refresh_token: Option<String>,
//     pub status: String,
//     pub last_seen: Option<String>,
//     #[serde(skip_serializing)]
//     pub password: String,
//     pub is_admin: bool,
//     pub is_active: bool,
// }

#[derive(Debug, Serialize)]
pub struct ErrorResponse {
    pub error: String,
    pub response_message: String,
}

pub struct AuthTokens {
    pub access_token: String,
    pub refresh_token: String,
    pub auth_cookie: String,
}

enum TokenStatus {
    Valid,
    Expired,
    Invalid(String),
}

fn verify_access_token(token: &str, secret: &str, user: &UserProfile) -> TokenStatus {
    let validation = Validation::default();
    let decoding_key = DecodingKey::from_secret(secret.as_bytes());

    match decode::<JwtClaims>(token, &decoding_key, &validation) {
        Ok(token_data) => {
            if token_data.claims.email != user.email {
                return TokenStatus::Invalid("User credentials do not match".to_string());
            }
            TokenStatus::Valid
        }
        Err(err) => {
            use jsonwebtoken::errors::ErrorKind;
            match err.kind() {
                ErrorKind::ExpiredSignature => TokenStatus::Expired,
                _ => TokenStatus::Invalid(format!("Token verification failed: {}", err)),
            }
        }
    }
}

// ============================================================================
// Middleware Implementation
// ============================================================================

pub async fn access_middleware(
    State(state): State<crate::AppState>,
    cookies: Cookies,
    req: Request,
    next: Next,
) -> impl IntoResponse {
    let auth_config = match state.config.auth.as_ref() {
        Some(config) => config,
        None => {
            error!("AUTH CONFIGURATION MISSING!");
            return Err((
                StatusCode::INTERNAL_SERVER_ERROR,
                Json(ErrorResponse {
                    error: "Server Error".to_string(),
                    response_message: "Auth configuration is missing".to_string(),
                }),
            ));
        }
    };

    let session_state = MiddlewareState {
        jwt_secret: auth_config.jwt_secret.clone(),
        cookie_name: "rusty_chat_auth_cookie".to_string(),
    };

    // ----------------------------------------------------------
    // AUTH COOKIE CHECK
    // ----------------------------------------------------------
    let _auth_cookie = cookies.get(&session_state.cookie_name).ok_or_else(|| {
        error!("MISSING AUTH COOKIE!");
        (
            StatusCode::UNAUTHORIZED,
            Json(ErrorResponse {
                error: "Unauthorized".to_string(),
                response_message: "Request rejected, please re-authenticate".to_string(),
            }),
        )
    })?;

    // ----------------------------------------------------------
    // SESSION MIDDLEWARE OUTPUT CHECK
    // ----------------------------------------------------------
    let sessions_middleware_output = req
        .extensions()
        .get::<SessionsMiddlewareOutput>()
        .ok_or_else(|| {
            error!("SESSION MIDDLEWARE OUTPUT MISSING!");
            (
                StatusCode::NOT_FOUND,
                Json(ErrorResponse {
                    error: "Not Found".to_string(),
                    response_message: "_ User not received from sessions middleware".to_string(),
                }),
            )
        })?
        .clone();

    // ----------------------------------------------------------
    // AUTH HEADER CHECK
    // ----------------------------------------------------------
    let auth_header = req
        .headers()
        .get(header::AUTHORIZATION)
        .and_then(|h| h.to_str().ok())
        .ok_or_else(|| {
            error!("AUTHORIZATION HEADER MISSING!");
            (
                StatusCode::FORBIDDEN,
                Json(ErrorResponse {
                    error: "Forbidden".to_string(),
                    response_message: "Authorization header missing".to_string(),
                }),
            )
        })?;

    // ----------------------------------------------------------
    // BEARER FORMAT CHECK
    // ----------------------------------------------------------
    if !auth_header.starts_with("Bearer ") {
        error!("INVALID BEARER FORMAT!");
        return Err((
            StatusCode::FORBIDDEN,
            Json(ErrorResponse {
                error: "Forbidden".to_string(),
                response_message:
                    "Authorization string does not match expected (Bearer Token) format".to_string(),
            }),
        ));
    }

    let access_token = auth_header.trim_start_matches("Bearer ");

    // ----------------------------------------------------------
    // TOKEN GENERATION (FOR RENEWAL)
    // ----------------------------------------------------------
    let _tokens = match generate_tokens(
        "auth",
        User {
            id: sessions_middleware_output.user.id,
            email: sessions_middleware_output.user.email.clone(),
        },
        state.config.as_ref(),
    )
    .await
    {
        Ok(tokens) => tokens,
        Err(_) => {
            error!("TOKEN GENERATION ERROR!");
            return Err((
                StatusCode::INTERNAL_SERVER_ERROR,
                Json(ErrorResponse {
                    response_message: "Failed to generate tokens".to_string(),
                    error: "Token generation error".to_string(),
                }),
            ));
        }
    };

    // ----------------------------------------------------------
    // ACCESS TOKEN VERIFICATION
    // ----------------------------------------------------------
    match verify_access_token(
        access_token,
        &session_state.jwt_secret,
        &sessions_middleware_output.user,
    ) {
        TokenStatus::Valid => {
            // normal path (no log)
        }

        TokenStatus::Expired => {
            // normal path (no log)
        }

        TokenStatus::Invalid(msg) => {
            error!("INVALID ACCESS TOKEN!");
            return Err((
                StatusCode::UNAUTHORIZED,
                Json(ErrorResponse {
                    error: "Unauthorized".to_string(),
                    response_message: msg,
                }),
            ));
        }
    }

    // ----------------------------------------------------------
    // NORMAL FLOW (NO LOGGING)
    // ----------------------------------------------------------
    Ok(next.run(req).await)
}
