//! Message transformation logic for converting Danube messages to Qdrant points

use danube_connect_core::{ConnectorError, ConnectorResult, SinkRecord};
use qdrant_client::qdrant::{PointStruct, Value};
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use std::collections::HashMap;

/// Expected message format from Danube
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct VectorMessage {
    /// Optional point ID (if not provided, will be generated)
    #[serde(skip_serializing_if = "Option::is_none")]
    pub id: Option<String>,

    /// Vector embedding (required)
    pub vector: Vec<f32>,

    /// Optional payload/metadata
    #[serde(skip_serializing_if = "Option::is_none")]
    pub payload: Option<serde_json::Value>,
}

/// Transform a Danube SinkRecord into a Qdrant PointStruct
pub fn transform_to_point(
    record: &SinkRecord,
    expected_dimension: usize,
    include_danube_metadata: bool,
) -> ConnectorResult<PointStruct> {
    // Parse message from typed payload (already serde_json::Value)
    let message: VectorMessage = serde_json::from_value(record.payload().clone()).map_err(|e| {
        ConnectorError::invalid_data(
            format!("Failed to deserialize message: {}", e),
            vec![], // No raw bytes in v0.7.0 - payload is typed
        )
    })?;

    // Validate vector dimension
    if message.vector.len() != expected_dimension {
        return Err(ConnectorError::invalid_data(
            format!(
                "Vector dimension mismatch: expected {}, got {}",
                expected_dimension,
                message.vector.len()
            ),
            vec![], // No raw bytes in v0.7.0 - payload is typed
        ));
    }

    // Generate point ID
    let point_id = generate_point_id(&message, record);

    // Build payload
    let payload = build_payload(message.payload, record, include_danube_metadata)?;

    // Create Qdrant point
    Ok(PointStruct::new(point_id, message.vector, payload))
}

/// Generate a unique point ID
/// Priority: 1) Use message.id if provided, 2) Hash of (topic + offset)
fn generate_point_id(message: &VectorMessage, record: &SinkRecord) -> u64 {
    if let Some(ref id) = message.id {
        // Try to parse as u64
        if let Ok(num_id) = id.parse::<u64>() {
            return num_id;
        }

        // Otherwise hash the string ID
        return hash_string_to_u64(id);
    }

    // Generate ID from topic + offset to ensure uniqueness across topics
    let composite_key = format!("{}:{}", record.topic(), record.offset());
    hash_string_to_u64(&composite_key)
}

/// Hash a string to u64 using SHA256
fn hash_string_to_u64(s: &str) -> u64 {
    let mut hasher = Sha256::new();
    hasher.update(s.as_bytes());
    let result = hasher.finalize();

    // Take first 8 bytes and convert to u64
    u64::from_be_bytes(result[0..8].try_into().unwrap())
}

/// Build Qdrant payload from message and Danube metadata
fn build_payload(
    message_payload: Option<serde_json::Value>,
    record: &SinkRecord,
    include_danube_metadata: bool,
) -> ConnectorResult<HashMap<String, Value>> {
    let mut payload = HashMap::new();

    // Add user payload if present
    if let Some(json_payload) = message_payload {
        // Convert JSON value to Qdrant payload
        add_json_to_payload(&mut payload, "", json_payload);
    }

    // Add Danube metadata if enabled
    if include_danube_metadata {
        payload.insert(
            "_danube_topic".to_string(),
            Value::from(record.topic().to_string()),
        );
        payload.insert(
            "_danube_offset".to_string(),
            Value::from(record.offset() as i64),
        );
        payload.insert(
            "_danube_timestamp".to_string(),
            Value::from(record.publish_time() as i64),
        );
        payload.insert(
            "_danube_producer".to_string(),
            Value::from(record.producer_name().to_string()),
        );

        // Add custom attributes if present
        for (key, value) in record.attributes() {
            let prefixed_key = format!("_danube_attr_{}", key);
            payload.insert(prefixed_key, Value::from(value.clone()));
        }
    }

    Ok(payload)
}

/// Recursively convert JSON value to Qdrant payload values
fn add_json_to_payload(
    payload: &mut HashMap<String, Value>,
    prefix: &str,
    value: serde_json::Value,
) {
    match value {
        serde_json::Value::Null => {
            // Skip null values
        }
        serde_json::Value::Bool(b) => {
            payload.insert(prefix.to_string(), Value::from(b));
        }
        serde_json::Value::Number(n) => {
            if let Some(i) = n.as_i64() {
                payload.insert(prefix.to_string(), Value::from(i));
            } else if let Some(f) = n.as_f64() {
                payload.insert(prefix.to_string(), Value::from(f));
            }
        }
        serde_json::Value::String(s) => {
            payload.insert(prefix.to_string(), Value::from(s));
        }
        serde_json::Value::Array(arr) => {
            // Convert array to Qdrant list value
            let list_values: Vec<Value> = arr
                .into_iter()
                .filter_map(|item| match item {
                    serde_json::Value::String(s) => Some(Value::from(s)),
                    serde_json::Value::Number(n) => {
                        if let Some(i) = n.as_i64() {
                            Some(Value::from(i))
                        } else if let Some(f) = n.as_f64() {
                            Some(Value::from(f))
                        } else {
                            None
                        }
                    }
                    serde_json::Value::Bool(b) => Some(Value::from(b)),
                    _ => None,
                })
                .collect();

            if !list_values.is_empty() {
                payload.insert(
                    prefix.to_string(),
                    Value {
                        kind: Some(qdrant_client::qdrant::value::Kind::ListValue(
                            qdrant_client::qdrant::ListValue {
                                values: list_values,
                            },
                        )),
                    },
                );
            }
        }
        serde_json::Value::Object(obj) => {
            // Flatten nested objects with dot notation
            for (key, val) in obj {
                let new_prefix = if prefix.is_empty() {
                    key
                } else {
                    format!("{}.{}", prefix, key)
                };
                add_json_to_payload(payload, &new_prefix, val);
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use danube_connect_core::SinkRecord;
    use danube_core::message::{MessageID, StreamMessage};
    use std::collections::HashMap;

    #[test]
    fn test_transform_valid_message() {
        let json = serde_json::json!({
            "id": "test-123",
            "vector": [0.1, 0.2, 0.3],
            "payload": {
                "text": "Hello world",
                "user_id": "user-456"
            }
        });

        let message = StreamMessage {
            request_id: 1,
            msg_id: MessageID {
                producer_id: 100,
                topic_name: "/test/vectors".to_string(),
                broker_addr: "localhost:6650".to_string(),
                topic_offset: 42,
            },
            payload: serde_json::to_vec(&json).unwrap(),
            publish_time: 1234567890,
            producer_name: "test-producer".to_string(),
            subscription_name: Some("test-sub".to_string()),
            attributes: HashMap::new(),
            schema_id: None,
            schema_version: None,
        };

        let record = SinkRecord::from_stream_message(message, None);
        let point = transform_to_point(&record, 3, true).unwrap();

        // Verify point was created successfully
        assert!(point.id.is_some());
        assert!(point.payload.len() > 0); // Should have payload with metadata
    }

    #[test]
    fn test_transform_dimension_mismatch() {
        let json = serde_json::json!({
            "vector": [0.1, 0.2]  // Only 2 dimensions
        });

        let message = StreamMessage {
            request_id: 1,
            msg_id: MessageID {
                producer_id: 100,
                topic_name: "/test/vectors".to_string(),
                broker_addr: "localhost:6650".to_string(),
                topic_offset: 42,
            },
            payload: serde_json::to_vec(&json).unwrap(),
            publish_time: 1234567890,
            producer_name: "test-producer".to_string(),
            subscription_name: Some("test-sub".to_string()),
            attributes: HashMap::new(),
            schema_id: None,
            schema_version: None,
        };

        let record = SinkRecord::from_stream_message(message, None);
        let result = transform_to_point(&record, 3, false); // Expect 3 dimensions

        assert!(result.is_err());
    }

    #[test]
    fn test_hash_string_to_u64() {
        let id1 = hash_string_to_u64("test-123");
        let id2 = hash_string_to_u64("test-456");

        // Different strings should produce different IDs
        assert_ne!(id1, id2);

        // Same string should produce same ID
        assert_eq!(hash_string_to_u64("test-123"), id1);
    }

    #[test]
    fn test_generate_point_id_from_offset() {
        let message = VectorMessage {
            id: None,
            vector: vec![0.1, 0.2, 0.3],
            payload: None,
        };

        let stream_message = StreamMessage {
            request_id: 1,
            msg_id: MessageID {
                producer_id: 100,
                topic_name: "/test/vectors".to_string(),
                broker_addr: "localhost:6650".to_string(),
                topic_offset: 42,
            },
            payload: vec![],
            publish_time: 1234567890,
            producer_name: "test-producer".to_string(),
            subscription_name: Some("test-sub".to_string()),
            attributes: HashMap::new(),
            schema_id: None,
            schema_version: None,
        };

        let record = SinkRecord::from_stream_message(stream_message, None);
        let id = generate_point_id(&message, &record);

        // Should generate consistent ID based on topic + offset
        assert!(id > 0);
    }
}
