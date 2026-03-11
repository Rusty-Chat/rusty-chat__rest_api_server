// use cookie::Cookie;
use crate::utils::load_config::AppConfig;
use time;
use tower_cookies::{Cookie, Cookies};

pub async fn deploy_auth_cookie(cookies: Cookies, cookie_value: String, config: &AppConfig) {
    // let cookie = Cookie::build(("name", "value"))
    //     .domain("www.rustychat.com")
    //     .path("/")
    //     .secure(true)
    //     .http_only(true);
    //
    // jar.add(cookie);
    // // jar.remove(Cookie::build("name").path("/"));

    // Create a basic cookie
    let mut cookie = Cookie::new("rusty_chat_auth_cookie", cookie_value);

    let is_dev = config.app.environment.as_deref().unwrap_or("production") == "development";
    let max_age_hours = config
        .auth
        .as_ref()
        .map(|auth| auth.jwt_session_expiration_time_in_hours)
        .and_then(|hours| i64::try_from(hours).ok())
        .unwrap_or(24);

    // Set cookie attributes for security
    cookie.set_path("/");
    cookie.set_http_only(true);
    cookie.set_secure(!is_dev);
    cookie.set_same_site(tower_cookies::cookie::SameSite::Lax);

    // Optional: set expiration
    cookie.set_max_age(time::Duration::hours(max_age_hours));

    cookies.add(cookie);
}
