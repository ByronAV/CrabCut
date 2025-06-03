use std::{env, sync::OnceLock};
use actix_web::{App, HttpServer};

mod url;
mod user;
mod password;

// We're retrieving the necessary env vars before beginning the service
static PORT: OnceLock<u16> = OnceLock::new();

fn get_port() -> u16 {
    *PORT.get_or_init(|| {
        env::var("PORT")
            .ok()
            .and_then(|val| val.parse::<u16>().ok())
            .expect("Please specify the port number for the HTTP server with the environment variable PORT.")
    })
}

async fn main() -> Result<(), Box<dyn std::error::Error>> {
    println!("Backend server is starting...");

    HttpServer::new(|| {
        App::new()
            .service(url::create_short_url)
            .service(url::get_short_url)
    })
    .bind(format!("0.0.0.0:{}", get_port()))?
    .run()
    .await
}