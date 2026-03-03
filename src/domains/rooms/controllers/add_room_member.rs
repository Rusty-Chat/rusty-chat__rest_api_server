use crate::{AppState};
use axum::{
    Json,
    extract::{Path, State},
    http::StatusCode,
    response::IntoResponse,
};
use serde::{Deserialize, Serialize};
use std::time::{SystemTime, UNIX_EPOCH};
use tracing::error;
use chrono::NaiveDateTime;

#[derive(Debug, Deserialize)]
pub struct AddMemberPayload {
    pub user_id: i64,
    pub role: Option<String>, // "admin" or "member", default "member"
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
    pub co_members: Vec<i64>, // for group rooms only
    pub is_public: bool,
    pub created_at: NaiveDateTime,
    pub updated_at: NaiveDateTime,
}

#[derive(Debug, Serialize)]
pub struct Response {
    response_message: String,
    error: Option<String>,
}

fn current_time_millis() -> u128 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .expect("Failed to evaluate time in milliseconds!")
        .as_millis()
}

pub async fn add_room_member(
    State(state): State<AppState>,
    Path(room_id): Path<i64>,
    Json(payload): Json<AddMemberPayload>,
) -> impl IntoResponse {
    if let Some(ref role) = payload.role && role != "admin" && role != "member" {
        return (
            StatusCode::BAD_REQUEST,
            Json(Response {
                response_message: "Invalid role".into(),
                error: Some("Role must be 'admin' or 'member'".into()),
            }),
        );
    }
    let joined_at = current_time_millis().to_string();

    // Check if user is already a member?
    let user_exists = sqlx::query(
        r#"
        SELECT 1
        FROM users
        WHERE id = $1
        "#,
    )
    .bind(payload.user_id)
    .fetch_one(&state.db)
    .await;

    if user_exists.is_err() {
        return (
            StatusCode::NOT_FOUND,
            Json(Response {
                response_message: format!(
                    "User with id: '{}' not found or does not exist",
                    payload.user_id
                ),
                error: Some("Member does not exist or room not found".into()),
            }),
        );
    }

    let room_exists = sqlx::query_as::<_, Room>(
        r#"
        SELECT *
        FROM rooms
        WHERE id = $1
        "#,
    )
    .bind(room_id)
    .fetch_one(&state.db)
    .await;

    match room_exists {
        Ok(r) => r,
        Err(e) => {
            return (
                StatusCode::NOT_FOUND,
                Json(Response {
                    response_message: format!(
                        "Room with id: '{}' not found or does not exist",
                        room_id
                    ),
                    error: Some(format!("{:?}", e)),
                }),
            );
        }
    };

    let result = sqlx::query(
        r#"
        INSERT INTO room_members (room_id, user_id, role, joined_at)
        VALUES ($1, $2, $3, $4)
        ON CONFLICT (room_id, user_id) DO NOTHING
        "#,
    )
    .bind(room_id)
    .bind(payload.user_id)
    .bind(payload.role.unwrap_or_else(|| "member".to_string()))
    .bind(joined_at)
    .execute(&state.db)
    .await;

    match result {
        Ok(query_result) => {
            if query_result.rows_affected() == 0 {
                (
                    StatusCode::CONFLICT, // or OK with message "Already a member"
                    Json(Response {
                        response_message: format!(
                            "User with id: '{}' is already a member of this room",
                            payload.user_id
                        ),
                        error: None,
                    }),
                )
            } else {
                // Update the rooms table to include the new member in co_members array
                let update_result = sqlx::query(
                    "UPDATE rooms SET co_members = array_append(co_members, $1) WHERE id = $2",
                )
                .bind(payload.user_id)
                .bind(room_id)
                .execute(&state.db)
                .await;

                if let Err(e) = update_result {
                    error!("Failed to update rooms co_members: {}", e);
                    return (
                        StatusCode::INTERNAL_SERVER_ERROR,
                        Json(Response {
                            response_message: "Failed to update room details".into(),
                            error: Some(format!("Database error: {}", e)),
                        }),
                    );
                }

                (
                    StatusCode::CREATED,
                    Json(Response {
                        response_message: "Member added successfully".into(),
                        error: None,
                    }),
                )
            }
        }
        Err(e) => {
            error!("ADD ROOM MEMBER REQUEST FAILED");
            (
                StatusCode::INTERNAL_SERVER_ERROR,
                Json(Response {
                    response_message: "Failed to add room member".into(),
                    error: Some(format!("Database error: {}", e)),
                }),
            )
        }
    }
}
