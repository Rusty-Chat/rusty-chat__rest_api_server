use crate::middlewares::auth_sessions_middleware::UserProfile;
use crate::utils::generate_tokens::User;
use axum::{
    Json,
    extract::{Request, State},
    http::StatusCode,
    middleware::Next,
    response::IntoResponse,
};
use crate::middlewares::auth_access_middleware::ErrorResponse;
use sqlx;
use tracing::error;

// ============================================================================
// Middleware Implementation
// ============================================================================

pub async fn admin_routes_protector(
    State(state): State<crate::AppState>,
    req: Request,
    next: Next,
) -> impl IntoResponse {
    let email = match req.headers().get("email").and_then(|h| h.to_str().ok()) {
        Some(user_email) => user_email,
        None => {
            error!("FAILED TO EXTRACT EMAIL HEADER ON ADMIN ROUTE REQUEST!");

            return Err((
                StatusCode::NOT_FOUND,
                Json(ErrorResponse {
                    error: "Unsuccessful request".to_string(),
                    response_message: "Failed to extract email header on request".to_string(),
                }),
            ));
        }
    };

    match sqlx::query_as::<_, UserProfile>(
        r#"
        SELECT
            id,
            full_name,
            email,
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
        Ok(Some(u)) => {
            if !u.is_admin || !u.is_active {
                error!("ADMIN ROUTE HIT BY UNAUTHORIZED USER!");

                return Err((
                    StatusCode::FORBIDDEN,
                    Json(ErrorResponse {
                        error: "Forbidden".to_string(),
                        response_message: "Only active admins can perform this action".to_string(),
                    }),
                ));
            }
        }

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

    // ----------------------------------------------------------
    // NORMAL FLOW (NO LOGGING)
    // ----------------------------------------------------------
    Ok(next.run(req).await)
}
