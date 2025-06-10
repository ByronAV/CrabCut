use std::{io, env, sync::OnceLock};
use actix_web::{web, App, HttpServer};
use sqlx::{postgres::PgPoolOptions};

mod url;

// We're retrieving the necessary env vars before beginning the service
static PORT: OnceLock<u16> = OnceLock::new();
static DATABASE_URL: OnceLock<String> = OnceLock::new();
static REDIS_URL: OnceLock<String> = OnceLock::new();

fn get_port() -> u16 {
    *PORT.get_or_init(|| {
        env::var("PORT")
            .ok()
            .and_then(|val| val.parse::<u16>().ok())
            .expect("Please specify the port number for the HTTP server with the environment variable PORT.")
    })
}

fn get_db_url() -> &'static str {
    DATABASE_URL.get_or_init(|| {
        env::var("DATABASE_URL")
            .expect("Please specify the server for the PostgreSQL Database microservice in variable DATABASE_URL.")
    }).as_str()
}

fn get_redis_url() -> &'static str {
    REDIS_URL.get_or_init(|| {
        env::var("REDIS_URL")
            .expect("Please specify the server for the Redis microservice in variable REDIS_URL.")
    }).as_str()
}

#[tokio::main(flavor="current_thread")]
async fn main() -> io::Result<()> {
    println!("Backend server is starting...");

    let db_url = get_db_url();
    println!("Connecting to database at: {}", db_url);

    // Create a connection pool to the database
    let pool = PgPoolOptions::new()
        .max_connections(10)
        .connect(db_url)
        .await
        .expect("Failed to create Postgres pool");

    // Wrap the pool in web::Data and move it into the App
    let pool = web::Data::new(pool);

    println!("Connected to database successfully");
    println!("Connecting to Redis at: {}", get_redis_url());
    
    // Create the redis client
    let redis_client = redis::Client::open(get_redis_url()
        .to_string()  // Convert the static str to String
    ).expect("Failed to create Redis client");

    let redis_client = web::Data::new(redis_client);
    println!("Connected to Redis successfully");

    HttpServer::new(move || {
        App::new()
            .app_data(redis_client.clone())
            .app_data(pool.clone())
            .service(url::create_short_url)
            .service(url::get_short_url)
    })
    .bind(format!("0.0.0.0:{}", get_port()))?
    .run()
    .await
}