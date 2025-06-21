use std::{io, env, sync::OnceLock};
use actix_web::{web, App, HttpServer};
use sqlx::{postgres::PgPoolOptions, PgPool};
use rdkafka::producer::FutureProducer;
use rdkafka::ClientConfig;
use bb8_redis::RedisConnectionManager;
use bb8::Pool;

mod url;

// We're retrieving the necessary env vars before beginning the service
static PORT: OnceLock<u16> = OnceLock::new();
static DATABASE_URL: OnceLock<String> = OnceLock::new();
static REDIS_URL: OnceLock<String> = OnceLock::new();
static KAFKA_BROKERS: OnceLock<String> = OnceLock::new();

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

fn get_kafka_brokers() -> &'static str {
    KAFKA_BROKERS.get_or_init(|| {
        env::var("KAFKA_BROKERS")
            .expect("Please specify the brokers for the Kafka microservice in variable KAFKA_BROKERS.")
    }).as_str()
}

#[tokio::main(flavor="current_thread")]
async fn main() -> io::Result<()> {
    println!("Backend server is starting...");

    // Connect to the database and pass it to the apps
    let db_pool = connect_to_db().await;

    // Connect to Redis and pass it to the apps
    let redis_pool = connect_to_redis().await;

    // Connect to Kafka and pass it to the apps
    let producer_data = connect_to_kafka().await;

    HttpServer::new(move || {
        App::new()
            .app_data(producer_data.clone())
            .app_data(redis_pool.clone())
            .app_data(db_pool.clone())
            .service(url::create_short_url)
            .service(url::get_short_url)
    })
    .bind(format!("0.0.0.0:{}", get_port()))?
    .run()
    .await
}

async fn connect_to_db() -> web::Data<PgPool> {
    let db_url = get_db_url();
    println!("Connecting to database at: {}", db_url);

    // Create a connection pool to the database
    let db_pool = PgPoolOptions::new()
        .max_connections(10)
        .connect(db_url)
        .await
        .expect("Failed to create Postgres pool");

    // Wrap the pool in web::Data and move it into the App
    let db_pool = web::Data::new(db_pool);
    println!("Connected to database successfully");
    db_pool
}

async fn connect_to_redis() -> web::Data<Pool<RedisConnectionManager>> {
    let redis_url = get_redis_url();
    println!("Connecting to Redis at: {}", redis_url);

    // Create a connection pool to Redis
    let redis_manager = RedisConnectionManager::new(redis_url).unwrap();
    let redis_pool = Pool::builder().build(redis_manager).await.unwrap();

    // Wrap the pool in web::Data and move it into the App
    let redis_pool = web::Data::new(redis_pool);
    println!("Connected to Redis successfully");
    redis_pool
}

async fn connect_to_kafka() -> web::Data<FutureProducer> {
    let kafka_brokers = get_kafka_brokers();
    println!("Connecting to Kafka at: {}", kafka_brokers);

    // Create a Kafka producer
    let producer: FutureProducer = ClientConfig::new()
        .set("bootstrap.servers", kafka_brokers)
        .create()
        .expect("Failed to create Kafka producer");

    // Wrap the producer in web::Data and move it into the App
    let producer_data = web::Data::new(producer);
    println!("Connected to Kafka successfully");
    producer_data
}