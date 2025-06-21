use std::{io, env, sync::OnceLock};
use actix_web::{web};
use rdkafka::consumer::{StreamConsumer, Consumer};
use rdkafka::ClientConfig;
use serde::{Deserialize};
use sqlx::{postgres::PgPoolOptions, PgPool};

static PORT: OnceLock<u16> = OnceLock::new();
static DATABASE_URL: OnceLock<String> = OnceLock::new();
static KAFKA_BROKERS: OnceLock<String> = OnceLock::new();
static KAFKA_TOPIC: OnceLock<String> = OnceLock::new();

#[derive(Debug, Deserialize)]
struct ClickEvent {
    short_url: String,
    ip_address: Option<String>,
    user_agent: Option<String>,
    referrer: Option<String>,
}

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

fn get_kafka_brokers() -> &'static str {
    KAFKA_BROKERS.get_or_init(|| {
        env::var("KAFKA_BROKERS")
            .expect("Please specify the brokers for the Kafka microservice in variable KAFKA_BROKERS.")
    }).as_str()
}

fn get_kafka_topic() -> &'static str {
    KAFKA_TOPIC.get_or_init(|| {
        env::var("KAFKA_TOPIC")
            .expect("Please specify the topic for the Kafka microservice in variable KAFKA_TOPIC.")
    }).as_str()
}



#[tokio::main(flavor="current_thread")]
async fn main() -> io::Result<()> {
    println!("Analytics server is starting...");

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
    println!("Connecting to Kafka at: {}", get_kafka_brokers());

    // Create a Kafka consumer
    let kafka_consumer: StreamConsumer = ClientConfig::new()
        .set("bootstrap.servers", get_kafka_brokers())
        .set("group.id", "analytics-service")
        .set("enable.partition.eof", "false")
        .set("auto.offset.reset", "earliest")
        .create()
        .expect("Failed to create Kafka consumer"); 

    kafka_consumer.subscribe(&[get_kafka_topic()])
        .expect("Failed to subscribe to Kafka topic");

    println!("Listening for messages on topic: {}", get_kafka_topic());

    let mut stream = kafka_consumer.stream();
    while let Some(msg) = stream.next().await {
        match msg {
            Ok(m) => {
                if let Some(payload) = m.payload() {
                    if let Ok(event) = serde_json::from_slice::<ClickEvent>(payload) {
                        //store_click_event(&pool, event).await;
                    } else {
                        eprintln!("Failed to deserialize message payload: {:?}", payload);
                    }
                }
            }
            Err(e) => {
                eprintln!("Kafka - Error receiving message: {:?}", e);
            }
        }
    }

    Ok(())
}

