use crate::AppState;
use crate::middlewares::auth_sessions_middleware::SessionsMiddlewareOutput;
use axum::{
    Json,
    extract::{Extension, Path, Query, State},
    http::StatusCode,
    response::IntoResponse,
};
use chrono::NaiveDateTime;
use serde::{Deserialize, Serialize};
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
}

#[derive(Debug, Serialize)]
pub struct ResponseCore {
    count: usize,
    messages: Option<Vec<Message>>,
}

#[derive(Debug, Serialize)]
pub struct GetRoomMessagesResponse {
    pub response_message: String,
    pub response: Option<ResponseCore>,
    pub error: Option<String>,
}

#[derive(Deserialize)]
pub struct SearchParams {
    user_id: i64,
}

pub async fn get_room_messages(
    State(state): State<AppState>,
    Extension(session): Extension<SessionsMiddlewareOutput>,
    Query(params): Query<SearchParams>,
    Path(room_id): Path<i64>,
) -> impl IntoResponse {
    let user_id = params.user_id;

    // Step 1: Fetch all the messages in this room
    let messages_result = sqlx::query_as::<_, Message>(
        r#"
        SELECT * 
        FROM messages 
        WHERE room_id = $1 
        ORDER BY created_at ASC
        "#,
    )
    .bind(room_id)
    .fetch_all(&state.db)
    .await;

    match messages_result {
        Ok(msgs) => {
            return (
                StatusCode::OK,
                Json(GetRoomMessagesResponse {
                    response_message: "Room messages fetched successfully".to_string(),
                    response: Some(ResponseCore {
                        count: msgs.len(),
                        messages: Some(msgs),
                    }),
                    error: None,
                }),
            );
        }
        Err(e) => {
            error!("FAILED TO FETCH ROOM MESSAGES!");
            return (
                StatusCode::INTERNAL_SERVER_ERROR,
                Json(GetRoomMessagesResponse {
                    response_message: "Failed to fetch room messages".to_string(),
                    response: None,
                    error: Some(e.to_string()),
                }),
            );
        }
    };
}
