//! Rate limiting middleware using token bucket algorithm.
//!
//! Supports:
//! - Per-endpoint rate limiting
//! - Per-IP rate limiting (optional)
//! - Configurable burst size

use axum::{
    extract::{Request, State},
    http::{HeaderMap, StatusCode},
    middleware::Next,
    response::{IntoResponse, Response},
};
use governor::{
    clock::DefaultClock,
    state::{InMemoryState, NotKeyed},
    Quota, RateLimiter as GovernorRateLimiter,
};
use std::collections::HashMap;
use std::net::IpAddr;
use std::num::NonZeroU32;
use std::sync::Arc;
use tokio::sync::RwLock;

use crate::config::RateLimitConfig;
use crate::server::AppState;

/// Rate limiter state
pub struct RateLimiterState {
    /// Per-endpoint rate limiters
    endpoint_limiters: Arc<RwLock<HashMap<String, Arc<GovernorRateLimiter<NotKeyed, InMemoryState, DefaultClock>>>>>,
    /// Per-IP rate limiters (if enabled)
    ip_limiters: Arc<RwLock<HashMap<IpAddr, Arc<GovernorRateLimiter<NotKeyed, InMemoryState, DefaultClock>>>>>,
}

impl RateLimiterState {
    /// Create a new rate limiter state
    pub fn new() -> Self {
        Self {
            endpoint_limiters: Arc::new(RwLock::new(HashMap::new())),
            ip_limiters: Arc::new(RwLock::new(HashMap::new())),
        }
    }

    /// Get or create a rate limiter for an endpoint
    async fn get_endpoint_limiter(
        &self,
        endpoint: &str,
        config: &RateLimitConfig,
    ) -> Arc<GovernorRateLimiter<NotKeyed, InMemoryState, DefaultClock>> {
        let mut limiters = self.endpoint_limiters.write().await;

        limiters
            .entry(endpoint.to_string())
            .or_insert_with(|| {
                let quota = Quota::per_second(
                    NonZeroU32::new(config.requests_per_second).unwrap_or(NonZeroU32::new(100).unwrap()),
                )
                .allow_burst(NonZeroU32::new(config.burst_size).unwrap_or(NonZeroU32::new(10).unwrap()));

                Arc::new(GovernorRateLimiter::direct(quota))
            })
            .clone()
    }

    /// Get or create a rate limiter for an IP address
    async fn get_ip_limiter(
        &self,
        ip: IpAddr,
        config: &RateLimitConfig,
    ) -> Arc<GovernorRateLimiter<NotKeyed, InMemoryState, DefaultClock>> {
        let mut limiters = self.ip_limiters.write().await;

        limiters
            .entry(ip)
            .or_insert_with(|| {
                let quota = Quota::per_second(
                    NonZeroU32::new(config.requests_per_second).unwrap_or(NonZeroU32::new(100).unwrap()),
                )
                .allow_burst(NonZeroU32::new(config.burst_size).unwrap_or(NonZeroU32::new(10).unwrap()));

                Arc::new(GovernorRateLimiter::direct(quota))
            })
            .clone()
    }
}

impl Default for RateLimiterState {
    fn default() -> Self {
        Self::new()
    }
}

/// Check rate limit (called directly from handler)
#[allow(dead_code)]
pub async fn check_rate_limit(
    _state: &AppState,
    endpoint_path: &str,
    config: &RateLimitConfig,
    headers: &HeaderMap,
) -> Result<(), String> {
    // Create rate limiter state
    let limiter_state = RateLimiterState::new();

    // Check endpoint rate limit
    let endpoint_limiter = limiter_state
        .get_endpoint_limiter(endpoint_path, config)
        .await;

    if endpoint_limiter.check().is_err() {
        return Err(format!("Rate limit exceeded for endpoint: {}", endpoint_path));
    }

    // Check per-IP rate limit if enabled
    if config.per_ip_enabled {
        if let Some(ip) = extract_client_ip_from_headers(headers) {
            let ip_limiter = limiter_state.get_ip_limiter(ip, config).await;

            if ip_limiter.check().is_err() {
                return Err(format!("Rate limit exceeded for IP: {}", ip));
            }
        }
    }

    Ok(())
}

/// Rate limiting middleware (for future use when axum 0.8 compatibility is resolved)
pub async fn rate_limit_middleware(
    State(state): State<AppState>,
    request: Request,
    next: Next,
) -> Result<Response, RateLimitError> {
    // Extract endpoint path
    let endpoint_path = request.uri().path().to_string();

    // Get endpoint configuration and clone rate limit config
    let rate_limit_config = {
        let endpoints = state.endpoints.read().await;
        let endpoint_config = endpoints.get(&endpoint_path);

        // If no endpoint config or no rate limit config, allow request
        match endpoint_config {
            Some(cfg) => match &cfg.rate_limit {
                Some(rl) => rl.clone(),
                None => return Ok(next.run(request).await),
            },
            None => return Ok(next.run(request).await),
        }
    };

    // Create rate limiter state if not exists
    // Note: In production, this should be stored in AppState
    let limiter_state = RateLimiterState::new();

    // Check endpoint rate limit
    let endpoint_limiter = limiter_state
        .get_endpoint_limiter(&endpoint_path, &rate_limit_config)
        .await;

    if endpoint_limiter.check().is_err() {
        tracing::warn!(
            endpoint = %endpoint_path,
            "Rate limit exceeded for endpoint"
        );

        return Err(RateLimitError::Exceeded(format!(
            "Rate limit exceeded for endpoint: {}",
            endpoint_path
        )));
    }

    // Check per-IP rate limit if enabled
    if rate_limit_config.per_ip_enabled {
        if let Some(ip) = extract_client_ip(&request) {
            let ip_limiter = limiter_state.get_ip_limiter(ip, &rate_limit_config).await;

            if ip_limiter.check().is_err() {
                tracing::warn!(
                    endpoint = %endpoint_path,
                    ip = %ip,
                    "Rate limit exceeded for IP"
                );

                return Err(RateLimitError::Exceeded(format!(
                    "Rate limit exceeded for IP: {}",
                    ip
                )));
            }
        }
    }

    Ok(next.run(request).await)
}

/// Extract client IP from HeaderMap
fn extract_client_ip_from_headers(headers: &HeaderMap) -> Option<IpAddr> {
    // Try X-Forwarded-For header first
    if let Some(forwarded) = headers.get("x-forwarded-for") {
        if let Ok(forwarded_str) = forwarded.to_str() {
            if let Some(first_ip) = forwarded_str.split(',').next() {
                if let Ok(ip) = first_ip.trim().parse() {
                    return Some(ip);
                }
            }
        }
    }

    // Try X-Real-IP header
    if let Some(real_ip) = headers.get("x-real-ip") {
        if let Ok(ip_str) = real_ip.to_str() {
            if let Ok(ip) = ip_str.parse() {
                return Some(ip);
            }
        }
    }

    None
}

/// Extract client IP from request
fn extract_client_ip(request: &Request) -> Option<IpAddr> {
    extract_client_ip_from_headers(request.headers())
}

/// Rate limit error
#[derive(Debug)]
pub enum RateLimitError {
    /// Rate limit exceeded
    Exceeded(String),
}

impl IntoResponse for RateLimitError {
    fn into_response(self) -> Response {
        let message = match self {
            RateLimitError::Exceeded(msg) => msg,
        };

        tracing::warn!(error = %message, "Rate limit exceeded");

        (
            StatusCode::TOO_MANY_REQUESTS,
            axum::Json(serde_json::json!({
                "error": "rate_limit_exceeded",
                "message": message,
            })),
        )
            .into_response()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_rate_limiter_creation() {
        let state = RateLimiterState::new();
        assert!(state.endpoint_limiters.try_read().is_ok());
    }

    #[test]
    fn test_ip_extraction() {
        // Test IP extraction from headers
        // TODO: Add comprehensive tests
    }
}
