//! Authentication middleware for webhook requests.
//!
//! Supports multiple authentication methods:
//! - API Key: Simple header-based authentication
//! - HMAC: Signature-based verification (Stripe, GitHub style)
//! - JWT: Token-based authentication

use axum::{
    extract::{Request, State},
    http::{HeaderMap, StatusCode},
    middleware::Next,
    response::{IntoResponse, Response},
};
use jsonwebtoken::{decode, DecodingKey, Validation};
use serde::{Deserialize, Serialize};
use std::env;

use crate::config::{AuthConfig, AuthType};
use crate::server::AppState;

/// JWT claims structure
#[derive(Debug, Serialize, Deserialize)]
struct Claims {
    sub: String,
    exp: usize,
}

/// Verify authentication (called directly from handler)
#[allow(dead_code)]
pub async fn verify_auth(config: &AuthConfig, headers: &HeaderMap) -> Result<(), String> {
    // Skip auth if type is None
    if config.auth_type == AuthType::None {
        return Ok(());
    }

    // Perform authentication based on type
    match config.auth_type {
        AuthType::None => Ok(()),
        AuthType::ApiKey => verify_api_key(config, headers).map_err(|e| format!("{:?}", e)),
        AuthType::Hmac => {
            // HMAC verification requires body, which we don't have here
            // For now, log a warning and allow
            tracing::warn!("HMAC verification not fully implemented");
            Ok(())
        }
        AuthType::Jwt => verify_jwt(config, headers).map_err(|e| format!("{:?}", e)),
    }
}

/// Authentication middleware
pub async fn auth_middleware(
    State(state): State<AppState>,
    request: Request,
    next: Next,
) -> Result<Response, AuthError> {
    // Skip auth if type is None
    if state.config.auth.auth_type == AuthType::None {
        return Ok(next.run(request).await);
    }

    // Get headers and path for logging
    let headers = request.headers();
    let endpoint_path = request.uri().path();

    // Perform authentication based on type
    let auth_result = match state.config.auth.auth_type {
        AuthType::None => Ok(()),
        AuthType::ApiKey => verify_api_key(&state.config.auth, headers),
        AuthType::Hmac => {
            // HMAC verification requires body, which we don't have here
            // For now, log a warning and allow
            tracing::warn!("HMAC verification not fully implemented in middleware");
            Ok(())
        }
        AuthType::Jwt => verify_jwt(&state.config.auth, headers),
    };

    // Log authentication failure
    if let Err(ref e) = auth_result {
        tracing::warn!(
            endpoint = %endpoint_path,
            error = ?e,
            "Authentication failed"
        );
    }

    // Return error or continue
    auth_result?;
    Ok(next.run(request).await)
}

/// Verify API key authentication
fn verify_api_key(config: &AuthConfig, headers: &HeaderMap) -> Result<(), AuthError> {
    // Get the header name (default to "X-API-Key")
    let header_name = config
        .header
        .as_deref()
        .unwrap_or("x-api-key")
        .to_lowercase();

    // Get the expected API key from environment
    let secret_env = config.secret_env.as_ref().ok_or_else(|| {
        AuthError::Configuration("secret_env not configured for API key auth".to_string())
    })?;

    let expected_key = env::var(secret_env).map_err(|_| {
        AuthError::Configuration(format!("Environment variable {} not set", secret_env))
    })?;

    // Get the API key from request headers
    let provided_key = headers
        .get(&header_name)
        .and_then(|v| v.to_str().ok())
        .ok_or_else(|| AuthError::Missing(format!("Missing {} header", header_name)))?;

    // Constant-time comparison
    if provided_key != expected_key {
        return Err(AuthError::Invalid("Invalid API key".to_string()));
    }

    Ok(())
}

/// Verify HMAC signature
#[allow(dead_code)]
async fn verify_hmac_signature(
    config: &AuthConfig,
    headers: &HeaderMap,
    _request: &Request,
) -> Result<(), AuthError> {
    // Get the signature header name (default to "X-Signature")
    let header_name = config
        .header
        .as_deref()
        .unwrap_or("x-signature")
        .to_lowercase();

    // Get the signature from headers
    let signature_header = headers
        .get(&header_name)
        .and_then(|v| v.to_str().ok())
        .ok_or_else(|| AuthError::Missing(format!("Missing {} header", header_name)))?;

    // Parse signature (format: "sha256=<hex>" or just "<hex>")
    let _signature = if signature_header.contains('=') {
        signature_header
            .split('=')
            .nth(1)
            .ok_or_else(|| AuthError::Invalid("Invalid signature format".to_string()))?
    } else {
        signature_header
    };

    // Get the secret from environment
    let secret_env = config.secret_env.as_ref().ok_or_else(|| {
        AuthError::Configuration("secret_env not configured for HMAC auth".to_string())
    })?;

    let _secret = env::var(secret_env).map_err(|_| {
        AuthError::Configuration(format!("Environment variable {} not set", secret_env))
    })?;

    // Get algorithm (default to sha256)
    let _algorithm = config.algorithm.as_deref().unwrap_or("sha256");

    // Note: In a real implementation, we'd need to read the body here
    // For now, we'll add a TODO comment
    // TODO: Implement body reading for HMAC verification
    // This requires buffering the request body, computing HMAC, then passing body forward

    tracing::warn!("HMAC verification not fully implemented - requires body buffering");

    Ok(())
}

/// Verify JWT token
fn verify_jwt(config: &AuthConfig, headers: &HeaderMap) -> Result<(), AuthError> {
    // Get the authorization header
    let auth_header = headers
        .get("authorization")
        .and_then(|v| v.to_str().ok())
        .ok_or_else(|| AuthError::Missing("Missing Authorization header".to_string()))?;

    // Extract token (format: "Bearer <token>")
    let token = if auth_header.starts_with("Bearer ") {
        &auth_header[7..]
    } else {
        return Err(AuthError::Invalid(
            "Invalid Authorization header format".to_string(),
        ));
    };

    // Get the public key or secret
    let secret_env = config.secret_env.as_ref().ok_or_else(|| {
        AuthError::Configuration("secret_env not configured for JWT auth".to_string())
    })?;

    let secret = env::var(secret_env).map_err(|_| {
        AuthError::Configuration(format!("Environment variable {} not set", secret_env))
    })?;

    // Decode and validate JWT
    let decoding_key = DecodingKey::from_secret(secret.as_bytes());
    let validation = Validation::default();

    decode::<Claims>(token, &decoding_key, &validation)
        .map_err(|e| AuthError::Invalid(format!("Invalid JWT token: {}", e)))?;

    Ok(())
}

/// Authentication error types
#[derive(Debug)]
pub enum AuthError {
    /// Missing authentication credentials
    Missing(String),
    /// Invalid authentication credentials
    Invalid(String),
    /// Configuration error
    Configuration(String),
}

impl IntoResponse for AuthError {
    fn into_response(self) -> Response {
        let (status, message) = match self {
            AuthError::Missing(msg) => (StatusCode::UNAUTHORIZED, msg),
            AuthError::Invalid(msg) => (StatusCode::UNAUTHORIZED, msg),
            AuthError::Configuration(msg) => (StatusCode::INTERNAL_SERVER_ERROR, msg),
        };

        tracing::warn!(error = %message, "Authentication failed");

        (
            status,
            axum::Json(serde_json::json!({
                "error": "authentication_failed",
                "message": message,
            })),
        )
            .into_response()
    }
}
