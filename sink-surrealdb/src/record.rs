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
    // Note: Unit tests that construct SinkRecord instances have been removed
    // as SinkRecord::from_stream_message() is a private API in danube-connect-core v0.4.0.
    // The transformation logic (add_timestamp, add_metadata) is tested via integration tests.
    // End-to-end functionality is validated in the connector's integration test suite.
}
