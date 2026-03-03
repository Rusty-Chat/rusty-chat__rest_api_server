use crate::AppState;
use axum::{extract::{Path, State}, http::StatusCode, response::IntoResponse, Json};
use chrono::NaiveDateTime;
use serde::Serialize;
use tracing::error;

#[derive(Debug, Serialize, sqlx::FromRow)]
pub struct Message {
    pub id: i64,
    pub room_id: i64,
    pub sender_id: Option<i64>,
    #[sqlx(rename = "type")]
    pub message_type: String,
    pub text_content: Option<String>,
    pub attachment_1: Option<String>,
    pub attachment_2: Option<String>,
    pub attachment_3: Option<String>,
    pub attachment_4: Option<String>,
    pub status: String,
    pub sent_at: String,
    pub created_at: NaiveDateTime,
    pub updated_at: NaiveDateTime,
}

#[derive(Debug, Serialize)]
pub struct ArchiveResponse {
    pub response_message: String,
    pub response: Option<Message>,
    pub error: Option<String>,
}

pub async fn un_archive_message(
    State(state): State<AppState>,
    // Extension(session): Extension<SessionsMiddlewareOutput>,
    Path((message_id, user_id)): Path<(i64, i64)>,
) -> impl IntoResponse {
    // First, un-archive the message
    let unarchive_res = sqlx::query(
        "DELETE FROM message_archives WHERE user_id = $1 AND message_id = $2"
    )
    .bind(user_id)
    .bind(message_id)
    .execute(&state.db)
    .await;

    match unarchive_res {
        Ok(_) => {
            // Fetch the message details
            let message_res = sqlx::query_as::<_, Message>(
                "SELECT id, room_id, sender_id, type, text_content, attachment_1, attachment_2, attachment_3, attachment_4, status, sent_at, created_at, updated_at FROM messages WHERE id = $1"
            )
            .bind(message_id)
            .fetch_one(&state.db)
            .await;

            match message_res {
                Ok(message) => (
                    StatusCode::OK,
                    Json(ArchiveResponse {
                        response_message: "Message un-archived successfully".to_string(),
                        response: Some(message),
                        error: None,
                    }),
                ),
                Err(e) => {
                    error!("FAILED_TO_FETCH_UN_ARCHIVED_MESSAGE!");
                    (
                        StatusCode::INTERNAL_SERVER_ERROR,
                        Json(ArchiveResponse {
                            response_message: "Message un-archived but failed to fetch details".to_string(),
                            response: None,
                            error: Some(e.to_string()),
                        }),
                    )
                }
            }
        },
        Err(e) => {
            error!("FAILED_TO_UN_ARCHIVE_MESSAGE!");
            (
                StatusCode::INTERNAL_SERVER_ERROR,
                Json(ArchiveResponse {
                    response_message: "Failed to un-archive message".to_string(),
                    response: None,
                    error: Some(e.to_string()),
                }),
            )
        }
    }
}
