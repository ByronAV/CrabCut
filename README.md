# ⚡ CrabCut: High-Speed URL Shortener API

**CrabCut** is a blazing-fast, scalable URL shortening service designed for performance, reliability, and extensibility. It allows users to convert long URLs into concise, shareable links while collecting usage analytics. Built with a distributed architecture and optimized for high read throughput, CrabCut is ideal for real-time use cases with massive traffic volumes.

---

## Features

- Shorten long URLs to compact links
- Support for custom short links (up to 16 characters)
- Click analytics for tracking most popular URLs
- High throughput and low latency redirections
- Permanent storage of shortened links
- API key-based rate limiting and access control
- Intelligent in-memory caching for popular links

---

## Functional Requirements

- Create short URLs from long URLs
- Redirect users from short URLs to the original long URLs
- Allow custom aliases (max 16 characters)
- Store mappings indefinitely
- Track and expose metrics such as click counts
- Provide a RESTful API interface for external integration

---

## Non-Functional Requirements

- High availability (HA) and fault tolerance
- Low-latency redirection (sub-millisecond goal)
- Horizontally scalable backend and cache layers
- Designed for read-heavy workloads (200:1 read/write ratio)
- API-first design for future third-party integrations

---

## System Capacity Estimates

### Traffic
- **Writes (shortening)**: ~40 requests/sec
- **Reads (redirects)**: ~8000 requests/sec (200:1 read/write ratio)

### Storage
- ~120B URLs over 100 years (~60 TB total)
- Each URL mapping ≈ 500 bytes (includes metadata)

### Memory (Caching)
- 80% of traffic targets 20% of data (Pareto principle)
- Cache ≈ 20% of 700M daily requests = ~70 GB of hot data

---

## High-Level Architecture

![alt text](/public/ShortURL.png)


### Why this architecture?
- Eliminates SPOF (Single Point of Failure)
- Scalable horizontally to handle high RPS (requests per second)
- Efficient caching of frequently accessed short URLs
- Persistent storage for long-term durability

---

## REST API

### Create Short URL

**Endpoint:**

```bash
POST /api/create
```

**Request Body:**
```json
{
  "url": "https://very-long-url.com/page",
  "api_key": "your-api-key",
  "custom_url": "optional-custom-alias"
}
```

**Response:**
```json
{
  "short_url": "https://crabcut.io/xYz123"
}
```

- url: The long URL to shorten

- api_key: Used for rate limiting and abuse protection

- custom_url (optional): Custom alias (≤ 16 characters)

### Redirect to Long URL

**Endpoint:**

```bash
GET /{short_url}
```

**Behavior:**

- Returns HTTP 302 Redirect to the original long URL
- Ensures backend can collect analytics (instead of 301 which is cached by clients)


## Future Work
- Click-through analytics dashboard
- Expiring links (temporary short URLs)
- User account and dashboard
- Admin panel for moderation and abuse reports
- gRPC + WebSocket support for real-time integrations

## License
This project is licensed under the MIT License.

## Contributing
Want to make CrabCut better? Open an issue or submit a pull request! All contributions are welcome.

## Contact
For questions or feature requests, [open an issue](https://github.com/ByronAV/CrabCut/issues) or reach out directly via GitHub.
