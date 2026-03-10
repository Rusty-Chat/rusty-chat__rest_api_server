use crate::AppState;
use crate::domains::rooms::controllers::create_group_chat_room::create_group;
use crate::domains::rooms::controllers::create_private_chat_room::create_room;
use crate::domains::rooms::controllers::update_room_profile_image::update_room_profile_image;

use crate::domains::rooms::controllers::add_room_member::add_room_member;
use crate::domains::rooms::controllers::archive_room::archive_room;
use crate::domains::rooms::controllers::bookmark_room::bookmark_room;
use crate::domains::rooms::controllers::get_all_closed_rooms::get_all_closed_rooms;
use crate::domains::rooms::controllers::get_all_group_rooms::get_all_group_rooms;
use crate::domains::rooms::controllers::get_all_open_rooms::get_all_open_rooms;
use crate::domains::rooms::controllers::get_all_private_rooms::get_all_private_rooms;
use crate::domains::rooms::controllers::get_all_rooms::get_all_rooms;
use crate::domains::rooms::controllers::get_room::get_room;
use crate::domains::rooms::controllers::get_user_rooms::get_user_rooms;
use crate::domains::rooms::controllers::pin_room::pin_room;
use crate::domains::rooms::controllers::unarchive_room::unarchive_room;
use crate::domains::rooms::controllers::unbookmark_room::unbookmark_room;
use crate::domains::rooms::controllers::unpin_room::unpin_room;
use crate::domains::rooms::controllers::update_room::update_room;

use crate::domains::rooms::controllers::add_room_admin::add_room_admin;
use crate::domains::rooms::controllers::remove_room_admin::remove_room_admin;
use crate::domains::rooms::controllers::remove_room_member::remove_room_member;
use crate::middlewares::auth_access_middleware::access_middleware;
use crate::middlewares::auth_sessions_middleware::sessions_middleware;
use axum::routing::{get, patch, post};
use axum::{Router, middleware};
use tower_cookies::CookieManagerLayer;

pub fn rooms_routes(state: &AppState) -> Router<AppState> {
    Router::new()
        .route("/create-private-chat-room", post(create_room))
        .route("/create-group-chat-room", post(create_group))
        .route("/update-room/{room_id}", patch(update_room))
        .route(
            "/update-room-profile-image/{room_id}",
            patch(update_room_profile_image),
        )
        .route("/get-room/{room_id}", get(get_room))
        .route("/get-all-rooms", get(get_all_rooms))
        .route("/get-user-rooms/{user_id}", get(get_user_rooms))
        .route("/get-all-group-rooms", get(get_all_group_rooms))
        .route("/get-all-private-rooms", get(get_all_private_rooms))
        .route("/get-all-open-rooms", get(get_all_open_rooms))
        .route("/get-all-closed-rooms", get(get_all_closed_rooms))
        .route("/bookmark-room/{room_id}", patch(bookmark_room))
        .route("/unbookmark-room/{room_id}", patch(unbookmark_room))
        .route("/pin-room/{room_id}", patch(pin_room))
        .route("/unpin-room/{room_id}", patch(unpin_room))
        .route("/archive-room/{room_id}", patch(archive_room))
        .route("/unarchive-room/{room_id}", patch(unarchive_room))
        .route("/add-room-member/{room_id}", post(add_room_member))
        .route("/remove-room-member/{room_id}", post(remove_room_member))
        .route("/add-room-admin/{room_id}", patch(add_room_admin))
        .route("/remove-room-admin/{room_id}", patch(remove_room_admin))
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
