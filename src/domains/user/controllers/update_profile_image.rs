use crate::middlewares::auth_sessions_middleware::SessionsMiddlewareOutput;

use crate::AppState;
use crate::domains::user::controllers::update_user::UserLookup;
use crate::utils::file_upload_handler::{UploadType, upload_file};
use axum::extract::State;
use axum::{
    Json,
    extract::Multipart,
    extract::{Extension, Path},
    http::StatusCode,
    response::IntoResponse,
};
use chrono::NaiveDateTime;
use serde::Serialize;
use tower_cookies::Cookies;
use tracing::error;


#[derive(Debug, Serialize, sqlx::FromRow)]
pub struct UserProfile {
    id: i64,
    full_name: String,
    email: String,
    profile_image: Option<String>,
    access_token: String,
    refresh_token: String,
    status: String,
    last_seen: Option<String>,
    #[serde(skip_serializing)]
    password: String,
    is_admin: bool,
    is_active: bool,
    country: String,
    phone_number: String,
    is_logged_out: bool,
    pub created_at: NaiveDateTime,
    pub updated_at: NaiveDateTime,
}


#[derive(Debug, Serialize)]
pub struct UpdateResponse {
    response_message: String,
    response: Option<UserProfile>,
    error: Option<String>,
}

pub async fn update_profile_image(
    _cookies: Cookies,
    // Extension(db_pool): Extension<PgPool>,
    State(state): State<AppState>,
    Extension(session): Extension<SessionsMiddlewareOutput>,
    Path(user_id): Path<i64>,
    mut multipart: Multipart,
    // Json(payload): Json<UpdateProfileImagePayload>,
) -> impl IntoResponse {
    // extract file for upload
    // let field: Result<Option<Field>, MultipartError> = multipart
    let user_result = sqlx::query_as::<_, UserLookup>(
        "SELECT id, email, created_at, updated_at FROM users WHERE id = $1",
    )
    // user_id from request param
    .bind(user_id)
    .fetch_optional(&state.db)
    .await;

    let user = match user_result {
        Ok(Some(user)) => user,
        Ok(None) => {
            error!("PROFILE IMAGE UPDATE FAILED: USER NOT FOUND!");

            return (
                StatusCode::UNAUTHORIZED,
                Json(UpdateResponse {
                    response_message: "Login failed".to_string(),
                    response: None,
                    error: Some("User not found, profile update failed".to_string()),
                }),
            );
        }
        Err(e) => {
            error!("PROFILE IMAGE UPDATE FAILED!");

            return (
                StatusCode::INTERNAL_SERVER_ERROR,
                Json(UpdateResponse {
                    response_message: "User update failed".to_string(),
                    response: None,
                    error: Some(format!("Database error: {}", e)),
                }),
            );
        }
    };

    // only an admin or owner of the profile can update
    if session.user.email != user.email && !session.user.is_admin {
        error!("UNAUTHORIZED USER UPDATE ATTEMPT!");

        return (
            StatusCode::UNAUTHORIZED,
            Json(UpdateResponse {
                response_message: "You're not permitted to perform this action for this user"
                    .into(),
                response: None,
                error: Some("Unauthorized user update attempt".into()),
            }),
        );
    }

    let file = match multipart.next_field().await {
        Ok(Some(file)) => file,
        Ok(None) => {
            error!("FILE UPLOAD FAILED!");

            return (
                StatusCode::BAD_REQUEST,
                Json(UpdateResponse {
                    response_message: "No file provided".into(),
                    response: None,
                    error: Some("File upload failed".into()),
                }),
            );
        }
        Err(e) => {
            error!("FAILED TO EXTRACT FILE FOR UPLOAD: {}", e);

            return (
                StatusCode::INTERNAL_SERVER_ERROR,
                Json(UpdateResponse {
                    response_message: "File upload failed".into(),
                    response: None,
                    error: Some(e.to_string()),
                }),
            );
        }
    };

    let file_url =
        match upload_file(State(&state), file, &user_id, UploadType::UserProfileImage).await {
            Ok(file_url) => file_url,
            Err(e) => {
                error!("PROFILE IMAGE UPLOAD FAILED!");

                return (
                    StatusCode::INTERNAL_SERVER_ERROR,
                    Json(UpdateResponse {
                        response_message: "Failed to update user".into(),
                        response: None,
                        error: Some(format!("Database error: {}", e)),
                    }),
                );
            }
        };

    let res = sqlx::query_as::<_, UserProfile>(
        r#"
            UPDATE users
            SET
                profile_image = $1,
                updated_at = NOW()
            WHERE id = $2
            RETURNING *
            "#,
    )
    .bind(file_url)
    .bind(user_id)
    .fetch_one(&state.db)
    .await;

    match res {
        Ok(updated_user) => (
            StatusCode::OK,
            Json(UpdateResponse {
                response_message: "User updated successfully".into(),
                response: Some(updated_user),
                error: None,
            }),
        ),
        Err(e) => {
            error!("FAILED TO UPDATE USER PROFILE IMAGE!");

            (
                StatusCode::INTERNAL_SERVER_ERROR,
                Json(UpdateResponse {
                    response_message: "Failed to update user".into(),
                    response: None,
                    error: Some(format!("Database error: {}", e)),
                }),
            )
        }
    }
}
