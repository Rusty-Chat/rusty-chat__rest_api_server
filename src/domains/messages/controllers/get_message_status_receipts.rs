use crate::AppState;
use crate::middlewares::auth_sessions_middleware::SessionsMiddlewareOutput;
use axum::{
    Json,
    extract::{Extension, Path, State},
    http::StatusCode,
    response::IntoResponse,
};
use chrono::NaiveDateTime;
use serde::Serialize;
use tracing::error;

#[derive(Debug, Serialize, sqlx::FromRow)]
pub struct MessageStatusReceipt {
    pub id: i64,
    pub message_id: i64,
    pub sender_id: i64,
    pub receiver_id: Option<i64>,
    pub room_id: i64,
    pub status: String,
    pub action: String,
    pub created_at: NaiveDateTime,
    pub updated_at: NaiveDateTime,
}

#[derive(Debug, Serialize)]
pub struct GetMessageStatusReceiptsResponse {
    pub response_message: String,
    pub response: Option<Vec<MessageStatusReceipt>>,
    pub error: Option<String>,
}

pub async fn get_message_status_receipts(
    State(state): State<AppState>,
    Extension(_session): Extension<SessionsMiddlewareOutput>,
    Path(message_id): Path<i64>,
) -> impl IntoResponse {
    let receipts_res = sqlx::query_as::<_, MessageStatusReceipt>(
        "SELECT * FROM message_status_receipts WHERE message_id = $1 ORDER BY created_at DESC",
    )
    .bind(message_id)
    .fetch_all(&state.db)
    .await;

    match receipts_res {
        Ok(receipts) => (
            StatusCode::OK,
            Json(GetMessageStatusReceiptsResponse {
                response_message: "Message status receipts fetched successfully".to_string(),
                response: Some(receipts),
                error: None,
            }),
        ),
        Err(e) => {
            error!("FAILED TO FETCH MESSAGE STATUS RECEIPTS!");
            (
                StatusCode::INTERNAL_SERVER_ERROR,
                Json(GetMessageStatusReceiptsResponse {
                    response_message: "Failed to fetch message status receipts".to_string(),
                    response: None,
                    error: Some(e.to_string()),
                }),
            )
        }
    }
}
