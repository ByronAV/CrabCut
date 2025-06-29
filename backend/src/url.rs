
use actix_web::{web, get, post, Responder, HttpRequest, HttpResponse, web::Json};
use serde::{Deserialize, Serialize};
use serde_json;
use sha2::{Digest, Sha256};
use base64::{engine::general_purpose::URL_SAFE_NO_PAD, Engine as _};
use sqlx::PgPool;
use bb8::Pool;
use bb8_redis::RedisConnectionManager;
use rdkafka::producer::{FutureProducer, FutureRecord};
use std::sync::Arc;
use bb8_redis::redis::AsyncCommands;

type SharedRedisPool = Arc<Pool<RedisConnectionManager>>;

#[derive(Deserialize)]
struct ShortenRequest {
    long_url: String,
    custom_alias: Option<String>,
}

#[derive(Serialize)]
pub struct ClickEvent {
    pub short_url: String,
    pub ip_address: Option<String>,
    pub user_agent: Option<String>,
    pub referer: Option<String>,
}

#[get("/health")]
pub async fn health_check() -> impl Responder {
    HttpResponse::Ok().body("OK")
}

#[post("/create")]
pub async fn create_short_url(
    req: Json<ShortenRequest>,
    db_pool: web::Data<PgPool>,
    redis_pool: web::Data<SharedRedisPool>) -> impl Responder {

    let alias = req.custom_alias.as_deref().unwrap_or("");
    let long_url = req.long_url.trim();
    // Check here first if we got a bad request
    if (!alias.is_empty() && !validate_alias(alias)) || long_url.is_empty() {
        return HttpResponse::BadRequest().body("Invalid input");
    } else if alias.is_empty() {
        // If the alias is empty, we generate a random one
        // For now, we will just return a placeholder
        let short_url = hash_long_url(long_url);
        // Here we have a new short URL. We are going to store this
        // in the database for future reference.
        save_short_url_to_db(&db_pool, &redis_pool, long_url, &short_url);
        return HttpResponse::Ok().json(serde_json::json!({
            "short_url": format!("https://crabcut.io/{}", short_url)    
        }));
    } else {
        // We have an alias, we will use it
        if !validate_alias(alias) {
            return HttpResponse::BadRequest().body("Invalid alias");
        }
        // Check if the alias is unique
        if !is_alias_unique(&db_pool, alias).await.unwrap_or_else(|_| {
            // If we cannot check the alias, we return a server error
            eprintln!("Error checking alias uniqueness");
            false
        }) {
            return HttpResponse::Conflict().body("Alias already exists");
        }
        save_short_url_to_db(&db_pool, &redis_pool, long_url, alias);
        // If the alias is valid and unique, we can create the short URL
        return HttpResponse::Ok().json(serde_json::json!({
            "short_url": format!("https://crabcut.io/{}", alias)
        }));
    }
}

#[get("/{short_url}")]
pub async fn get_short_url(
    req: HttpRequest,
    path: web::Path<String>,
    redis_pool: web::Data<SharedRedisPool>,
    db_pool: web::Data<PgPool>,
    kafka_producer: web::Data<FutureProducer>,
) -> impl Responder {

    // First we are going to check if short url is in cache
    // If it is, we will return the long url
    // If it is not, we will check the database
    let short_url = path.into_inner();
    let mut redis_conn = match redis_pool.get().await {
        Ok(c) => c,
        Err(_) => return HttpResponse::InternalServerError().body("Failed to connect to Redis"),
    };


    // Check redis cache first
    let cached_url: Option<String> = match redis_conn.get(&short_url).await {
        Ok(url) => url,
        Err(e) => {
            eprintln!("Failed to get from Redis: {:?}", e);
            return HttpResponse::InternalServerError().body("Internal Server Error");
        }
    };

    if let Some(long_url) = cached_url {
        // Cache hit!!!, return it
        let short_url_clone = short_url.clone();
        let long_url_clone = long_url.clone();
        let pool_clone = db_pool.clone();
        let redis_pool_clone = redis_pool.clone();
        tokio::spawn(async move {
            let _ = sqlx::query!(
                r#"
                UPDATE urls SET click_count = click_count + 1
                WHERE short_url = $1
                "#,
                short_url_clone
            )
            .execute(pool_clone.get_ref())
            .await;
            
            // Reset the expiration time for the cached URL
            if let Ok(mut redis_conn) = redis_pool_clone.get().await {
                let _ = redis_conn.expire(&short_url_clone, 3600).await;
            }
        });

        // Create a ClickEvent to log the click
        // and send it to Kafka
        let click = ClickEvent {
            short_url: short_url.clone(),
            ip_address: req.peer_addr().map(|addr| addr.ip().to_string()),
            user_agent: req.headers().get("User-Agent").and_then(|h| h.to_str().ok()).map(String::from),
            referer: req.headers().get("Referer").and_then(|h| h.to_str().ok()).map(String::from),
        };
        tokio::spawn(handle_click_event(&kafka_producer.get_ref().clone(), click));

        return HttpResponse::Found()
            .append_header(("Location", long_url))
            .finish();
    }

    // Otherwise cache miss :(
    let result = sqlx::query!(
        r#"
        SELECT long_url FROM urls
        WHERE short_url = $1
        LIMIT 1
        "#,
        short_url
    )
    .fetch_one(db_pool.get_ref())
    .await;

    let long_url = match result {
        Ok(record) => record.long_url,
        Err(sqlx::Error::RowNotFound) => {
            // If the record is not found, we return a 404
            return HttpResponse::NotFound().body("Short URL not found");
        }
        Err(e) => {
            eprintln!("Database error: {:?}", e);
            return HttpResponse::InternalServerError().body("Internal Server Error");
        }
    };

    let short_url_clone = short_url.clone();
    let long_url_clone = long_url.clone();
    let pool_clone = db_pool.clone();
    let redis_pool_clone = redis_pool.clone();
    tokio::spawn(async move {
        let _ = sqlx::query!(
            "UPDATE urls SET click_count = click_count + 1 WHERE short_url = $1",
            short_url_clone
        )
        .execute(pool_clone.get_ref())
        .await;

        // Store the long URL in Redis cache with an expiration time of 1 hour
        if let Ok(mut redis_conn) = redis_pool_clone.get().await {
            let _ = redis_conn.set_ex(&short_url_clone, &long_url_clone, 3600).await;
        }
    });

    // Create a ClickEvent to log the click
    // and send it to Kafka
    let click = ClickEvent {
        short_url: short_url.clone(),
        ip_address: req.peer_addr().map(|addr| addr.ip().to_string()),
        user_agent: req.headers().get("User-Agent").and_then(|h| h.to_str().ok()).map(String::from),
        referer: req.headers().get("Referer").and_then(|h| h.to_str().ok()).map(String::from),
    };
    tokio::spawn(handle_click_event(&kafka_producer.get_ref().clone(), click));

    HttpResponse::Found()
        .append_header(("Location", long_url))
        .finish()
}

fn validate_alias(alias: &str) -> bool {
    // Checking that the alias is less than 16 chars
    if alias.len() > 16 {
        return false;
    }
    // We are accepting only a-zA-Z0-9 for now
    return alias.chars().all(|c| c.is_ascii_alphanumeric())
}

fn hash_long_url(long_url: &str) -> String {
    // This function takes the long URL and creates a
    // unique short URL for it. We are using SHA256
    // to create a hash of the long URL and then
    // encoding it in base64 to create a short URL.
    
    let mut hasher = Sha256::new();
    hasher.update(long_url);
    let hash = hasher.finalize();
    // Encoding the hash in base64
    let short_url = URL_SAFE_NO_PAD.encode(&hash);
    // We will take the first 8 characters of the hash
    let short_url = &short_url[..8];
    return short_url.to_string();
}

async fn is_alias_unique(db_pool: &PgPool, alias: &str) -> Result<bool, sqlx::Error> {
    // Check in database if the alias is unique
    let record = sqlx::query!(
        r#"
        SELECT 1 as exists_flag FROM urls
        WHERE short_url = $1
        LIMIT 1
        "#,
        alias
    )
    .fetch_optional(db_pool)
    .await?;

    Ok(record.is_none())
}

fn save_short_url_to_db(db_pool: &PgPool, redis_pool: &SharedRedisPool, long_url: &str, short_url: &str) {
    let long_url = long_url.to_string();
    let short_url = short_url.to_string();
    let db_pool = db_pool.clone(); // `PgPool` is `Clone`
    let redis_pool_clone = redis_pool.clone();

    println!("Adding URL to database: long_url = {}, short_url = {}", long_url, short_url);
    tokio::spawn(async move {
        if let Err(e) = sqlx::query!(
            r#"
            INSERT INTO urls (long_url, short_url)
            VALUES ($1, $2)
            ON CONFLICT (short_url) DO NOTHING
            "#,
            long_url,
            short_url
        )
        .execute(&db_pool)
        .await
        {
            eprintln!("Failed to insert URL: {:?}", e); // Log or handle error
        }

        // Store the long URL in Redis cache with an expiration time of 1 hour
        let mut redis_conn = redis_pool_clone.get().await.expect("Failed to get Redis connection");
        let _ : Result<(), _> = redis_conn.set_ex(&short_url, &long_url, 3600).await;
    });
}

async fn handle_click_event(producer: &FutureProducer, click: ClickEvent) {
    let payload = serde_json::to_string(&click).unwrap();

    let record = FutureRecord::to("click-events")
        .payload(&payload)
        .key(&click.short_url);

    // Send data to Kafka (non-blocking, fire-and-forget)
    let _ = producer.send(record, 0).await;
}