use crate::AppState;
use crate::domains::auth::controllers::login_user::login_user;
use crate::domains::auth::controllers::logout_user::logout_user;
use crate::domains::auth::controllers::register_user::register_user;
use axum::{Router, routing::post};
use tower_cookies::CookieManagerLayer;

pub fn auth_routes(_state: &AppState) -> Router<AppState> {
    Router::new()
        .route("/register", post(register_user))
        .route("/login", post(login_user))
        .route("/logout", post(logout_user))
        .layer(CookieManagerLayer::new())
}
