use crate::AppState;
use crate::middlewares::auth_sessions_middleware::SessionsMiddlewareOutput;
use axum::{
    Json,
    extract::{Extension, Path, State},
    http::StatusCode,
    response::IntoResponse,
};
use chrono::{DateTime, Utc};
use serde::Serialize;
use tracing::error;

#[derive(Debug, Serialize, sqlx::FromRow)]
pub struct MessageEdit {
    pub id: i64,
    pub message_id: i64,
    pub previous_context: String,
    pub new_content: String,
    pub created_at: DateTime<Utc>,
}

#[derive(Debug, Serialize)]
pub struct GetMessageEditHistoryResponse {
    pub response_message: String,
    pub response: Option<Vec<MessageEdit>>,
    pub error: Option<String>,
}

pub async fn get_message_edit_history(
    State(state): State<AppState>,
    Extension(_session): Extension<SessionsMiddlewareOutput>,
    Path(message_id): Path<i64>,
) -> impl IntoResponse {
    let edits_res = sqlx::query_as::<_, MessageEdit>(
        "SELECT * FROM message_edits WHERE message_id = $1 ORDER BY created_at DESC",
    )
    .bind(message_id)
    .fetch_all(&state.db)
    .await;

    match edits_res {
        Ok(edits) => (
            StatusCode::OK,
            Json(GetMessageEditHistoryResponse {
                response_message: "Message edit history fetched successfully".to_string(),
                response: Some(edits),
                error: None,
            }),
        ),
        Err(e) => {
            error!("FAILED TO FETCH MESSAGE EDIT HISTORY!");
            (
                StatusCode::INTERNAL_SERVER_ERROR,
                Json(GetMessageEditHistoryResponse {
                    response_message: "Failed to fetch message edit history".to_string(),
                    response: None,
                    error: Some(e.to_string()),
                }),
            )
        }
    }
}
