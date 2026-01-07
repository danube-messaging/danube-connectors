//! Record processing module for SurrealDB Sink Connector
//!
//! This module converts Danube messages into SurrealDB records.
//! Payloads are already deserialized as serde_json::Value by the runtime.
//!
//! Supports two storage modes:
//! - Document: Regular document storage (default)
//! - TimeSeries: Adds timestamp field for time-series optimization

use crate::config::{StorageMode, TopicMapping};
use chrono::{DateTime, Utc};
use danube_connect_core::{ConnectorResult, SinkRecord};
use serde_json::{json, Value};

/// Represents a SurrealDB record ready for insertion
#[derive(Debug, Clone)]
pub struct SurrealDBRecord {
    /// Optional record ID (from message attributes)
    pub id: Option<String>,

    /// Record data - payload wrapped based on schema type
    pub data: Value,
}

/// Convert a Danube SinkRecord into a SurrealDB record
///
/// This function uses danube-connect-core's unified deserialization method,
/// ensuring consistent behavior across all sink connectors.
///
/// Record ID comes from message attributes (set by producer).
///
/// For TimeSeries mode, adds a timestamp field for temporal queries.
pub fn to_surrealdb_record(
    record: &SinkRecord,
    mapping: &TopicMapping,
) -> ConnectorResult<SurrealDBRecord> {
    // Get record ID from message attributes (set by producer)
    let id = record.get_attribute("record_id").map(|s| s.to_string());

    // Get typed payload (already deserialized by runtime)
    let mut data = record.payload().clone();

    // Add timestamp for time-series mode
    if mapping.storage_mode == StorageMode::TimeSeries {
        add_timestamp(&mut data, record, mapping)?;
    }

    // Add Danube metadata if configured
    if mapping.include_danube_metadata {
        add_metadata(&mut data, record);
    }

    Ok(SurrealDBRecord { id, data })
}

/// Add timestamp for time-series mode
///
/// Uses Danube publish_time (microseconds since epoch) as the timestamp
fn add_timestamp(
    data: &mut Value,
    record: &SinkRecord,
    _mapping: &TopicMapping,
) -> ConnectorResult<()> {
    // Convert publish_time (microseconds) to DateTime<Utc>
    let publish_time_micros = record.publish_time();
    let publish_time_secs = (publish_time_micros / 1_000_000) as i64;
    let publish_time_nanos = ((publish_time_micros % 1_000_000) * 1000) as u32;

    let timestamp = DateTime::from_timestamp(publish_time_secs, publish_time_nanos)
        .unwrap_or_else(|| Utc::now());

    // Add timestamp to data
    if let Value::Object(map) = data {
        map.insert("_timestamp".to_string(), json!(timestamp.to_rfc3339()));
    }

    Ok(())
}

/// Add Danube metadata to the record
fn add_metadata(data: &mut Value, record: &SinkRecord) {
    // Convert publish_time (microseconds) to DateTime<Utc>
    let publish_time_secs = record.publish_time() / 1_000_000;
    let publish_time_nanos = ((record.publish_time() % 1_000_000) * 1000) as u32;
    let datetime = DateTime::from_timestamp(publish_time_secs as i64, publish_time_nanos)
        .unwrap_or_else(|| Utc::now());

    let metadata = json!({
        "danube_topic": record.topic(),
        "danube_offset": record.offset(),
        "danube_timestamp": datetime.to_rfc3339(),
        "danube_message_id": record.message_id(),
    });

    if let Value::Object(map) = data {
        map.insert("_danube_metadata".to_string(), metadata);
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::config::StorageMode;
    use danube_connect_core::SchemaType;
    use danube_core::message::{MessageID, StreamMessage};
    use serde_json::json;
    use std::collections::HashMap;

    fn create_test_record(payload: Vec<u8>) -> SinkRecord {
        let message = StreamMessage {
            request_id: 1,
            msg_id: MessageID {
                producer_id: 1,
                topic_name: "/test/topic".to_string(),
                broker_addr: "localhost:6650".to_string(),
                topic_offset: 42,
            },
            payload,
            publish_time: Utc::now().timestamp_micros() as u64,
            producer_name: "test-producer".to_string(),
            subscription_name: Some("test-sub".to_string()),
            attributes: HashMap::new(),
            schema_id: None,
            schema_version: None,
        };

        SinkRecord::from_stream_message(message, None)
    }

    #[test]
    fn test_json_record() {
        let payload = serde_json::to_vec(&json!({
            "id": "user-123",
            "name": "John Doe",
            "age": 30
        }))
        .unwrap();

        let record = create_test_record(payload);
        let mapping = TopicMapping {
            topic: "/test/topic".to_string(),
            subscription: "test-sub".to_string(),
            table_name: "users".to_string(),
            include_danube_metadata: false,
            batch_size: None,
            flush_interval_ms: None,
            schema_type: SchemaType::Json,
            storage_mode: StorageMode::Document,
        };

        let result = to_surrealdb_record(&record, &mapping).unwrap();
        assert_eq!(result.id, None); // ID comes from attributes, not payload
        assert_eq!(result.data["name"], "John Doe");
        assert_eq!(result.data["age"], 30);
        assert_eq!(result.data["id"], "user-123"); // ID stays in payload
    }

    #[test]
    fn test_string_record() {
        // In v0.7.0, payloads are JSON-serialized
        let payload = serde_json::to_vec(&json!("Hello, SurrealDB!")).unwrap();
        let record = create_test_record(payload);
        let mapping = TopicMapping {
            topic: "/test/topic".to_string(),
            subscription: "test-sub".to_string(),
            table_name: "messages".to_string(),
            include_danube_metadata: false,
            batch_size: None,
            flush_interval_ms: None,
            schema_type: SchemaType::String,
            storage_mode: StorageMode::Document,
        };

        let result = to_surrealdb_record(&record, &mapping).unwrap();
        assert_eq!(result.id, None);
        // String schema type returns Value::String
        assert_eq!(result.data, "Hello, SurrealDB!");
    }

    #[test]
    fn test_int64_record() {
        // In v0.7.0, payloads are JSON-serialized
        let payload = serde_json::to_vec(&json!(42)).unwrap();
        let record = create_test_record(payload);
        let mapping = TopicMapping {
            topic: "/test/topic".to_string(),
            subscription: "test-sub".to_string(),
            table_name: "counters".to_string(),
            include_danube_metadata: false,
            batch_size: None,
            flush_interval_ms: None,
            schema_type: SchemaType::Int64,
            storage_mode: StorageMode::Document,
        };

        let result = to_surrealdb_record(&record, &mapping).unwrap();
        assert_eq!(result.id, None);
        // Number returned as-is
        assert_eq!(result.data, 42);
    }

    #[test]
    fn test_bytes_record() {
        // In v0.7.0, binary data is wrapped in JSON object with base64
        let raw_bytes = vec![0x01, 0x02, 0x03, 0x04];
        let payload_obj = json!({
            "data": base64::Engine::encode(&base64::engine::general_purpose::STANDARD, &raw_bytes),
            "size": raw_bytes.len(),
            "encoding": "base64"
        });
        let payload = serde_json::to_vec(&payload_obj).unwrap();
        let record = create_test_record(payload);
        let mapping = TopicMapping {
            topic: "/test/topic".to_string(),
            subscription: "test-sub".to_string(),
            table_name: "binary_data".to_string(),
            include_danube_metadata: false,
            batch_size: None,
            flush_interval_ms: None,
            schema_type: SchemaType::Bytes,
            storage_mode: StorageMode::Document,
        };

        let result = to_surrealdb_record(&record, &mapping).unwrap();
        assert_eq!(result.id, None);
        assert_eq!(result.data["size"], 4);
        // Verify base64 encoding is preserved
        let expected_base64 =
            base64::Engine::encode(&base64::engine::general_purpose::STANDARD, &raw_bytes);
        assert_eq!(result.data["data"], expected_base64);
    }

    #[test]
    fn test_metadata_enrichment() {
        let payload = serde_json::to_vec(&json!({"event": "test"})).unwrap();
        let record = create_test_record(payload);
        let mapping = TopicMapping {
            topic: "/test/topic".to_string(),
            subscription: "test-sub".to_string(),
            table_name: "events".to_string(),
            include_danube_metadata: true,
            batch_size: None,
            flush_interval_ms: None,
            schema_type: SchemaType::Json,
            storage_mode: StorageMode::Document,
        };

        let result = to_surrealdb_record(&record, &mapping).unwrap();
        assert!(result.data.get("_danube_metadata").is_some());
        let metadata = &result.data["_danube_metadata"];
        assert_eq!(metadata["danube_topic"], "/test/topic");
        assert_eq!(metadata["danube_offset"], 42);
    }
}
