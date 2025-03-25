use std::{
    time::Duration,
    collections::HashMap,
    sync::{Arc, Mutex},
};
use axum::{
    extract::State,
    http::{Request, StatusCode, HeaderMap},
    response::Response,
    middleware::Next,
};

/// Authentication middleware to validate bearer tokens
pub async fn auth(
    headers: HeaderMap,
    req: Request<axum::body::Body>,
    next: Next,
) -> Result<Response, StatusCode> {
    // Get the Authorization header
    let auth_header = headers.get("Authorization")
        .and_then(|value| value.to_str().ok())
        .and_then(|value| {
            if value.starts_with("Bearer ") {
                Some(value[7..].to_string())
            } else {
                None
            }
        });
    
    // Check if token exists and matches
    match auth_header {
        Some(token) if token == "secret-api-token" => Ok(next.run(req).await),
        _ => Err(StatusCode::UNAUTHORIZED)
    }
}

/// Simple rate limiter based on client IP
#[derive(Clone)]
pub struct RateLimiter {
    requests: Arc<Mutex<HashMap<String, (usize, std::time::Instant)>>>,
    max_requests: usize,
    window: Duration,
}

impl RateLimiter {
    /// Create a new rate limiter
    pub fn new(max_requests: usize, window: Duration) -> Self {
        Self {
            requests: Arc::new(Mutex::new(HashMap::new())),
            max_requests,
            window,
        }
    }

    /// Check if a request should be rate limited
    pub fn check(&self, client_ip: &str) -> bool {
        let now = std::time::Instant::now();
        let mut requests = self.requests.lock().unwrap();
        
        if let Some((count, timestamp)) = requests.get_mut(client_ip) {
            if now.duration_since(*timestamp) > self.window {
                // Reset if window has passed
                *count = 1;
                *timestamp = now;
                true
            } else if *count >= self.max_requests {
                // Rate limit exceeded
                false
            } else {
                // Increment count
                *count += 1;
                true
            }
        } else {
            // First request from this client
            requests.insert(client_ip.to_string(), (1, now));
            true
        }
    }
}

/// Rate limiting middleware with state
pub async fn rate_limit(
    State(limiter): State<RateLimiter>,
    req: Request<axum::body::Body>,
    next: Next,
) -> Result<Response, StatusCode> {
    // In a real implementation, get client IP from request
    // For demo purposes, we'll use a hardcoded value
    let client_ip = "127.0.0.1";
    
    if !limiter.check(client_ip) {
        return Err(StatusCode::TOO_MANY_REQUESTS);
    }
    
    Ok(next.run(req).await)
}

/// Logging middleware
pub async fn request_logger(
    req: Request<axum::body::Body>,
    next: Next,
) -> Response {
    let path = req.uri().path().to_owned();
    let method = req.method().clone();
    
    let start = std::time::Instant::now();
    let response = next.run(req).await;
    let latency = start.elapsed();
    
    tracing::info!(
        method = %method,
        path = %path,
        status = %response.status().as_u16(),
        latency = ?latency,
        "Request"
    );
    
    response
} 