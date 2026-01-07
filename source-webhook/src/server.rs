//! HTTP server implementation using axum framework.

use axum::{
    body::Bytes,
    extract::{Path, State},
    http::{HeaderMap, StatusCode},
    middleware,
    response::{IntoResponse, Response},
    routing::{get, post},
    Json, Router,
};
use serde_json::json;
use std::collections::HashMap;
use std::net::SocketAddr;
use std::sync::Arc;
use tower_http::{timeout::TimeoutLayer, trace::TraceLayer};

use crate::auth;
use crate::config::{EndpointConfig, WebhookSourceConfig};
use crate::connector::WebhookConnector;
use crate::rate_limit;
use danube_connect_core::SourceRecord;
use tokio::sync::mpsc::Sender;
use tokio::sync::RwLock;

/// Shared application state
#[derive(Clone)]
pub struct AppState {
    pub config: WebhookSourceConfig,
    pub endpoints: Arc<RwLock<HashMap<String, EndpointConfig>>>,
    pub message_tx: Sender<SourceRecord>,
}

/// Start the HTTP server with state components (called from connector initialize)
pub async fn start_server_with_state(
    config: WebhookSourceConfig,
    endpoints: Arc<RwLock<HashMap<String, EndpointConfig>>>,
    message_tx: Sender<SourceRecord>,
) -> anyhow::Result<()> {
    let bind_addr: SocketAddr = config.bind_address().parse()?;

    // Create application state
    let state = AppState {
        config: config.clone(),
        endpoints,
        message_tx,
    };

    // Build webhook handler with auth and rate limiting middleware
    let webhook_handler_with_middleware = post(webhook_handler)
        .layer(middleware::from_fn_with_state(
            state.clone(),
            rate_limit::rate_limit_middleware,
        ))
        .layer(middleware::from_fn_with_state(
            state.clone(),
            auth::auth_middleware,
        ));

    // Build main router
    let app = Router::new()
        // Health endpoints (no auth/rate limiting)
        .route("/health", get(health_handler))
        .route("/ready", get(readiness_handler))
        // Webhook endpoint with auth and rate limiting middleware
        .route("/{*path}", webhook_handler_with_middleware)
        // Add global middleware
        .layer(TimeoutLayer::with_status_code(
            StatusCode::REQUEST_TIMEOUT,
            std::time::Duration::from_secs(config.server.timeout_seconds),
        ))
        .layer(TraceLayer::new_for_http())
        .with_state(state);

    tracing::info!("Starting HTTP server on {}", bind_addr);

    // Start server
    let listener = tokio::net::TcpListener::bind(bind_addr).await?;

    axum::serve(listener, app)
        .await
        .map_err(|e| anyhow::anyhow!("Server error: {}", e))?;

    Ok(())
}

/// Webhook handler - processes incoming webhooks
async fn webhook_handler(
    State(state): State<AppState>,
    Path(path): Path<String>,
    headers: HeaderMap,
    body: Bytes,
) -> Result<Response, AppError> {
    let endpoint_path = format!("/{}", path);

    tracing::debug!(
        endpoint = %endpoint_path,
        body_size = body.len(),
        "Received webhook request"
    );

    // Check if endpoint exists
    let endpoints = state.endpoints.read().await;
    let endpoint_config = endpoints
        .get(&endpoint_path)
        .ok_or_else(|| AppError::NotFound(format!("Endpoint not found: {}", endpoint_path)))?
        .clone();
    drop(endpoints);

    // Extract headers as HashMap
    let header_map = extract_headers(&headers);

    // Extract client IP
    let client_ip = extract_client_ip(&headers);

    // Validate content type
    if let Some(content_type) = header_map.get("content-type") {
        if !content_type.contains("application/json") {
            tracing::warn!(
                endpoint = %endpoint_path,
                content_type = %content_type,
                "Non-JSON content type received"
            );
        }
    }

    // Check body size
    let max_size = state.config.server.max_body_size;
    if body.len() > max_size {
        return Err(AppError::PayloadTooLarge(format!(
            "Payload size {} exceeds maximum {}",
            body.len(),
            max_size
        )));
    }

    // Create SourceRecord from webhook data
    let source_record = WebhookConnector::create_source_record(
        &endpoint_config,
        &state.config.core.connector_name,
        &endpoint_path,
        body.to_vec(),
        &header_map,
        client_ip.as_deref(),
    );

    // Send to channel for processing by runtime
    state.message_tx.send(source_record).await.map_err(|e| {
        tracing::error!(
            endpoint = %endpoint_path,
            error = ?e,
            "Failed to send webhook to channel"
        );
        AppError::Internal("Failed to queue webhook for processing".to_string())
    })?;

    // Return success
    Ok((
        StatusCode::OK,
        Json(json!({
            "status": "accepted",
            "endpoint": endpoint_path,
            "topic": endpoint_config.danube_topic,
        })),
    )
        .into_response())
}

/// Health check handler - always returns OK
async fn health_handler() -> impl IntoResponse {
    (
        StatusCode::OK,
        Json(json!({
            "status": "healthy",
            "timestamp": chrono::Utc::now().to_rfc3339(),
        })),
    )
}

/// Readiness check handler - always ready if server is running
async fn readiness_handler() -> impl IntoResponse {
    (
        StatusCode::OK,
        Json(json!({
            "status": "ready",
            "timestamp": chrono::Utc::now().to_rfc3339(),
        })),
    )
}

/// Extract headers as HashMap
fn extract_headers(headers: &HeaderMap) -> HashMap<String, String> {
    let mut map = HashMap::new();
    for (key, value) in headers.iter() {
        if let Ok(value_str) = value.to_str() {
            map.insert(key.as_str().to_lowercase(), value_str.to_string());
        }
    }
    map
}

/// Extract client IP from headers
fn extract_client_ip(headers: &HeaderMap) -> Option<String> {
    // Try X-Forwarded-For first
    if let Some(forwarded) = headers.get("x-forwarded-for") {
        if let Ok(value) = forwarded.to_str() {
            // Take the first IP in the list
            if let Some(ip) = value.split(',').next() {
                return Some(ip.trim().to_string());
            }
        }
    }

    // Try X-Real-IP
    if let Some(real_ip) = headers.get("x-real-ip") {
        if let Ok(value) = real_ip.to_str() {
            return Some(value.to_string());
        }
    }

    None
}

/// Application errors
#[derive(Debug)]
#[allow(dead_code)]
pub enum AppError {
    NotFound(String),
    BadRequest(String),
    Unauthorized(String),
    PayloadTooLarge(String),
    TooManyRequests(String),
    Internal(String),
    ServiceUnavailable(String),
}

impl IntoResponse for AppError {
    fn into_response(self) -> Response {
        let (status, message) = match self {
            AppError::NotFound(msg) => (StatusCode::NOT_FOUND, msg),
            AppError::BadRequest(msg) => (StatusCode::BAD_REQUEST, msg),
            AppError::Unauthorized(msg) => (StatusCode::UNAUTHORIZED, msg),
            AppError::PayloadTooLarge(msg) => (StatusCode::PAYLOAD_TOO_LARGE, msg),
            AppError::TooManyRequests(msg) => (StatusCode::TOO_MANY_REQUESTS, msg),
            AppError::Internal(msg) => (StatusCode::INTERNAL_SERVER_ERROR, msg),
            AppError::ServiceUnavailable(msg) => (StatusCode::SERVICE_UNAVAILABLE, msg),
        };

        (
            status,
            Json(json!({
                "error": message,
                "timestamp": chrono::Utc::now().to_rfc3339(),
            })),
        )
            .into_response()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_extract_client_ip() {
        let mut headers = HeaderMap::new();

        // Test X-Forwarded-For
        headers.insert("x-forwarded-for", "192.168.1.1, 10.0.0.1".parse().unwrap());
        let ip = extract_client_ip(&headers);
        assert_eq!(ip, Some("192.168.1.1".to_string()));

        // Test X-Real-IP
        let mut headers = HeaderMap::new();
        headers.insert("x-real-ip", "192.168.1.2".parse().unwrap());
        let ip = extract_client_ip(&headers);
        assert_eq!(ip, Some("192.168.1.2".to_string()));
    }
}
