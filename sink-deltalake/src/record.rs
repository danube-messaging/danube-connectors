//! Record transformation module for Delta Lake Sink Connector
//!
//! This module handles conversion from Danube SinkRecords to Arrow RecordBatches,
//! which are then written to Delta Lake as Parquet files.
//!
//! Supports all Danube schema types (Json, String, Int64) and includes optional
//! Danube metadata as a JSON column.

use crate::config::TopicMapping;
use arrow::array::{
    ArrayRef, BooleanArray, Float32Array, Float64Array, Int16Array, Int32Array, Int64Array,
    Int8Array, StringArray, TimestampMicrosecondArray, UInt16Array, UInt32Array, UInt64Array,
    UInt8Array,
};
use arrow::datatypes::{DataType, Field, Schema, TimeUnit};
use arrow::record_batch::RecordBatch;
use chrono::Utc;
use danube_connect_core::{ConnectorError, ConnectorResult, SinkRecord};
use serde_json::Value;
use std::sync::Arc;

/// Convert a batch of Danube SinkRecords into an Arrow RecordBatch
///
/// This function:
/// 1. Gets typed payload from each record (already deserialized by runtime)
/// 2. Extracts fields according to user-defined schema
/// 3. Optionally adds Danube metadata as a JSON column
/// 4. Builds Arrow arrays for each column
/// 5. Creates a RecordBatch
pub fn to_record_batch(
    records: &[SinkRecord],
    mapping: &TopicMapping,
) -> ConnectorResult<RecordBatch> {
    if records.is_empty() {
        return Err(ConnectorError::fatal(
            "Cannot create RecordBatch from empty records",
        ));
    }

    // Build Arrow schema from config
    let schema = build_arrow_schema(mapping)?;

    // Get typed payloads from records (already deserialized by runtime)
    let deserialized: Vec<Value> = records.iter().map(|r| r.payload().clone()).collect();

    // Build arrays for each field in the schema
    let mut arrays: Vec<ArrayRef> = Vec::new();

    for field in &mapping.schema {
        let array = build_array_for_field(&field.name, &field.data_type, &deserialized)?;
        arrays.push(array);
    }

    // Add metadata column if configured
    if mapping.include_danube_metadata {
        let metadata_array = build_metadata_array(records)?;
        arrays.push(metadata_array);
    }

    // Create RecordBatch
    let batch = RecordBatch::try_new(schema, arrays)
        .map_err(|e| ConnectorError::fatal(format!("Failed to create Arrow RecordBatch: {}", e)))?;

    Ok(batch)
}

/// Build Arrow schema from user-defined schema configuration
pub fn build_arrow_schema(mapping: &TopicMapping) -> ConnectorResult<Arc<Schema>> {
    let mut fields: Vec<Field> = Vec::new();

    // Add user-defined fields
    for schema_field in &mapping.schema {
        let data_type = parse_arrow_type(&schema_field.data_type)?;
        let field = Field::new(&schema_field.name, data_type, schema_field.nullable);
        fields.push(field);
    }

    // Add metadata field if configured
    if mapping.include_danube_metadata {
        fields.push(Field::new("_danube_metadata", DataType::Utf8, false));
    }

    Ok(Arc::new(Schema::new(fields)))
}

/// Parse Arrow data type from string
fn parse_arrow_type(type_str: &str) -> ConnectorResult<DataType> {
    let data_type = match type_str {
        "Utf8" => DataType::Utf8,
        "Int8" => DataType::Int8,
        "Int16" => DataType::Int16,
        "Int32" => DataType::Int32,
        "Int64" => DataType::Int64,
        "UInt8" => DataType::UInt8,
        "UInt16" => DataType::UInt16,
        "UInt32" => DataType::UInt32,
        "UInt64" => DataType::UInt64,
        "Float32" => DataType::Float32,
        "Float64" => DataType::Float64,
        "Boolean" => DataType::Boolean,
        "Timestamp" => DataType::Timestamp(TimeUnit::Microsecond, None),
        "Binary" => DataType::Binary,
        _ => {
            return Err(ConnectorError::fatal(format!(
                "Unsupported Arrow data type: {}",
                type_str
            )))
        }
    };

    Ok(data_type)
}

/// Build an Arrow array for a specific field
fn build_array_for_field(
    field_name: &str,
    data_type: &str,
    values: &[Value],
) -> ConnectorResult<ArrayRef> {
    match data_type {
        "Utf8" => {
            let array: StringArray = values
                .iter()
                .map(|v| extract_string_field(v, field_name))
                .collect();
            Ok(Arc::new(array))
        }
        "Int8" => {
            let array: Int8Array = values
                .iter()
                .map(|v| extract_int_field(v, field_name).map(|n| n as i8))
                .collect();
            Ok(Arc::new(array))
        }
        "Int16" => {
            let array: Int16Array = values
                .iter()
                .map(|v| extract_int_field(v, field_name).map(|n| n as i16))
                .collect();
            Ok(Arc::new(array))
        }
        "Int32" => {
            let array: Int32Array = values
                .iter()
                .map(|v| extract_int_field(v, field_name).map(|n| n as i32))
                .collect();
            Ok(Arc::new(array))
        }
        "Int64" => {
            let array: Int64Array = values
                .iter()
                .map(|v| extract_int_field(v, field_name))
                .collect();
            Ok(Arc::new(array))
        }
        "UInt8" => {
            let array: UInt8Array = values
                .iter()
                .map(|v| extract_uint_field(v, field_name).map(|n| n as u8))
                .collect();
            Ok(Arc::new(array))
        }
        "UInt16" => {
            let array: UInt16Array = values
                .iter()
                .map(|v| extract_uint_field(v, field_name).map(|n| n as u16))
                .collect();
            Ok(Arc::new(array))
        }
        "UInt32" => {
            let array: UInt32Array = values
                .iter()
                .map(|v| extract_uint_field(v, field_name).map(|n| n as u32))
                .collect();
            Ok(Arc::new(array))
        }
        "UInt64" => {
            let array: UInt64Array = values
                .iter()
                .map(|v| extract_uint_field(v, field_name))
                .collect();
            Ok(Arc::new(array))
        }
        "Float32" => {
            let array: Float32Array = values
                .iter()
                .map(|v| extract_float_field(v, field_name).map(|n| n as f32))
                .collect();
            Ok(Arc::new(array))
        }
        "Float64" => {
            let array: Float64Array = values
                .iter()
                .map(|v| extract_float_field(v, field_name))
                .collect();
            Ok(Arc::new(array))
        }
        "Boolean" => {
            let array: BooleanArray = values
                .iter()
                .map(|v| extract_bool_field(v, field_name))
                .collect();
            Ok(Arc::new(array))
        }
        "Timestamp" => {
            let array: TimestampMicrosecondArray = values
                .iter()
                .map(|v| extract_timestamp_field(v, field_name))
                .collect();
            Ok(Arc::new(array))
        }
        _ => Err(ConnectorError::fatal(format!(
            "Unsupported data type for field '{}': {}",
            field_name, data_type
        ))),
    }
}

/// Extract string field from JSON value
fn extract_string_field(value: &Value, field_name: &str) -> Option<String> {
    match value {
        Value::Object(map) => map
            .get(field_name)
            .and_then(|v| v.as_str())
            .map(String::from),
        _ => None,
    }
}

/// Extract integer field from JSON value
fn extract_int_field(value: &Value, field_name: &str) -> Option<i64> {
    match value {
        Value::Object(map) => map.get(field_name).and_then(|v| v.as_i64()),
        _ => None,
    }
}

/// Extract unsigned integer field from JSON value
fn extract_uint_field(value: &Value, field_name: &str) -> Option<u64> {
    match value {
        Value::Object(map) => map.get(field_name).and_then(|v| v.as_u64()),
        _ => None,
    }
}

/// Extract float field from JSON value
fn extract_float_field(value: &Value, field_name: &str) -> Option<f64> {
    match value {
        Value::Object(map) => map.get(field_name).and_then(|v| v.as_f64()),
        _ => None,
    }
}

/// Extract boolean field from JSON value
fn extract_bool_field(value: &Value, field_name: &str) -> Option<bool> {
    match value {
        Value::Object(map) => map.get(field_name).and_then(|v| v.as_bool()),
        _ => None,
    }
}

/// Extract timestamp field from JSON value (expects ISO 8601 string or Unix timestamp)
fn extract_timestamp_field(value: &Value, field_name: &str) -> Option<i64> {
    match value {
        Value::Object(map) => {
            if let Some(field_value) = map.get(field_name) {
                // Try parsing as Unix timestamp (seconds)
                if let Some(timestamp_secs) = field_value.as_i64() {
                    return Some(timestamp_secs * 1_000_000); // Convert to microseconds
                }
                // Try parsing as ISO 8601 string
                if let Some(timestamp_str) = field_value.as_str() {
                    if let Ok(dt) = chrono::DateTime::parse_from_rfc3339(timestamp_str) {
                        return Some(dt.timestamp_micros());
                    }
                }
            }
            None
        }
        _ => None,
    }
}

/// Build metadata array with Danube message metadata as JSON
fn build_metadata_array(records: &[SinkRecord]) -> ConnectorResult<ArrayRef> {
    let metadata_strings: Vec<String> = records
        .iter()
        .map(|record| {
            let metadata = serde_json::json!({
                "topic": record.topic(),
                "offset": record.offset(),
                "timestamp": record.publish_time(),
                "timestamp_iso": Utc::now().to_rfc3339(),
                "partition": record.partition.as_ref().map(|s| s.as_str()).unwrap_or("none"),
            });
            metadata.to_string()
        })
        .collect();

    let array = StringArray::from(metadata_strings);
    Ok(Arc::new(array))
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    #[test]
    fn test_extract_string_field() {
        let value = json!({"name": "Alice", "age": 30});
        assert_eq!(
            extract_string_field(&value, "name"),
            Some("Alice".to_string())
        );
        assert_eq!(extract_string_field(&value, "missing"), None);
    }

    #[test]
    fn test_extract_int_field() {
        let value = json!({"name": "Alice", "age": 30});
        assert_eq!(extract_int_field(&value, "age"), Some(30));
        assert_eq!(extract_int_field(&value, "missing"), None);
    }

    #[test]
    fn test_parse_arrow_type() {
        assert!(matches!(parse_arrow_type("Utf8"), Ok(DataType::Utf8)));
        assert!(matches!(parse_arrow_type("Int64"), Ok(DataType::Int64)));
        assert!(matches!(parse_arrow_type("Float64"), Ok(DataType::Float64)));
        assert!(matches!(parse_arrow_type("Boolean"), Ok(DataType::Boolean)));
        assert!(parse_arrow_type("InvalidType").is_err());
    }
}
