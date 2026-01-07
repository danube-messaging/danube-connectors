//! Record transformation module for Delta Lake Sink Connector
//!
//! This module handles conversion from Danube SinkRecords to Arrow RecordBatches,
//! which are then written to Delta Lake as Parquet files.
//!
//! Supports all Danube schema types (Json, String, Int64) and includes optional
//! Danube metadata as a JSON column.

use crate::config::TopicMapping;
use arrow::array::{ArrayRef, StringArray};
use arrow::datatypes::{DataType, Field, Schema, TimeUnit};
use arrow::record_batch::RecordBatch;
use arrow_json::ReaderBuilder;
use chrono::Utc;
use danube_connect_core::{ConnectorError, ConnectorResult, SinkRecord};
use serde_json::Value;
use std::io::Cursor;
use std::sync::Arc;

/// Convert a batch of Danube SinkRecords into an Arrow RecordBatch
///
/// This function uses arrow-json's ReaderBuilder for efficient, robust conversion:
/// 1. Gets typed payloads from records (already deserialized by runtime)
/// 2. Transforms JSON based on field_mappings (supports nested JSON paths)
/// 3. Uses arrow-json to build RecordBatch with proper null handling and type coercion
/// 4. Optionally adds Danube metadata as a JSON column
pub fn to_record_batch(
    records: &[SinkRecord],
    mapping: &TopicMapping,
) -> ConnectorResult<RecordBatch> {
    if records.is_empty() {
        return Err(ConnectorError::fatal(
            "Cannot create RecordBatch from empty records",
        ));
    }

    // Build Arrow schema from field mappings (without metadata)
    let schema = build_arrow_schema_without_metadata(mapping)?;

    // Transform payloads to match target schema (handle JSON path remapping)
    let transformed_json: Vec<Value> = records
        .iter()
        .map(|record| transform_payload_for_schema(record.payload(), mapping))
        .collect();

    // Use arrow-json to build RecordBatch efficiently
    let batch = json_to_record_batch(schema, &transformed_json)?;

    // If metadata is needed, add it as an additional column
    if mapping.include_danube_metadata {
        let metadata_array = build_metadata_array(records)?;
        return add_metadata_column(batch, metadata_array);
    }

    Ok(batch)
}

/// Transform a JSON payload based on field mappings
/// Extracts values using JSON paths and creates a flat JSON object matching the target schema
fn transform_payload_for_schema(payload: &Value, mapping: &TopicMapping) -> Value {
    let mut transformed = serde_json::Map::new();

    for field_mapping in &mapping.field_mappings {
        // Use pre-split path_parts for optimized extraction (avoids repeated string splitting)
        if let Some(value) = extract_value_by_path_parts(payload, &field_mapping.path_parts) {
            transformed.insert(field_mapping.column.clone(), value.clone());
        }
    }

    Value::Object(transformed)
}

/// Convert JSON values to Arrow RecordBatch using arrow-json
/// This is more efficient and robust than manual array building
fn json_to_record_batch(
    schema: Arc<Schema>,
    json_values: &[Value],
) -> ConnectorResult<RecordBatch> {
    // Convert JSON values to newline-delimited JSON bytes
    let json_bytes: Vec<u8> = json_values
        .iter()
        .map(|v| v.to_string())
        .collect::<Vec<String>>()
        .join("\n")
        .into_bytes();

    // Use arrow-json reader to parse
    let cursor = Cursor::new(json_bytes);
    let mut reader = ReaderBuilder::new(schema)
        .build(cursor)
        .map_err(|e| ConnectorError::fatal(format!("Failed to create JSON reader: {}", e)))?;

    // Read the batch
    let batch = reader
        .next()
        .ok_or_else(|| ConnectorError::fatal("No batch produced from JSON reader"))?
        .map_err(|e| ConnectorError::fatal(format!("Failed to read JSON batch: {}", e)))?;

    Ok(batch)
}

/// Add metadata column to an existing RecordBatch
fn add_metadata_column(
    batch: RecordBatch,
    metadata_array: ArrayRef,
) -> ConnectorResult<RecordBatch> {
    let mut columns: Vec<ArrayRef> = batch.columns().to_vec();
    columns.push(metadata_array);

    let mut fields: Vec<Field> = batch
        .schema()
        .fields()
        .iter()
        .map(|f| (**f).clone())
        .collect();
    fields.push(Field::new("_danube_metadata", DataType::Utf8, false));

    let new_schema = Arc::new(Schema::new(fields));

    RecordBatch::try_new(new_schema, columns)
        .map_err(|e| ConnectorError::fatal(format!("Failed to add metadata column: {}", e)))
}

/// Build Arrow schema from field mappings (without metadata column)
fn build_arrow_schema_without_metadata(mapping: &TopicMapping) -> ConnectorResult<Arc<Schema>> {
    let mut fields: Vec<Field> = Vec::new();

    for field_mapping in &mapping.field_mappings {
        let data_type = parse_arrow_type(&field_mapping.data_type)?;
        let field = Field::new(&field_mapping.column, data_type, field_mapping.nullable);
        fields.push(field);
    }

    Ok(Arc::new(Schema::new(fields)))
}

/// Build Arrow schema from field mappings
pub fn build_arrow_schema(mapping: &TopicMapping) -> ConnectorResult<Arc<Schema>> {
    let mut fields: Vec<Field> = Vec::new();

    // Add fields from field mappings
    for field_mapping in &mapping.field_mappings {
        let data_type = parse_arrow_type(&field_mapping.data_type)?;
        let field = Field::new(&field_mapping.column, data_type, field_mapping.nullable);
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

/// Extract value from JSON using pre-split path parts (optimized)
/// Avoids repeated string splitting for better performance
fn extract_value_by_path_parts<'a>(value: &'a Value, path_parts: &[String]) -> Option<&'a Value> {
    let mut current = value;

    for part in path_parts {
        current = current.get(part.as_str())?;
    }

    Some(current)
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
    fn test_extract_value_by_path_parts_optimized() {
        let value = json!({
            "user": {
                "profile": {
                    "name": "Alice",
                    "age": 30
                }
            },
            "status": "active"
        });

        // Test nested path
        let nested_parts = vec!["user".to_string(), "profile".to_string(), "name".to_string()];
        assert_eq!(
            extract_value_by_path_parts(&value, &nested_parts).and_then(|v| v.as_str()),
            Some("Alice")
        );

        // Test simple path
        let simple_parts = vec!["status".to_string()];
        assert_eq!(
            extract_value_by_path_parts(&value, &simple_parts).and_then(|v| v.as_str()),
            Some("active")
        );

        // Test missing path
        let missing_parts = vec!["user".to_string(), "profile".to_string(), "missing".to_string()];
        assert_eq!(extract_value_by_path_parts(&value, &missing_parts), None);
    }

    #[test]
    fn test_parse_arrow_type() {
        assert!(matches!(parse_arrow_type("Utf8"), Ok(DataType::Utf8)));
        assert!(matches!(parse_arrow_type("Int64"), Ok(DataType::Int64)));
        assert!(matches!(parse_arrow_type("Float64"), Ok(DataType::Float64)));
        assert!(matches!(parse_arrow_type("Boolean"), Ok(DataType::Boolean)));
        assert!(parse_arrow_type("InvalidType").is_err());
    }

    #[test]
    fn test_transform_payload_for_schema() {
        use crate::config::FieldMapping;

        let payload = json!({
            "user": {
                "name": "Alice",
                "age": 30
            },
            "status": "active"
        });

        // Create field mappings and initialize path_parts
        let mut field_mapping1 = FieldMapping {
            json_path: "user.name".to_string(),
            path_parts: vec![], // Will be initialized
            column: "name".to_string(),
            data_type: "Utf8".to_string(),
            nullable: false,
        };
        field_mapping1.init_path_parts();

        let mut field_mapping2 = FieldMapping {
            json_path: "status".to_string(),
            path_parts: vec![], // Will be initialized
            column: "status".to_string(),
            data_type: "Utf8".to_string(),
            nullable: false,
        };
        field_mapping2.init_path_parts();

        let mapping = TopicMapping {
            topic: "/test".to_string(),
            subscription: "test-sub".to_string(),
            delta_table_path: "test-path".to_string(),
            expected_schema_subject: None,
            field_mappings: vec![field_mapping1, field_mapping2],
            write_mode: crate::config::WriteMode::Append,
            batch_size: None,
            flush_interval_ms: None,
            include_danube_metadata: false,
        };

        let transformed = transform_payload_for_schema(&payload, &mapping);
        assert_eq!(transformed["name"].as_str(), Some("Alice"));
        assert_eq!(transformed["status"].as_str(), Some("active"));
    }
}
