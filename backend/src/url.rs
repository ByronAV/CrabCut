
use actix_web::{get, post, Responder, HttpResponse, web::Json, FromRequest};
use serde::Deserialize;
use serde_json;
use sha2::{Digest, Sha256};
use base64::{engine::general_purpose::URL_SAFE_NO_PAD, Engine as _};
use sqlx::PgPool;

#[derive(Deserialize)]
struct ShortenRequest {
    long_url: String,
    custom_alias: Option<String>,
}

#[get("/health")]
pub async fn health_check() -> impl Responder {
    HttpResponse::Ok().body("OK")
}

#[post("/create")]
pub async fn create_short_url(req: Json<ShortenRequest>) -> impl Responder {

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

        return HttpResponse::Ok().json(serde_json::json!({
            "short_url": format!("https://crabcut.io/{}", short_url)
        }));
    } else {
        // We have an alias, we will use it
        if !validate_alias(alias) {
            return HttpResponse::BadRequest().body("Invalid alias");
        }
        // Check if the alias is unique
        if !is_alias_unique(alias) {
            return HttpResponse::Conflict().body("Alias already exists");
        }
        // If the alias is valid and unique, we can create the short URL
        return HttpResponse::Ok().json(serde_json::json!({
            "short_url": format!("https://crabcut.io/{}", alias)
        }));
    }
}

#[get("/{short_url}")]
pub async fn get_short_url() -> impl Responder {
    HttpResponse::Ok().body("OK")
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

fn is_alias_unique(alias: &str) -> bool {
    // Check the postgres database to see if the alias is unique
    // For now, we will just return true
    true
}

async fn save_short_url_to_db(db_pool: &PgPool, long_url: &str, short_url: &str, user_id: Option<uuid::Uuid>) -> Result<(), String> {
    return sqlx::query!(
        r#"
        INSERT INTO urls (long_url, short_url)
        VALUES ($1, $2)
        ON CONFLICT (short_url) DO NOTHING
        "#,
        long_url, short_url
    )
    .execute(db_pool)
}