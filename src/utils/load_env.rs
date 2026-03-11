use dotenvy;
use std::env;

pub fn load_env() {
    dotenvy::dotenv().ok();

    let env = env::var("APP__DEPLOY__ENV").unwrap_or("development".into());
    let filename = format!(".env.{}", env);

    dotenvy::from_filename(&filename).ok();
    // println!("Loaded config: {}", filename);
}
