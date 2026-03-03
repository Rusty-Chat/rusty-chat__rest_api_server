use crate::AppState;
use crate::middlewares::auth_sessions_middleware::SessionsMiddlewareOutput;
use axum::{
    extract::{Path, State, Extension},
    http::StatusCode,
    response::IntoResponse,
    Json,
};
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
pub struct DeleteMessageResponse {
    pub response_message: String,
    pub response: Option<Message>,
    pub error: Option<String>,
}

pub async fn delete_message(
    State(state): State<AppState>,
    Extension(session): Extension<SessionsMiddlewareOutput>,
    Path((message_id, sender_id)): Path<(i64, i64)>,
) -> impl IntoResponse {
    // 1. Fetch message to check ownership and get room_id
    let message_result = sqlx::query_as::<_, Message>("SELECT * FROM messages WHERE id = $1")
        .bind(message_id)
        .fetch_optional(&state.db)
        .await;

    let message = match message_result {
        Ok(Some(m)) => m,
        Ok(None) => {
            error!("MESSAGE NOT FOUND!");
            
            return (
                StatusCode::NOT_FOUND,
                Json(DeleteMessageResponse {
                    response_message: "Message not found or does not exist".to_string(),
                    response: None,
                    error: Some("Message not found".to_string()),
                }),
            );
        }
        Err(e) => {
            error!("DATABASE ERROR!");
            
            return (
                StatusCode::INTERNAL_SERVER_ERROR,
                Json(DeleteMessageResponse {
                    response_message: "Database error".to_string(),
                    response: None,
                    error: Some(e.to_string()),
                }),
            );
        }
    };

    // 2. Check permissions (sender or admin)
    if message.sender_id != Some(sender_id) && !session.user.is_admin {
        error!("UNAUTHORIZED MESSAGE DELETE ATTEMPT!");
        
        return (
            StatusCode::FORBIDDEN,
            Json(DeleteMessageResponse {
                response_message: "You don't have permission to delete this message".to_string(),
                response: None,
                error: Some("Forbidden".to_string()),
            }),
        );
    }

    // 4. Delete message(status receipts are automatically deleted on message delivery).
    let delete_res = sqlx::query("DELETE FROM messages WHERE id = $1")
        .bind(message_id)
        .execute(&state.db)
        .await;

    match delete_res {
        Ok(_) => (
            StatusCode::OK,
            Json(DeleteMessageResponse {
                response_message: "Message deleted successfully".to_string(),
                response: Some(message),
                error: None,
            }),
        ),
        Err(e) => {
            error!("FAILED_TO_DELETE_MESSAGE!");
            
            (
                StatusCode::INTERNAL_SERVER_ERROR,
                Json(DeleteMessageResponse {
                    response_message: "Failed to delete message".to_string(),
                    response: None,
                    error: Some(e.to_string()),
                }),
            )
        }
    }
}
