//! Flexible key type system for Caustics
//!
//! This module provides a unified key type that can handle different primary key types
//! (i32, String, Uuid, etc.) while maintaining transparency to the user.

use sea_orm::Value;
use std::fmt;
use std::str::FromStr;
use uuid::Uuid;

/// A flexible key type that can represent different primary key types
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub enum CausticsKey {
    Int(i32),
    String(String),
    Uuid(Uuid),
    // Add more types as needed
    // Note: For now, we keep the core types simple to maintain compatibility
    // Additional types can be added as needed
}

impl CausticsKey {
    /// Create a new key from an i32
    pub fn from_i32(value: i32) -> Self {
        Self::Int(value)
    }

    /// Create a new key from a String
    pub fn from_string(value: String) -> Self {
        Self::String(value)
    }

    /// Create a new key from a Uuid
    pub fn from_uuid(value: Uuid) -> Self {
        Self::Uuid(value)
    }

    /// Convert to a sea_orm::Value for database operations
    pub fn to_db_value(&self) -> Value {
        match self {
            Self::Int(value) => Value::Int(Some(*value)),
            Self::String(value) => Value::String(Some(Box::new(value.clone()))),
            Self::Uuid(value) => Value::Uuid(Some(Box::new(*value))),
        }
    }

    /// Convert to a value for a specific entity using registry type information
    pub fn as_value_for_entity<T: 'static + Default>(
        &self,
        registry: &dyn crate::EntityTypeRegistry,
        entity: &str,
    ) -> T {
        // Get the expected type for this entity's primary key
        if let Some(_expected_type_id) = registry.get_primary_key_type(entity) {
            // Convert the key to the expected type using the registry
            let converted = registry.convert_key_for_primary_key(entity, self.clone());
            // Try to downcast to the expected type
            if let Ok(value) = converted.downcast::<T>() {
                *value
            } else {
                T::default()
            }
        } else {
            T::default()
        }
    }

    /// Convert to a value for a specific field using registry type information
    pub fn as_value_for_field<T: 'static + Default>(
        &self,
        registry: &dyn crate::EntityTypeRegistry,
        entity: &str,
        field: &str,
    ) -> T {
        // Get the expected type for this entity's foreign key field
        if let Some(_expected_type_id) = registry.get_foreign_key_type(entity, field) {
            // Convert the key to the expected type using the registry
            let converted = registry.convert_key_for_foreign_key(entity, field, self.clone());
            // Try to downcast to the expected type
            if let Ok(value) = converted.downcast::<T>() {
                *value
            } else {
                T::default()
            }
        } else {
            T::default()
        }
    }

    /// Create from a sea_orm::Value
    pub fn from_db_value(value: &Value) -> Option<Self> {
        match value {
            Value::Int(Some(i)) => Some(Self::Int(*i)),
            Value::String(Some(s)) => {
                // Try to parse as UUID first, then fall back to string
                if let Ok(uuid) = Uuid::parse_str(s) {
                    Some(Self::Uuid(uuid))
                } else {
                    Some(Self::String((**s).clone()))
                }
            }
            Value::Uuid(Some(uuid)) => Some(Self::Uuid(**uuid)),
            _ => None,
        }
    }
}

impl fmt::Display for CausticsKey {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Int(value) => write!(f, "{}", value),
            Self::String(value) => write!(f, "{}", value),
            Self::Uuid(value) => write!(f, "{}", value),
        }
    }
}

impl FromStr for CausticsKey {
    type Err = String;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        // Handle wrapped format first: Int(42), String(hello), Uuid(uuid-string)
        if s.starts_with("Int(") && s.ends_with(")") {
            let value_str = &s[4..s.len()-1];
            if let Ok(value) = value_str.parse::<i32>() {
                return Ok(Self::Int(value));
            }
        } else if s.starts_with("String(") && s.ends_with(")") {
            let value_str = &s[7..s.len()-1];
            return Ok(Self::String(value_str.to_string()));
        } else if s.starts_with("Uuid(") && s.ends_with(")") {
            let value_str = &s[5..s.len()-1];
            if let Ok(uuid) = Uuid::parse_str(value_str) {
                return Ok(Self::Uuid(uuid));
            }
        }

        // Fall back to comprehensive type parsing for raw values
        // Try i32 first (most common for primary keys)
        if let Ok(value) = s.parse::<i32>() {
            return Ok(Self::Int(value));
        }

        // Try i64 (for larger integers)
        if let Ok(value) = s.parse::<i64>() {
            return Ok(Self::Int(value as i32)); // Convert to i32 for consistency
        }

        // Try u32 (for unsigned integers)
        if let Ok(value) = s.parse::<u32>() {
            return Ok(Self::Int(value as i32));
        }

        // Try u64 (for larger unsigned integers)
        if let Ok(value) = s.parse::<u64>() {
            return Ok(Self::Int(value as i32)); // Convert to i32 for consistency
        }

        // Try f64 (for floating point numbers)
        if let Ok(value) = s.parse::<f64>() {
            return Ok(Self::Int(value as i32)); // Convert to i32 for consistency
        }

        // Try f32 (for single precision floats)
        if let Ok(value) = s.parse::<f32>() {
            return Ok(Self::Int(value as i32)); // Convert to i32 for consistency
        }

        // Try bool (for boolean values)
        if let Ok(value) = s.parse::<bool>() {
            return Ok(Self::Int(if value { 1 } else { 0 }));
        }

        // Try UUID (for UUID strings)
        if let Ok(uuid) = Uuid::parse_str(s) {
            return Ok(Self::Uuid(uuid));
        }

        // Fall back to string for everything else
        Ok(Self::String(s.to_string()))
    }
}

/// Centralized function to parse any string value into the most appropriate SeaORM Value
/// This replaces the scattered i32->String fallback logic throughout the codebase
pub fn parse_string_to_sea_orm_value(s: &str) -> sea_orm::Value {
    // Try to parse as i32 first (most common for primary keys)
    if let Ok(value) = s.parse::<i32>() {
        return sea_orm::Value::Int(Some(value));
    }

    // Try i64 (for larger integers)
    if let Ok(value) = s.parse::<i64>() {
        return sea_orm::Value::BigInt(Some(value));
    }

    // Try u32 (for unsigned integers)
    if let Ok(value) = s.parse::<u32>() {
        return sea_orm::Value::Int(Some(value as i32));
    }

    // Try u64 (for larger unsigned integers)
    if let Ok(value) = s.parse::<u64>() {
        return sea_orm::Value::BigInt(Some(value as i64));
    }

    // Try f64 (for floating point numbers)
    if let Ok(value) = s.parse::<f64>() {
        return sea_orm::Value::Double(Some(value));
    }

    // Try f32 (for single precision floats)
    if let Ok(value) = s.parse::<f32>() {
        return sea_orm::Value::Float(Some(value));
    }

    // Try bool (for boolean values)
    if let Ok(value) = s.parse::<bool>() {
        return sea_orm::Value::Bool(Some(value));
    }

    // Try UUID (for UUID strings)
    if let Ok(uuid) = Uuid::parse_str(s) {
        return sea_orm::Value::Uuid(Some(Box::new(uuid)));
    }

    // Fall back to string for everything else
    sea_orm::Value::String(Some(Box::new(s.to_string())))
}

/// Centralized function to extract any value from database results as a string
/// This replaces the scattered multi-type fallback logic in query builders
pub fn extract_db_value_as_string(row: &sea_orm::QueryResult, alias: &str) -> Option<String> {
    // Try all possible database types in order of likelihood
    if let Ok(v) = row.try_get::<i64>("", alias) {
        return Some(v.to_string());
    }
    if let Ok(v) = row.try_get::<i32>("", alias) {
        return Some(v.to_string());
    }
    if let Ok(v) = row.try_get::<u64>("", alias) {
        return Some(v.to_string());
    }
    if let Ok(v) = row.try_get::<u32>("", alias) {
        return Some(v.to_string());
    }
    if let Ok(v) = row.try_get::<f64>("", alias) {
        return Some(v.to_string());
    }
    if let Ok(v) = row.try_get::<f32>("", alias) {
        return Some(v.to_string());
    }
    if let Ok(v) = row.try_get::<bool>("", alias) {
        return Some(v.to_string());
    }
    if let Ok(v) = row.try_get::<String>("", alias) {
        return Some(v);
    }
    if let Ok(v) = row.try_get::<uuid::Uuid>("", alias) {
        return Some(v.to_string());
    }
    if let Ok(v) = row.try_get::<chrono::DateTime<chrono::Utc>>("", alias) {
        return Some(v.to_rfc3339());
    }
    if let Ok(v) = row.try_get::<chrono::NaiveDateTime>("", alias) {
        return Some(v.to_string());
    }
    if let Ok(v) = row.try_get::<chrono::NaiveDate>("", alias) {
        return Some(v.to_string());
    }
    if let Ok(v) = row.try_get::<chrono::NaiveTime>("", alias) {
        return Some(v.to_string());
    }
    
    // If nothing worked, return None
    None
}

// Conversion traits for easy integration
impl From<i32> for CausticsKey {
    fn from(value: i32) -> Self {
        Self::Int(value)
    }
}

impl From<String> for CausticsKey {
    fn from(value: String) -> Self {
        Self::String(value)
    }
}

impl From<Uuid> for CausticsKey {
    fn from(value: Uuid) -> Self {
        Self::Uuid(value)
    }
}

impl From<&str> for CausticsKey {
    fn from(value: &str) -> Self {
        Self::String(value.to_string())
    }
}

// Try to convert to specific types
impl TryFrom<CausticsKey> for i32 {
    type Error = String;

    fn try_from(key: CausticsKey) -> Result<Self, Self::Error> {
        match key {
            CausticsKey::Int(value) => Ok(value),
            _ => Err(format!("Cannot convert {:?} to i32", key)),
        }
    }
}

impl TryFrom<CausticsKey> for String {
    type Error = String;

    fn try_from(key: CausticsKey) -> Result<Self, Self::Error> {
        match key {
            CausticsKey::String(value) => Ok(value),
            CausticsKey::Uuid(value) => Ok(value.to_string()),
            _ => Err(format!("Cannot convert {:?} to String", key)),
        }
    }
}

impl TryFrom<CausticsKey> for Uuid {
    type Error = String;

    fn try_from(key: CausticsKey) -> Result<Self, Self::Error> {
        match key {
            CausticsKey::Uuid(value) => Ok(value),
            _ => Err(format!("Cannot convert {:?} to Uuid", key)),
        }
    }
}

// Implement From<CausticsKey> for sea_orm::Value
impl From<CausticsKey> for sea_orm::Value {
    fn from(key: CausticsKey) -> Self {
        key.to_db_value()
    }
}

/// Trait for types that can be converted to/from CausticsKey
pub trait KeyConvertible {
    fn to_caustics_key(self) -> CausticsKey;
    fn from_caustics_key(key: CausticsKey) -> Result<Self, String>
    where
        Self: Sized;
}

impl KeyConvertible for i32 {
    fn to_caustics_key(self) -> CausticsKey {
        CausticsKey::Int(self)
    }

    fn from_caustics_key(key: CausticsKey) -> Result<Self, String> {
        key.try_into()
    }
}

impl KeyConvertible for String {
    fn to_caustics_key(self) -> CausticsKey {
        CausticsKey::String(self)
    }

    fn from_caustics_key(key: CausticsKey) -> Result<Self, String> {
        key.try_into()
    }
}

impl KeyConvertible for Uuid {
    fn to_caustics_key(self) -> CausticsKey {
        CausticsKey::Uuid(self)
    }

    fn from_caustics_key(key: CausticsKey) -> Result<Self, String> {
        key.try_into()
    }
}

/// Helper trait for extracting keys from models
pub trait ExtractKey {
    fn extract_key(&self, field_name: &str) -> Option<CausticsKey>;
}

/// Helper trait for setting keys on models
pub trait SetKey {
    fn set_key(&mut self, field_name: &str, key: CausticsKey);
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_key_creation() {
        let int_key = CausticsKey::from_i32(42);
        assert_eq!(int_key, CausticsKey::Int(42));

        let string_key = CausticsKey::from_string("test".to_string());
        assert_eq!(string_key, CausticsKey::String("test".to_string()));

        let uuid = Uuid::new_v4();
        let uuid_key = CausticsKey::from_uuid(uuid);
        assert_eq!(uuid_key, CausticsKey::Uuid(uuid));
    }

    #[test]
    fn test_key_conversion() {
        let key: CausticsKey = 42.into();
        assert_eq!(key, CausticsKey::Int(42));

        let key: CausticsKey = "test".into();
        assert_eq!(key, CausticsKey::String("test".to_string()));

        let uuid = Uuid::new_v4();
        let key: CausticsKey = uuid.into();
        assert_eq!(key, CausticsKey::Uuid(uuid));
    }

    #[test]
    fn test_db_value_conversion() {
        let key = CausticsKey::Int(42);
        let db_value = key.to_db_value();
        assert_eq!(db_value, Value::Int(Some(42)));

        let key = CausticsKey::String("test".to_string());
        let db_value = key.to_db_value();
        assert_eq!(db_value, Value::String(Some(Box::new("test".to_string()))));

        let uuid = Uuid::new_v4();
        let key = CausticsKey::Uuid(uuid);
        let db_value = key.to_db_value();
        assert_eq!(db_value, Value::Uuid(Some(Box::new(uuid))));
    }

    #[test]
    fn test_key_convertible() {
        let key = 42i32.to_caustics_key();
        assert_eq!(key, CausticsKey::Int(42));

        let converted: i32 = i32::from_caustics_key(key).expect("Failed to convert CausticsKey to i32");
        assert_eq!(converted, 42);
    }
}
