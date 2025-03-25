use std::{
    future::Future,
    pin::Pin,
    task::{Context, Poll},
    time::Duration,
    collections::HashMap,
    sync::{Arc, Mutex},
};
use axum::{
    http::{Request, StatusCode},
    middleware::Next,
    response::Response,
    body::Body,
    extract::FromRequestParts,
    headers::{authorization::Bearer, Authorization},
    TypedHeader,
};

/// Authentication middleware to validate bearer tokens
pub async fn auth<B>(
    TypedHeader(auth): TypedHeader<Authorization<Bearer>>,
    req: Request<B>,
    next: Next<B>,
) -> Result<Response, StatusCode> {
    // In a real implementation, this should validate tokens against a database or auth service
    // For now, we'll just check against a hardcoded token for demonstration
    if auth.token() != "secret-api-token" {
        return Err(StatusCode::UNAUTHORIZED);
    }

    Ok(next.run(req).await)
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

/// Rate limiting middleware
pub async fn rate_limit(
    req: Request<Body>,
    next: Next<Body>,
    limiter: RateLimiter,
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
pub async fn request_logger<B>(req: Request<B>, next: Next<B>) -> Response {
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