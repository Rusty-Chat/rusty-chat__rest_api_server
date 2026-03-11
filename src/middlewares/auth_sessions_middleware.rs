use axum::extract::State;
use axum::{
    Json,
    extract::Request,
    http::{StatusCode, header},
    middleware::Next,
    response::IntoResponse,
};
use chrono::NaiveDateTime;
use jsonwebtoken::{DecodingKey, Validation, decode, errors::ErrorKind};
use serde::{Deserialize, Serialize};
use tower_cookies::Cookies;
use tracing::error;

// ============================================================================
// Types
// ============================================================================

#[derive(Debug, Serialize)]
pub struct ErrorResponse {
    pub error: String,
    pub response_message: String,
}

#[derive(Debug, Serialize, Deserialize, PartialEq)]
pub enum TokenKind {
    Access,
    Refresh,
    OneTimePassword,
}

#[derive(Debug, Deserialize)]
pub struct JwtClaims {
    pub id: i64,
    pub email: String,
    pub exp: usize,
    pub iat: usize,
    pub token_kind: TokenKind,
}

#[derive(Debug, Serialize, sqlx::FromRow, Clone)]
pub struct UserProfile {
    pub id: i64,
    pub full_name: String,
    pub email: String,
    pub profile_image: Option<String>,
    pub access_token: Option<String>,
    pub refresh_token: Option<String>,
    pub status: String,
    pub last_seen: Option<String>,
    pub password: String,
    pub is_admin: bool,
    pub is_active: bool,
    pub created_at: NaiveDateTime,
    pub updated_at: NaiveDateTime,
}

#[derive(Clone)]
pub struct MiddlewareState {
    pub jwt_secret: String,
    pub cookie_name: String,
}

#[derive(Clone, Debug)]
pub struct SessionsMiddlewareOutput {
    pub user: UserProfile,
    pub session_status: String,
}

// ============================================================================
// Sessions Middleware
// ============================================================================

pub async fn sessions_middleware(
    // Extension(db_pool): Extension<PgPool>,
    State(state): State<crate::AppState>,
    cookies: Cookies,
    mut req: Request,
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

    // ------------------------------------------------------------------------
    // Extract required headers
    // ------------------------------------------------------------------------
    let email = req
        .headers()
        .get("email")
        .and_then(|v| v.to_str().ok())
        .ok_or_else(|| {
            error!("EMAIL HEADER MISSING!");

            (
                StatusCode::UNAUTHORIZED,
                Json(ErrorResponse {
                    error: "Unauthorized".to_string(),
                    response_message: "Email header missing".to_string(),
                }),
            )
        })?;

    let _authorization = req
        .headers()
        .get(header::AUTHORIZATION)
        .and_then(|v| v.to_str().ok())
        .ok_or_else(|| {
            error!("AUTHORIZATION HEADER MISSING!");

            (
                StatusCode::UNAUTHORIZED,
                Json(ErrorResponse {
                    error: "Unauthorized".to_string(),
                    response_message: "Authorization header missing".to_string(),
                }),
            )
        })?;

    // ------------------------------------------------------------------------
    // Validate cookie presence
    // ------------------------------------------------------------------------
    if cookies.get(&session_state.cookie_name).is_none() {
        error!("AUTH COOKIE NOT FOUND!");

        return Err((
            StatusCode::UNAUTHORIZED,
            Json(ErrorResponse {
                error: "Unauthorized".to_string(),
                response_message: "Request rejected, please re-authenticate".to_string(),
            }),
        ));
    }

    // ------------------------------------------------------------------------
    // Fetch user from database
    // ------------------------------------------------------------------------
    let user = match sqlx::query_as::<_, UserProfile>(
        r#"
        SELECT
            id,
            full_name,
            email,
            refresh_token,
            profile_image,
            is_admin,
            is_active,
            access_token,
            refresh_token,
            status,
            last_seen,
            password,
            created_at,
            updated_at
        FROM users
        WHERE email = $1
        "#,
    )
    .bind(email)
    .fetch_optional(&state.db)
    .await
    {
        Ok(Some(u)) => u,

        Ok(None) => {
            error!("USER NOT FOUND!");

            return Err((
                StatusCode::NOT_FOUND,
                Json(ErrorResponse {
                    error: "Not Found".to_string(),
                    response_message: format!("User '{}' not found", email),
                }),
            ));
        }

        Err(e) => {
            error!("USER FETCH FAILED!");

            return Err((
                StatusCode::INTERNAL_SERVER_ERROR,
                Json(ErrorResponse {
                    error: "DB Error".to_string(),
                    response_message: e.to_string(),
                }),
            ));
        }
    };

    // ------------------------------------------------------------------------
    // Check active status
    // ------------------------------------------------------------------------
    if !user.is_active {
        error!("INACTIVE USER ACCESS BLOCKED!");

        return Err((
            StatusCode::FORBIDDEN,
            Json(ErrorResponse {
                error: "Forbidden".to_string(),
                response_message: "Your account is deactivated".to_string(),
            }),
        ));
    }

    // ------------------------------------------------------------------------
    // Ensure refresh/session token exists
    // ------------------------------------------------------------------------
    let refresh = match &user.refresh_token {
        Some(t) => t.clone(),
        None => {
            error!("REFRESH TOKEN MISSING!");

            return Err((
                StatusCode::NOT_FOUND,
                Json(ErrorResponse {
                    error: "Not Found".to_string(),
                    response_message: "Refresh token missing".to_string(),
                }),
            ));
        }
    };

    // ------------------------------------------------------------------------
    // Validate refresh/session JWT
    // ------------------------------------------------------------------------
    let decoding_key = DecodingKey::from_secret(session_state.jwt_secret.as_bytes());

    match decode::<JwtClaims>(&refresh, &decoding_key, &Validation::default()) {
        Ok(token_data) => {
            if token_data.claims.email != user.email {
                error!("USER EMAIL CLAIM MISMATCH!");

                return Err((
                    StatusCode::UNAUTHORIZED,
                    Json(ErrorResponse {
                        error: "Unauthorized".to_string(),
                        response_message: "User credentials do not match".to_string(),
                    }),
                ));
            }

            if token_data.claims.token_kind != TokenKind::Refresh {
                error!("INVALID TOKEN TYPE CLAIM!");

                return Err((
                    StatusCode::UNAUTHORIZED,
                    Json(ErrorResponse {
                        error: "Unauthorized".to_string(),
                        response_message: "Invalid token type".to_string(),
                    }),
                ));
            }

            // Insert session data
            req.extensions_mut().insert(SessionsMiddlewareOutput {
                user: UserProfile {
                    id: user.id,
                    full_name: user.full_name,
                    email: user.email.clone(),
                    profile_image: user.profile_image,
                    access_token: user.access_token,
                    refresh_token: user.refresh_token,
                    status: user.status,
                    last_seen: user.last_seen,
                    password: user.password,
                    is_admin: user.is_admin,
                    is_active: user.is_active,
                    created_at: user.created_at,
                    updated_at: user.updated_at,
                },
                session_status: "USER SESSION IS ACTIVE".to_string(),
            });
        }

        Err(err) => match err.kind() {
            ErrorKind::ExpiredSignature => {
                error!("SESSION EXPIRED!");

                return Err((
                    StatusCode::FORBIDDEN,
                    Json(ErrorResponse {
                        error: "Forbidden".to_string(),
                        response_message: "User session expired, please re-authenticate"
                            .to_string(),
                    }),
                ));
            }

            _ => {
                error!("SESSION VERIFICATION FAILED!");

                return Err((
                    StatusCode::INTERNAL_SERVER_ERROR,
                    Json(ErrorResponse {
                        error: "Session Verification Failed".to_string(),
                        response_message: err.to_string(),
                    }),
                ));
            }
        },
    }

    Ok(next.run(req).await)
}
