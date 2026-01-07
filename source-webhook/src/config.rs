//! Configuration module for the HTTP/Webhook source connector.
//!
//! This module handles loading and validating connector configuration from TOML files
//! with environment variable overrides for secrets.

use anyhow::{Context, Result};
use serde::{Deserialize, Serialize};
use std::env;
use std::fs;
use std::path::Path;

/// Root configuration for the webhook source connector
#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct WebhookSourceConfig {
    /// Core Danube connection settings
    pub core: CoreConfig,
    /// HTTP server settings
    pub server: ServerConfig,
    /// Platform-wide authentication (applies to all endpoints)
    pub auth: AuthConfig,
    /// Optional platform-wide rate limiting
    #[serde(default)]
    pub rate_limit: Option<RateLimitConfig>,
    /// Endpoint definitions (multiple endpoints for different event types)
    pub endpoints: Vec<EndpointConfig>,
}

/// Core Danube connection configuration
#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct CoreConfig {
    /// Danube broker service URL
    pub danube_service_url: String,
    /// Unique connector name
    pub connector_name: String,
    /// Optional metrics port (default: 9090)
    #[serde(default = "default_metrics_port")]
    pub metrics_port: u16,
}

fn default_metrics_port() -> u16 {
    9090
}

/// HTTP server configuration
#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct ServerConfig {
    /// Host to bind to (default: 0.0.0.0)
    #[serde(default = "default_host")]
    pub host: String,
    /// Port to listen on (default: 8080)
    #[serde(default = "default_port")]
    pub port: u16,
    /// Optional TLS certificate path
    pub tls_cert_path: Option<String>,
    /// Optional TLS key path
    pub tls_key_path: Option<String>,
    /// Request timeout in seconds (default: 30)
    #[serde(default = "default_timeout")]
    pub timeout_seconds: u64,
    /// Maximum request body size in bytes (default: 1MB)
    #[serde(default = "default_max_body_size")]
    pub max_body_size: usize,
}

fn default_host() -> String {
    "0.0.0.0".to_string()
}

fn default_port() -> u16 {
    8080
}

fn default_timeout() -> u64 {
    30
}

fn default_max_body_size() -> usize {
    1024 * 1024 // 1MB
}

/// Authentication configuration (platform-wide)
#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct AuthConfig {
    /// Authentication type
    #[serde(rename = "type")]
    pub auth_type: AuthType,
    /// Environment variable containing the secret (for HMAC, API key, JWT)
    pub secret_env: Option<String>,
    /// Header name to check (for HMAC, API key)
    pub header: Option<String>,
    /// Algorithm for HMAC (sha256, sha512)
    pub algorithm: Option<String>,
    /// Public key path for JWT verification
    pub public_key_path: Option<String>,
}

/// Authentication type
#[derive(Debug, Clone, Deserialize, Serialize, PartialEq, Eq)]
#[serde(rename_all = "lowercase")]
pub enum AuthType {
    /// No authentication
    None,
    /// API key in header
    ApiKey,
    /// HMAC signature verification
    Hmac,
    /// JWT token verification
    Jwt,
}

/// Rate limiting configuration
#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct RateLimitConfig {
    /// Requests per second
    pub requests_per_second: u32,
    /// Burst size (max requests in burst)
    pub burst_size: u32,
    /// Enable per-IP rate limiting
    #[serde(default)]
    pub per_ip_enabled: bool,
    /// Per-IP requests per second (if per_ip_enabled)
    pub per_ip_requests_per_second: Option<u32>,
}

/// Endpoint configuration
#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct EndpointConfig {
    /// HTTP path for this endpoint (e.g., "/webhooks/payments")
    pub path: String,
    /// Danube topic to publish to
    pub danube_topic: String,
    /// Number of partitions for the topic (0 or omitted = non-partitioned)
    #[serde(default)]
    pub partitions: u32,
    /// Reliable dispatch for this endpoint (default: false)
    #[serde(default)]
    pub reliable_dispatch: bool,
    /// Optional per-endpoint rate limiting (overrides platform-wide)
    pub rate_limit: Option<RateLimitConfig>,
}

impl WebhookSourceConfig {
    /// Load configuration from a TOML file
    pub fn from_file<P: AsRef<Path>>(path: P) -> Result<Self> {
        let content = fs::read_to_string(path.as_ref())
            .with_context(|| format!("Failed to read config file: {:?}", path.as_ref()))?;

        let mut config: WebhookSourceConfig = toml::from_str(&content)
            .with_context(|| "Failed to parse TOML configuration")?;

        // Apply environment variable overrides
        config.apply_env_overrides()?;

        // Validate configuration
        config.validate()?;

        Ok(config)
    }

    /// Apply environment variable overrides
    fn apply_env_overrides(&mut self) -> Result<()> {
        // Override Danube service URL
        if let Ok(url) = env::var("DANUBE_SERVICE_URL") {
            tracing::info!("Overriding danube_service_url from environment");
            self.core.danube_service_url = url;
        }

        // Override connector name
        if let Ok(name) = env::var("CONNECTOR_NAME") {
            tracing::info!("Overriding connector_name from environment");
            self.core.connector_name = name;
        }

        // Override server host
        if let Ok(host) = env::var("SERVER_HOST") {
            tracing::info!("Overriding server host from environment");
            self.server.host = host;
        }

        // Override server port
        if let Ok(port) = env::var("SERVER_PORT") {
            let port: u16 = port.parse()
                .with_context(|| "Invalid SERVER_PORT value")?;
            tracing::info!("Overriding server port from environment: {}", port);
            self.server.port = port;
        }

        Ok(())
    }

    /// Validate configuration
    pub fn validate(&self) -> Result<()> {
        // Validate Danube URL
        if self.core.danube_service_url.is_empty() {
            anyhow::bail!("danube_service_url cannot be empty");
        }

        // Validate connector name
        if self.core.connector_name.is_empty() {
            anyhow::bail!("connector_name cannot be empty");
        }

        // Validate endpoints
        if self.endpoints.is_empty() {
            anyhow::bail!("At least one endpoint must be configured");
        }

        // Validate endpoint paths are unique
        let mut paths = std::collections::HashSet::new();
        for endpoint in &self.endpoints {
            if !paths.insert(&endpoint.path) {
                anyhow::bail!("Duplicate endpoint path: {}", endpoint.path);
            }

            // Validate path starts with /
            if !endpoint.path.starts_with('/') {
                anyhow::bail!("Endpoint path must start with '/': {}", endpoint.path);
            }

            // Validate topic is not empty
            if endpoint.danube_topic.is_empty() {
                anyhow::bail!("danube_topic cannot be empty for endpoint: {}", endpoint.path);
            }
        }

        // Validate authentication configuration
        self.validate_auth()?;

        Ok(())
    }

    /// Validate authentication configuration
    fn validate_auth(&self) -> Result<()> {
        match self.auth.auth_type {
            AuthType::None => {
                // No validation needed
            }
            AuthType::ApiKey => {
                if self.auth.secret_env.is_none() {
                    anyhow::bail!("secret_env is required for API key authentication");
                }
                if self.auth.header.is_none() {
                    anyhow::bail!("header is required for API key authentication");
                }
            }
            AuthType::Hmac => {
                if self.auth.secret_env.is_none() {
                    anyhow::bail!("secret_env is required for HMAC authentication");
                }
                if self.auth.header.is_none() {
                    anyhow::bail!("header is required for HMAC authentication");
                }
                if self.auth.algorithm.is_none() {
                    anyhow::bail!("algorithm is required for HMAC authentication");
                }
            }
            AuthType::Jwt => {
                if self.auth.secret_env.is_none() && self.auth.public_key_path.is_none() {
                    anyhow::bail!("Either secret_env or public_key_path is required for JWT authentication");
                }
            }
        }

        Ok(())
    }

    /// Get authentication secret from environment variable
    #[allow(dead_code)]
    pub fn get_auth_secret(&self) -> Result<Option<String>> {
        if let Some(env_var) = &self.auth.secret_env {
            let secret = env::var(env_var)
                .with_context(|| format!("Authentication secret not found in environment variable: {}", env_var))?;
            
            if secret.is_empty() {
                anyhow::bail!("Authentication secret is empty in environment variable: {}", env_var);
            }
            
            Ok(Some(secret))
        } else {
            Ok(None)
        }
    }

    /// Get server bind address
    pub fn bind_address(&self) -> String {
        format!("{}:{}", self.server.host, self.server.port)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_default_values() {
        let config = CoreConfig {
            danube_service_url: "http://localhost:6650".to_string(),
            connector_name: "test".to_string(),
            metrics_port: default_metrics_port(),
        };
        assert_eq!(config.metrics_port, 9090);

        let server = ServerConfig {
            host: default_host(),
            port: default_port(),
            tls_cert_path: None,
            tls_key_path: None,
            timeout_seconds: default_timeout(),
            max_body_size: default_max_body_size(),
        };
        assert_eq!(server.host, "0.0.0.0");
        assert_eq!(server.port, 8080);
        assert_eq!(server.timeout_seconds, 30);
        assert_eq!(server.max_body_size, 1024 * 1024);
    }

    #[test]
    fn test_auth_type_deserialization() {
        let json = r#"{"type": "none"}"#;
        let auth: AuthConfig = serde_json::from_str(json).unwrap();
        assert_eq!(auth.auth_type, AuthType::None);

        let json = r#"{"type": "hmac"}"#;
        let auth: AuthConfig = serde_json::from_str(json).unwrap();
        assert_eq!(auth.auth_type, AuthType::Hmac);
    }
}
