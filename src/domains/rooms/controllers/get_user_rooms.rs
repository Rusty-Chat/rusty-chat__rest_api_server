use crate::AppState;
// use crate::middlewares::auth_sessions_middleware::SessionsMiddlewareOutput;
use axum::{
    Json,
    extract::{Path, State},
    http::StatusCode,
    response::IntoResponse,
};
use chrono::NaiveDateTime;
use serde::Serialize;
use tracing::error;

#[derive(Debug, Serialize, sqlx::FromRow)]
pub struct UserProfile {
    // #[sqlx(rename = "id")]
    id: i64,
    full_name: String,
    email: String,
    profile_image: Option<String>,
    #[serde(skip_serializing)]
    password: String,
    is_admin: bool,
    is_active: bool,
    status: String,
    country: String,
    phone_number: String,
    is_logged_out: bool,
    pub created_at: NaiveDateTime,
    pub updated_at: NaiveDateTime,
}

#[derive(Debug, Serialize, sqlx::FromRow)]
pub struct Room {
    pub id: i64,
    pub room_name: Option<String>,
    pub is_group: bool,
    pub created_by: Option<i64>,
    pub bookmarked_by: Vec<i64>,
    pub archived_by: Vec<i64>,
    pub pinned_by: Vec<i64>,
    pub room_profile_image: Option<String>,
    pub co_member: Option<i64>,
    pub co_members: Option<Vec<i64>>,
    pub is_public: bool,
    pub created_at: NaiveDateTime,
    pub updated_at: NaiveDateTime,
}

#[derive(Debug, Serialize)]
pub struct ResponseCore {
    count: usize,
    rooms: Option<Vec<Room>>,
}
#[derive(Debug, Serialize)]
pub struct RoomsResponse {
    response_message: String,
    response: Option<ResponseCore>,
    error: Option<String>,
}

pub async fn get_user_rooms(
    State(state): State<AppState>,
    // Extension(session): Extension<SessionsMiddlewareOutput>,
    Path(user_id): Path<i64>,
) -> impl IntoResponse {
    // Fetch user by email
    let user_result = sqlx::query_as::<_, UserProfile>(
        "SELECT id, full_name, email, profile_image, password, is_active, is_admin, country, phone_number, is_logged_out, status, created_at, updated_at FROM users WHERE id = $1",
    )
    .bind(&user_id)
    .fetch_optional(&state.db)
    .await;

    if user_result.is_err() {
        error!("USER NOT FOUND!");

        return (
            StatusCode::NOT_FOUND,
            Json(RoomsResponse {
                response_message: format!("User with id {} not found or does not exist", user_id),
                response: None,
                error: Some("NOT FOUND".into()),
            }),
        );
    }

    let result = sqlx::query_as::<_, Room>(
        r#"
        SELECT r.* 
        FROM rooms r
        INNER JOIN room_members rm ON r.id = rm.room_id
        WHERE rm.user_id = $1
        "#,
    )
    .bind(user_id)
    .fetch_all(&state.db)
    .await;

    match result {
        Ok(rooms) => {
            if !rooms.is_empty() {
                let room_ids: Vec<i64> = rooms.iter().map(|r| r.id).collect();

                // Set all message status receipts of all the messages in those rooms to "seen"
                let _ = sqlx::query(
                    r#"
                    INSERT INTO message_status_receipts (message_id, sender_id, room_id, status, action)
                    SELECT m.id, $1, m.room_id, 'seen', 'original-send'
                    FROM messages m
                    WHERE m.room_id = ANY($2)
                    AND NOT EXISTS (
                        SELECT 1 FROM message_status_receipts msr 
                        WHERE msr.message_id = m.id AND msr.sender_id = $1 AND msr.status = 'seen'
                    )
                    "#
                )
                .bind(user_id)
                .bind(&room_ids)
                .execute(&state.db)
                .await
                .map_err(|e| {
                    error!("FAILED TO UPDATE MESSAGE STATUS RECEIPTS TO SEEN: {}", e);
                    e
                });
            }

            (
                StatusCode::OK,
                Json(RoomsResponse {
                    response_message: "User rooms retrieved successfully".into(),
                    response: Some(ResponseCore {
                        count: rooms.len(),
                        rooms: Some(rooms),
                    }),
                    error: None,
                }),
            )
        }
        Err(e) => {
            error!("FETCH USER ROOMS REQUEST FAILED");
            (
                StatusCode::INTERNAL_SERVER_ERROR,
                Json(RoomsResponse {
                    response_message: "Failed to retrieve user rooms".into(),
                    response: None,
                    error: Some(format!("Database error: {}", e)),
                }),
            )
        }
    }
}
