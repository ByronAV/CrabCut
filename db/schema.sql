CREATE TABLE IF NOT EXISTS urls (
    short_url VARCHAR(16) PRIMARY KEY,
    long_url TEXT NOT NULL,
    click_count BIGINT DEFAULT 0,
    created_at TIMESTAMP NOT NULL DEFAULT NOW(),
);

CREATE TABLE IF NOT EXISTS clicks (
    id SERIAL PRIMARY KEY,
    short_url VARCHAR(16) NOT NULL REFERENCES urls(short_url) ON DELETE CASCADE,
    clicked_at TIMESTAMP NOT NULL DEFAULT NOW(),
    ip_address INET,
    user_agent TEXT,
    referrer TEXT,
);