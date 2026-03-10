use crate::AppState;
use crate::domains::messages::controllers::archive_message::archive_message;
use crate::domains::messages::controllers::bookmark_message::bookmark_message;
use crate::domains::messages::controllers::create_message::create_message;
use crate::domains::messages::controllers::delete_message::delete_message;
use crate::domains::messages::controllers::get_message_edit_history::get_message_edit_history;
use crate::domains::messages::controllers::get_message_status_receipts::get_message_status_receipts;
use crate::domains::messages::controllers::get_room_messages::get_room_messages;
use crate::domains::messages::controllers::react_to_message::react_to_message;
use crate::domains::messages::controllers::sync_messages_status_to_seen::sync_messages_status_to_seen;
use crate::domains::messages::controllers::sync_room_messages_status_to_delivered::sync_room_messages_status_to_delivered;
use crate::domains::messages::controllers::un_archive_message::un_archive_message;
use crate::domains::messages::controllers::un_bookmark_message::un_bookmark_message;
use crate::domains::messages::controllers::update_message::update_message;
use crate::middlewares::auth_access_middleware::access_middleware;
use crate::middlewares::auth_sessions_middleware::sessions_middleware;
use axum::routing::{delete, get, patch, post};
use axum::{Router, middleware};
use tower_cookies::CookieManagerLayer;

pub fn messages_routes(state: &AppState) -> Router<AppState> {
    Router::new()
        .route("/create-message", post(create_message))
        .route("/update-message/{message_id}", patch(update_message))
        .route(
            "/delete-message/{message_id}/{sender_id}",
            delete(delete_message),
        )
        .route(
            "/bookmark-message/{message_id}/{user_id}",
            post(bookmark_message),
        )
        .route(
            "/unbookmark-message/{message_id}/{user_id}",
            delete(un_bookmark_message),
        )
        .route(
            "/archive-message/{message_id}/{user_id}",
            post(archive_message),
        )
        .route(
            "/unarchive-message/{message_id}/{user_id}",
            delete(un_archive_message),
        )
        .route(
            "/get-message-edit-history/{message_id}",
            get(get_message_edit_history),
        )
        .route(
            "/get-message-status-receipts/{message_id}",
            get(get_message_status_receipts),
        )
        .route("/get-room-messages/{room_id}", get(get_room_messages))
        .route(
            "/sync-room-messages-status-to-delivered/{room_id}",
            post(sync_room_messages_status_to_delivered),
        )
        .route(
            "/sync-messages-status-to-seen",
            post(sync_messages_status_to_seen),
        )
        .route("/react-to-message/{message_id}", post(react_to_message))
        .route_layer(middleware::from_fn_with_state(
            state.clone(),
            access_middleware,
        ))
        .route_layer(middleware::from_fn_with_state(
            state.clone(),
            sessions_middleware,
        ))
        .layer(CookieManagerLayer::new())
}
