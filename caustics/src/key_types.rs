//! Flexible key type system for Caustics
//!
//! This module provides a unified key type that can handle different primary key types
//! (i32, String, Uuid, etc.) while maintaining transparency to the user.

use sea_orm::Value;
use std::fmt;
use std::str::FromStr;
use uuid::Uuid;

/// A flexible key type that can represent different primary key types
#[derive(Debug, Clone, PartialEq)]
pub enum CausticsKey {
    // Integer types
    I8(i8),
    I16(i16),
    I32(i32),
    I64(i64),
    ISize(isize),
    U8(u8),
    U16(u16),
    U32(u32),
    U64(u64),
    USize(usize),
    // Floating point types
    F32(f32),
    F64(f64),
    // String and UUID
    String(String),
    Uuid(Uuid),
    // Boolean
    Bool(bool),
    // DateTime types
    DateTimeUtc(chrono::DateTime<chrono::Utc>),
    NaiveDateTime(chrono::NaiveDateTime),
    NaiveDate(chrono::NaiveDate),
    NaiveTime(chrono::NaiveTime),
    // JSON
    Json(serde_json::Value),
    
    // Composite key types
    Composite(Vec<(String, CausticsKey)>),
    OptionalComposite(Option<Vec<(String, CausticsKey)>>),
}

impl CausticsKey {
    // Integer type constructors
    pub fn from_i8(value: i8) -> Self {
        Self::I8(value)
    }
    pub fn from_i16(value: i16) -> Self {
        Self::I16(value)
    }
    pub fn from_i32(value: i32) -> Self {
        Self::I32(value)
    }
    pub fn from_i64(value: i64) -> Self {
        Self::I64(value)
    }
    pub fn from_isize(value: isize) -> Self {
        Self::ISize(value)
    }
    pub fn from_u8(value: u8) -> Self {
        Self::U8(value)
    }
    pub fn from_u16(value: u16) -> Self {
        Self::U16(value)
    }
    pub fn from_u32(value: u32) -> Self {
        Self::U32(value)
    }
    pub fn from_u64(value: u64) -> Self {
        Self::U64(value)
    }
    pub fn from_usize(value: usize) -> Self {
        Self::USize(value)
    }

    // Floating point type constructors
    pub fn from_f32(value: f32) -> Self {
        Self::F32(value)
    }
    pub fn from_f64(value: f64) -> Self {
        Self::F64(value)
    }

    // String and UUID constructors
    pub fn from_string(value: String) -> Self {
        Self::String(value)
    }
    pub fn from_uuid(value: Uuid) -> Self {
        Self::Uuid(value)
    }

    // Boolean constructor
    pub fn from_bool(value: bool) -> Self {
        Self::Bool(value)
    }

    // DateTime constructors
    pub fn from_datetime_utc(value: chrono::DateTime<chrono::Utc>) -> Self {
        Self::DateTimeUtc(value)
    }
    pub fn from_naive_datetime(value: chrono::NaiveDateTime) -> Self {
        Self::NaiveDateTime(value)
    }
    pub fn from_naive_date(value: chrono::NaiveDate) -> Self {
        Self::NaiveDate(value)
    }
    pub fn from_naive_time(value: chrono::NaiveTime) -> Self {
        Self::NaiveTime(value)
    }

    // JSON constructor
    pub fn from_json(value: serde_json::Value) -> Self {
        Self::Json(value)
    }

    /// Convert to a sea_orm::Value for database operations
    pub fn to_db_value(&self) -> Value {
        match self {
            // Integer types
            Self::I8(value) => Value::TinyInt(Some(*value)),
            Self::I16(value) => Value::SmallInt(Some(*value)),
            Self::I32(value) => Value::Int(Some(*value)),
            Self::I64(value) => Value::BigInt(Some(*value)),
            Self::ISize(value) => Value::BigInt(Some(*value as i64)),
            Self::U8(value) => Value::TinyInt(Some(*value as i8)),
            Self::U16(value) => Value::SmallInt(Some(*value as i16)),
            Self::U32(value) => Value::Int(Some(*value as i32)),
            Self::U64(value) => Value::BigInt(Some(*value as i64)),
            Self::USize(value) => Value::BigInt(Some(*value as i64)),
            // Floating point types
            Self::F32(value) => Value::Float(Some(*value)),
            Self::F64(value) => Value::Double(Some(*value)),
            // String and UUID
            Self::String(value) => Value::String(Some(Box::new(value.clone()))),
            Self::Uuid(value) => Value::Uuid(Some(Box::new(*value))),
            // Boolean
            Self::Bool(value) => Value::Bool(Some(*value)),
            // DateTime types - convert to string representation
            Self::DateTimeUtc(value) => Value::String(Some(Box::new(value.to_rfc3339()))),
            Self::NaiveDateTime(value) => Value::String(Some(Box::new(value.to_string()))),
            Self::NaiveDate(value) => Value::String(Some(Box::new(value.to_string()))),
            Self::NaiveTime(value) => Value::String(Some(Box::new(value.to_string()))),
            // JSON
            Self::Json(value) => Value::Json(Some(Box::new(value.clone()))),
            // Composite keys - fallback to first key's value or null
            Self::Composite(fields) => {
                if let Some((_, first_key)) = fields.first() {
                    first_key.to_db_value()
                } else {
                    Value::String(None)
                }
            }
            Self::OptionalComposite(Some(fields)) => {
                if let Some((_, first_key)) = fields.first() {
                    first_key.to_db_value()
                } else {
                    Value::String(None)
                }
            }
            Self::OptionalComposite(None) => Value::String(None),
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
            Value::TinyInt(Some(i)) => Some(Self::I8(*i)),
            Value::SmallInt(Some(i)) => Some(Self::I16(*i)),
            Value::Int(Some(i)) => Some(Self::I32(*i)),
            Value::BigInt(Some(i)) => Some(Self::I64(*i)),
            Value::Float(Some(f)) => Some(Self::F32(*f)),
            Value::Double(Some(f)) => Some(Self::F64(*f)),
            Value::String(Some(s)) => {
                // Try to parse as UUID first, then fall back to string
                if let Ok(uuid) = Uuid::parse_str(s) {
                    Some(Self::Uuid(uuid))
                } else {
                    Some(Self::String((**s).clone()))
                }
            }
            Value::Uuid(Some(uuid)) => Some(Self::Uuid(**uuid)),
            Value::Bool(Some(b)) => Some(Self::Bool(*b)),
            // DateTime types are handled as strings in SeaORM
            // We'll need to parse them from string representation
            Value::Json(Some(j)) => Some(Self::Json((**j).clone())),
            _ => None,
        }
    }

    // Composite key methods
    pub fn composite(fields: Vec<(String, CausticsKey)>) -> Self {
        Self::Composite(fields)
    }

    pub fn optional_composite(fields: Option<Vec<(String, CausticsKey)>>) -> Self {
        Self::OptionalComposite(fields)
    }

    pub fn as_composite(&self) -> Option<&Vec<(String, CausticsKey)>> {
        match self {
            CausticsKey::Composite(fields) => Some(fields),
            CausticsKey::OptionalComposite(Some(fields)) => Some(fields),
            _ => None,
        }
    }

    pub fn is_optional_composite(&self) -> bool {
        matches!(self, CausticsKey::OptionalComposite(_))
    }

    pub fn to_db_conditions(&self) -> Vec<(String, sea_orm::Value)> {
        match self {
            CausticsKey::Composite(fields) => {
                fields.iter()
                    .map(|(name, key)| (name.clone(), key.to_db_value()))
                    .collect()
            }
            CausticsKey::OptionalComposite(Some(fields)) => {
                fields.iter()
                    .map(|(name, key)| (name.clone(), key.to_db_value()))
                    .collect()
            }
            CausticsKey::OptionalComposite(None) => Vec::new(),
            _ => vec![("".to_string(), self.to_db_value())],
        }
    }

    pub fn to_composite_db_values(&self) -> Vec<sea_orm::Value> {
        match self {
            CausticsKey::Composite(fields) => fields.iter().map(|(_, key)| key.to_db_value()).collect(),
            CausticsKey::OptionalComposite(Some(fields)) => fields.iter().map(|(_, key)| key.to_db_value()).collect(),
            CausticsKey::OptionalComposite(None) => Vec::new(),
            _ => vec![self.to_db_value()],
        }
    }
}

impl Eq for CausticsKey {}

impl std::hash::Hash for CausticsKey {
    fn hash<H: std::hash::Hasher>(&self, state: &mut H) {
        match self {
            // Integer types
            CausticsKey::I8(value) => value.hash(state),
            CausticsKey::I16(value) => value.hash(state),
            CausticsKey::I32(value) => value.hash(state),
            CausticsKey::I64(value) => value.hash(state),
            CausticsKey::ISize(value) => value.hash(state),
            CausticsKey::U8(value) => value.hash(state),
            CausticsKey::U16(value) => value.hash(state),
            CausticsKey::U32(value) => value.hash(state),
            CausticsKey::U64(value) => value.hash(state),
            CausticsKey::USize(value) => value.hash(state),
            // Floating point types - convert to integer representation for hashing
            CausticsKey::F32(value) => value.to_bits().hash(state),
            CausticsKey::F64(value) => value.to_bits().hash(state),
            // String and UUID
            CausticsKey::String(value) => value.hash(state),
            CausticsKey::Uuid(value) => value.hash(state),
            // Boolean
            CausticsKey::Bool(value) => value.hash(state),
            // DateTime types
            CausticsKey::DateTimeUtc(value) => value.hash(state),
            CausticsKey::NaiveDateTime(value) => value.hash(state),
            CausticsKey::NaiveDate(value) => value.hash(state),
            CausticsKey::NaiveTime(value) => value.hash(state),
            // JSON
            CausticsKey::Json(value) => value.to_string().hash(state),
            // Composite keys - hash all components
            CausticsKey::Composite(fields) => {
                for (name, key) in fields {
                    name.hash(state);
                    key.hash(state);
                }
            }
            CausticsKey::OptionalComposite(Some(fields)) => {
                for (name, key) in fields {
                    name.hash(state);
                    key.hash(state);
                }
            }
            CausticsKey::OptionalComposite(None) => "None".hash(state),
        }
    }
}

impl fmt::Display for CausticsKey {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            // Integer types
            Self::I8(value) => write!(f, "{}", value),
            Self::I16(value) => write!(f, "{}", value),
            Self::I32(value) => write!(f, "{}", value),
            Self::I64(value) => write!(f, "{}", value),
            Self::ISize(value) => write!(f, "{}", value),
            Self::U8(value) => write!(f, "{}", value),
            Self::U16(value) => write!(f, "{}", value),
            Self::U32(value) => write!(f, "{}", value),
            Self::U64(value) => write!(f, "{}", value),
            Self::USize(value) => write!(f, "{}", value),
            // Floating point types
            Self::F32(value) => write!(f, "{}", value),
            Self::F64(value) => write!(f, "{}", value),
            // String and UUID
            Self::String(value) => write!(f, "{}", value),
            Self::Uuid(value) => write!(f, "{}", value),
            // Boolean
            Self::Bool(value) => write!(f, "{}", value),
            // DateTime types
            Self::DateTimeUtc(value) => write!(f, "{}", value.to_rfc3339()),
            Self::NaiveDateTime(value) => write!(f, "{}", value),
            Self::NaiveDate(value) => write!(f, "{}", value),
            Self::NaiveTime(value) => write!(f, "{}", value),
            // JSON
            Self::Json(value) => write!(f, "{}", value),
            // Composite keys
            Self::Composite(fields) => {
                let field_strings: Vec<String> = fields.iter()
                    .map(|(name, key)| format!("{}:{}", name, key))
                    .collect();
                write!(f, "Composite({})", field_strings.join(", "))
            }
            Self::OptionalComposite(Some(fields)) => {
                let field_strings: Vec<String> = fields.iter()
                    .map(|(name, key)| format!("{}:{}", name, key))
                    .collect();
                write!(f, "OptionalComposite({})", field_strings.join(", "))
            }
            Self::OptionalComposite(None) => write!(f, "OptionalComposite(None)"),
        }
    }
}

impl FromStr for CausticsKey {
    type Err = String;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        // Handle wrapped format first: I32(42), String(hello), Uuid(uuid-string), etc.
        if s.starts_with("I8(") && s.ends_with(")") {
            let value_str = &s[3..s.len() - 1];
            if let Ok(value) = value_str.parse::<i8>() {
                return Ok(Self::I8(value));
            }
        } else if s.starts_with("I16(") && s.ends_with(")") {
            let value_str = &s[4..s.len() - 1];
            if let Ok(value) = value_str.parse::<i16>() {
                return Ok(Self::I16(value));
            }
        } else if s.starts_with("I32(") && s.ends_with(")") {
            let value_str = &s[4..s.len() - 1];
            if let Ok(value) = value_str.parse::<i32>() {
                return Ok(Self::I32(value));
            }
        } else if s.starts_with("I64(") && s.ends_with(")") {
            let value_str = &s[4..s.len() - 1];
            if let Ok(value) = value_str.parse::<i64>() {
                return Ok(Self::I64(value));
            }
        } else if s.starts_with("U8(") && s.ends_with(")") {
            let value_str = &s[3..s.len() - 1];
            if let Ok(value) = value_str.parse::<u8>() {
                return Ok(Self::U8(value));
            }
        } else if s.starts_with("U16(") && s.ends_with(")") {
            let value_str = &s[4..s.len() - 1];
            if let Ok(value) = value_str.parse::<u16>() {
                return Ok(Self::U16(value));
            }
        } else if s.starts_with("U32(") && s.ends_with(")") {
            let value_str = &s[4..s.len() - 1];
            if let Ok(value) = value_str.parse::<u32>() {
                return Ok(Self::U32(value));
            }
        } else if s.starts_with("U64(") && s.ends_with(")") {
            let value_str = &s[4..s.len() - 1];
            if let Ok(value) = value_str.parse::<u64>() {
                return Ok(Self::U64(value));
            }
        } else if s.starts_with("F32(") && s.ends_with(")") {
            let value_str = &s[4..s.len() - 1];
            if let Ok(value) = value_str.parse::<f32>() {
                return Ok(Self::F32(value));
            }
        } else if s.starts_with("F64(") && s.ends_with(")") {
            let value_str = &s[4..s.len() - 1];
            if let Ok(value) = value_str.parse::<f64>() {
                return Ok(Self::F64(value));
            }
        } else if s.starts_with("Bool(") && s.ends_with(")") {
            let value_str = &s[5..s.len() - 1];
            if let Ok(value) = value_str.parse::<bool>() {
                return Ok(Self::Bool(value));
            }
        } else if s.starts_with("String(") && s.ends_with(")") {
            let value_str = &s[7..s.len() - 1];
            return Ok(Self::String(value_str.to_string()));
        } else if s.starts_with("Uuid(") && s.ends_with(")") {
            let value_str = &s[5..s.len() - 1];
            if let Ok(uuid) = Uuid::parse_str(value_str) {
                return Ok(Self::Uuid(uuid));
            }
        }

        // Fall back to comprehensive type parsing for raw values
        // Try i8 first
        if let Ok(value) = s.parse::<i8>() {
            return Ok(Self::I8(value));
        }
        // Try i16
        if let Ok(value) = s.parse::<i16>() {
            return Ok(Self::I16(value));
        }
        // Try i32
        if let Ok(value) = s.parse::<i32>() {
            return Ok(Self::I32(value));
        }
        // Try i64
        if let Ok(value) = s.parse::<i64>() {
            return Ok(Self::I64(value));
        }
        // Try u8
        if let Ok(value) = s.parse::<u8>() {
            return Ok(Self::U8(value));
        }
        // Try u16
        if let Ok(value) = s.parse::<u16>() {
            return Ok(Self::U16(value));
        }
        // Try u32
        if let Ok(value) = s.parse::<u32>() {
            return Ok(Self::U32(value));
        }
        // Try u64
        if let Ok(value) = s.parse::<u64>() {
            return Ok(Self::U64(value));
        }
        // Try f32
        if let Ok(value) = s.parse::<f32>() {
            return Ok(Self::F32(value));
        }
        // Try f64
        if let Ok(value) = s.parse::<f64>() {
            return Ok(Self::F64(value));
        }
        // Try bool
        if let Ok(value) = s.parse::<bool>() {
            return Ok(Self::Bool(value));
        }
        // Try UUID
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
impl From<i8> for CausticsKey {
    fn from(value: i8) -> Self {
        Self::I8(value)
    }
}
impl From<i16> for CausticsKey {
    fn from(value: i16) -> Self {
        Self::I16(value)
    }
}
impl From<i32> for CausticsKey {
    fn from(value: i32) -> Self {
        Self::I32(value)
    }
}
impl From<i64> for CausticsKey {
    fn from(value: i64) -> Self {
        Self::I64(value)
    }
}
impl From<isize> for CausticsKey {
    fn from(value: isize) -> Self {
        Self::ISize(value)
    }
}
impl From<u8> for CausticsKey {
    fn from(value: u8) -> Self {
        Self::U8(value)
    }
}
impl From<u16> for CausticsKey {
    fn from(value: u16) -> Self {
        Self::U16(value)
    }
}
impl From<u32> for CausticsKey {
    fn from(value: u32) -> Self {
        Self::U32(value)
    }
}
impl From<u64> for CausticsKey {
    fn from(value: u64) -> Self {
        Self::U64(value)
    }
}
impl From<usize> for CausticsKey {
    fn from(value: usize) -> Self {
        Self::USize(value)
    }
}
impl From<f32> for CausticsKey {
    fn from(value: f32) -> Self {
        Self::F32(value)
    }
}
impl From<f64> for CausticsKey {
    fn from(value: f64) -> Self {
        Self::F64(value)
    }
}
impl From<bool> for CausticsKey {
    fn from(value: bool) -> Self {
        Self::Bool(value)
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
impl From<chrono::DateTime<chrono::Utc>> for CausticsKey {
    fn from(value: chrono::DateTime<chrono::Utc>) -> Self {
        Self::DateTimeUtc(value)
    }
}
impl From<chrono::NaiveDateTime> for CausticsKey {
    fn from(value: chrono::NaiveDateTime) -> Self {
        Self::NaiveDateTime(value)
    }
}
impl From<chrono::NaiveDate> for CausticsKey {
    fn from(value: chrono::NaiveDate) -> Self {
        Self::NaiveDate(value)
    }
}
impl From<chrono::NaiveTime> for CausticsKey {
    fn from(value: chrono::NaiveTime) -> Self {
        Self::NaiveTime(value)
    }
}
impl From<serde_json::Value> for CausticsKey {
    fn from(value: serde_json::Value) -> Self {
        Self::Json(value)
    }
}

// Try to convert to specific types
impl TryFrom<CausticsKey> for i8 {
    type Error = String;
    fn try_from(key: CausticsKey) -> Result<Self, Self::Error> {
        match key {
            CausticsKey::I8(value) => Ok(value),
            _ => Err(format!("Cannot convert {:?} to i8", key)),
        }
    }
}

impl TryFrom<CausticsKey> for i16 {
    type Error = String;
    fn try_from(key: CausticsKey) -> Result<Self, Self::Error> {
        match key {
            CausticsKey::I16(value) => Ok(value),
            _ => Err(format!("Cannot convert {:?} to i16", key)),
        }
    }
}

impl TryFrom<CausticsKey> for i32 {
    type Error = String;
    fn try_from(key: CausticsKey) -> Result<Self, Self::Error> {
        match key {
            CausticsKey::I32(value) => Ok(value),
            _ => Err(format!("Cannot convert {:?} to i32", key)),
        }
    }
}

impl TryFrom<CausticsKey> for i64 {
    type Error = String;
    fn try_from(key: CausticsKey) -> Result<Self, Self::Error> {
        match key {
            CausticsKey::I64(value) => Ok(value),
            _ => Err(format!("Cannot convert {:?} to i64", key)),
        }
    }
}

impl TryFrom<CausticsKey> for u8 {
    type Error = String;
    fn try_from(key: CausticsKey) -> Result<Self, Self::Error> {
        match key {
            CausticsKey::U8(value) => Ok(value),
            _ => Err(format!("Cannot convert {:?} to u8", key)),
        }
    }
}

impl TryFrom<CausticsKey> for u16 {
    type Error = String;
    fn try_from(key: CausticsKey) -> Result<Self, Self::Error> {
        match key {
            CausticsKey::U16(value) => Ok(value),
            _ => Err(format!("Cannot convert {:?} to u16", key)),
        }
    }
}

impl TryFrom<CausticsKey> for u32 {
    type Error = String;
    fn try_from(key: CausticsKey) -> Result<Self, Self::Error> {
        match key {
            CausticsKey::U32(value) => Ok(value),
            _ => Err(format!("Cannot convert {:?} to u32", key)),
        }
    }
}

impl TryFrom<CausticsKey> for u64 {
    type Error = String;
    fn try_from(key: CausticsKey) -> Result<Self, Self::Error> {
        match key {
            CausticsKey::U64(value) => Ok(value),
            _ => Err(format!("Cannot convert {:?} to u64", key)),
        }
    }
}

impl TryFrom<CausticsKey> for isize {
    type Error = String;
    fn try_from(key: CausticsKey) -> Result<Self, Self::Error> {
        match key {
            CausticsKey::ISize(value) => Ok(value),
            _ => Err(format!("Cannot convert {:?} to isize", key)),
        }
    }
}

impl TryFrom<CausticsKey> for usize {
    type Error = String;
    fn try_from(key: CausticsKey) -> Result<Self, Self::Error> {
        match key {
            CausticsKey::USize(value) => Ok(value),
            _ => Err(format!("Cannot convert {:?} to usize", key)),
        }
    }
}

impl TryFrom<CausticsKey> for f32 {
    type Error = String;
    fn try_from(key: CausticsKey) -> Result<Self, Self::Error> {
        match key {
            CausticsKey::F32(value) => Ok(value),
            _ => Err(format!("Cannot convert {:?} to f32", key)),
        }
    }
}

impl TryFrom<CausticsKey> for f64 {
    type Error = String;
    fn try_from(key: CausticsKey) -> Result<Self, Self::Error> {
        match key {
            CausticsKey::F64(value) => Ok(value),
            _ => Err(format!("Cannot convert {:?} to f64", key)),
        }
    }
}

impl TryFrom<CausticsKey> for bool {
    type Error = String;
    fn try_from(key: CausticsKey) -> Result<Self, Self::Error> {
        match key {
            CausticsKey::Bool(value) => Ok(value),
            _ => Err(format!("Cannot convert {:?} to bool", key)),
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

impl KeyConvertible for i8 {
    fn to_caustics_key(self) -> CausticsKey {
        CausticsKey::I8(self)
    }
    fn from_caustics_key(key: CausticsKey) -> Result<Self, String> {
        key.try_into()
    }
}
impl KeyConvertible for i16 {
    fn to_caustics_key(self) -> CausticsKey {
        CausticsKey::I16(self)
    }
    fn from_caustics_key(key: CausticsKey) -> Result<Self, String> {
        key.try_into()
    }
}
impl KeyConvertible for i32 {
    fn to_caustics_key(self) -> CausticsKey {
        CausticsKey::I32(self)
    }
    fn from_caustics_key(key: CausticsKey) -> Result<Self, String> {
        key.try_into()
    }
}
impl KeyConvertible for i64 {
    fn to_caustics_key(self) -> CausticsKey {
        CausticsKey::I64(self)
    }
    fn from_caustics_key(key: CausticsKey) -> Result<Self, String> {
        key.try_into()
    }
}
impl KeyConvertible for isize {
    fn to_caustics_key(self) -> CausticsKey {
        CausticsKey::ISize(self)
    }
    fn from_caustics_key(key: CausticsKey) -> Result<Self, String> {
        key.try_into()
    }
}
impl KeyConvertible for u8 {
    fn to_caustics_key(self) -> CausticsKey {
        CausticsKey::U8(self)
    }
    fn from_caustics_key(key: CausticsKey) -> Result<Self, String> {
        key.try_into()
    }
}
impl KeyConvertible for u16 {
    fn to_caustics_key(self) -> CausticsKey {
        CausticsKey::U16(self)
    }
    fn from_caustics_key(key: CausticsKey) -> Result<Self, String> {
        key.try_into()
    }
}
impl KeyConvertible for u32 {
    fn to_caustics_key(self) -> CausticsKey {
        CausticsKey::U32(self)
    }
    fn from_caustics_key(key: CausticsKey) -> Result<Self, String> {
        key.try_into()
    }
}
impl KeyConvertible for u64 {
    fn to_caustics_key(self) -> CausticsKey {
        CausticsKey::U64(self)
    }
    fn from_caustics_key(key: CausticsKey) -> Result<Self, String> {
        key.try_into()
    }
}
impl KeyConvertible for usize {
    fn to_caustics_key(self) -> CausticsKey {
        CausticsKey::USize(self)
    }
    fn from_caustics_key(key: CausticsKey) -> Result<Self, String> {
        key.try_into()
    }
}
impl KeyConvertible for f32 {
    fn to_caustics_key(self) -> CausticsKey {
        CausticsKey::F32(self)
    }
    fn from_caustics_key(key: CausticsKey) -> Result<Self, String> {
        key.try_into()
    }
}
impl KeyConvertible for f64 {
    fn to_caustics_key(self) -> CausticsKey {
        CausticsKey::F64(self)
    }
    fn from_caustics_key(key: CausticsKey) -> Result<Self, String> {
        key.try_into()
    }
}
impl KeyConvertible for bool {
    fn to_caustics_key(self) -> CausticsKey {
        CausticsKey::Bool(self)
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

/// Unified key conversion function that can convert any CausticsKey to any target type
/// This replaces the huge, duplicated conversion functions in build.rs
pub fn convert_key_to_type_from_string<T: 'static + Default + Send + Sync>(
    key: CausticsKey,
    target_type_str: &str,
) -> Box<dyn std::any::Any + Send + Sync> {
    match target_type_str {
        "i8" => match key {
            CausticsKey::I8(value) => Box::new(value),
            CausticsKey::I16(value) => Box::new(value as i8),
            CausticsKey::I32(value) => Box::new(value as i8),
            CausticsKey::I64(value) => Box::new(value as i8),
            CausticsKey::ISize(value) => Box::new(value as i8),
            CausticsKey::U8(value) => Box::new(value as i8),
            CausticsKey::U16(value) => Box::new(value as i8),
            CausticsKey::U32(value) => Box::new(value as i8),
            CausticsKey::U64(value) => Box::new(value as i8),
            CausticsKey::USize(value) => Box::new(value as i8),
            CausticsKey::F32(value) => Box::new(value as i8),
            CausticsKey::F64(value) => Box::new(value as i8),
            CausticsKey::String(value) => Box::new(value.parse::<i8>().unwrap_or(0)),
            CausticsKey::Bool(value) => Box::new(if value { 1i8 } else { 0i8 }),
            _ => Box::new(0i8),
        },
        "i16" => match key {
            CausticsKey::I8(value) => Box::new(value as i16),
            CausticsKey::I16(value) => Box::new(value),
            CausticsKey::I32(value) => Box::new(value as i16),
            CausticsKey::I64(value) => Box::new(value as i16),
            CausticsKey::ISize(value) => Box::new(value as i16),
            CausticsKey::U8(value) => Box::new(value as i16),
            CausticsKey::U16(value) => Box::new(value as i16),
            CausticsKey::U32(value) => Box::new(value as i16),
            CausticsKey::U64(value) => Box::new(value as i16),
            CausticsKey::USize(value) => Box::new(value as i16),
            CausticsKey::F32(value) => Box::new(value as i16),
            CausticsKey::F64(value) => Box::new(value as i16),
            CausticsKey::String(value) => Box::new(value.parse::<i16>().unwrap_or(0)),
            CausticsKey::Bool(value) => Box::new(if value { 1i16 } else { 0i16 }),
            _ => Box::new(0i16),
        },
        "i32" => match key {
            CausticsKey::I8(value) => Box::new(value as i32),
            CausticsKey::I16(value) => Box::new(value as i32),
            CausticsKey::I32(value) => Box::new(value),
            CausticsKey::I64(value) => Box::new(value as i32),
            CausticsKey::ISize(value) => Box::new(value as i32),
            CausticsKey::U8(value) => Box::new(value as i32),
            CausticsKey::U16(value) => Box::new(value as i32),
            CausticsKey::U32(value) => Box::new(value as i32),
            CausticsKey::U64(value) => Box::new(value as i32),
            CausticsKey::USize(value) => Box::new(value as i32),
            CausticsKey::F32(value) => Box::new(value as i32),
            CausticsKey::F64(value) => Box::new(value as i32),
            CausticsKey::String(value) => Box::new(value.parse::<i32>().unwrap_or(0)),
            CausticsKey::Bool(value) => Box::new(if value { 1i32 } else { 0i32 }),
            _ => Box::new(0i32),
        },
        "i64" => match key {
            CausticsKey::I8(value) => Box::new(value as i64),
            CausticsKey::I16(value) => Box::new(value as i64),
            CausticsKey::I32(value) => Box::new(value as i64),
            CausticsKey::I64(value) => Box::new(value),
            CausticsKey::ISize(value) => Box::new(value as i64),
            CausticsKey::U8(value) => Box::new(value as i64),
            CausticsKey::U16(value) => Box::new(value as i64),
            CausticsKey::U32(value) => Box::new(value as i64),
            CausticsKey::U64(value) => Box::new(value as i64),
            CausticsKey::USize(value) => Box::new(value as i64),
            CausticsKey::F32(value) => Box::new(value as i64),
            CausticsKey::F64(value) => Box::new(value as i64),
            CausticsKey::String(value) => Box::new(value.parse::<i64>().unwrap_or(0)),
            CausticsKey::Bool(value) => Box::new(if value { 1i64 } else { 0i64 }),
            _ => Box::new(0i64),
        },
        "isize" => match key {
            CausticsKey::I8(value) => Box::new(value as isize),
            CausticsKey::I16(value) => Box::new(value as isize),
            CausticsKey::I32(value) => Box::new(value as isize),
            CausticsKey::I64(value) => Box::new(value as isize),
            CausticsKey::ISize(value) => Box::new(value),
            CausticsKey::U8(value) => Box::new(value as isize),
            CausticsKey::U16(value) => Box::new(value as isize),
            CausticsKey::U32(value) => Box::new(value as isize),
            CausticsKey::U64(value) => Box::new(value as isize),
            CausticsKey::USize(value) => Box::new(value as isize),
            CausticsKey::F32(value) => Box::new(value as isize),
            CausticsKey::F64(value) => Box::new(value as isize),
            CausticsKey::String(value) => Box::new(value.parse::<isize>().unwrap_or(0)),
            CausticsKey::Bool(value) => Box::new(if value { 1isize } else { 0isize }),
            _ => Box::new(0isize),
        },
        "u8" => match key {
            CausticsKey::I8(value) => Box::new(value as u8),
            CausticsKey::I16(value) => Box::new(value as u8),
            CausticsKey::I32(value) => Box::new(value as u8),
            CausticsKey::I64(value) => Box::new(value as u8),
            CausticsKey::ISize(value) => Box::new(value as u8),
            CausticsKey::U8(value) => Box::new(value),
            CausticsKey::U16(value) => Box::new(value as u8),
            CausticsKey::U32(value) => Box::new(value as u8),
            CausticsKey::U64(value) => Box::new(value as u8),
            CausticsKey::USize(value) => Box::new(value as u8),
            CausticsKey::F32(value) => Box::new(value as u8),
            CausticsKey::F64(value) => Box::new(value as u8),
            CausticsKey::String(value) => Box::new(value.parse::<u8>().unwrap_or(0)),
            CausticsKey::Bool(value) => Box::new(if value { 1u8 } else { 0u8 }),
            _ => Box::new(0u8),
        },
        "u16" => match key {
            CausticsKey::I8(value) => Box::new(value as u16),
            CausticsKey::I16(value) => Box::new(value as u16),
            CausticsKey::I32(value) => Box::new(value as u16),
            CausticsKey::I64(value) => Box::new(value as u16),
            CausticsKey::ISize(value) => Box::new(value as u16),
            CausticsKey::U8(value) => Box::new(value as u16),
            CausticsKey::U16(value) => Box::new(value),
            CausticsKey::U32(value) => Box::new(value as u16),
            CausticsKey::U64(value) => Box::new(value as u16),
            CausticsKey::USize(value) => Box::new(value as u16),
            CausticsKey::F32(value) => Box::new(value as u16),
            CausticsKey::F64(value) => Box::new(value as u16),
            CausticsKey::String(value) => Box::new(value.parse::<u16>().unwrap_or(0)),
            CausticsKey::Bool(value) => Box::new(if value { 1u16 } else { 0u16 }),
            _ => Box::new(0u16),
        },
        "u32" => match key {
            CausticsKey::I8(value) => Box::new(value as u32),
            CausticsKey::I16(value) => Box::new(value as u32),
            CausticsKey::I32(value) => Box::new(value as u32),
            CausticsKey::I64(value) => Box::new(value as u32),
            CausticsKey::ISize(value) => Box::new(value as u32),
            CausticsKey::U8(value) => Box::new(value as u32),
            CausticsKey::U16(value) => Box::new(value as u32),
            CausticsKey::U32(value) => Box::new(value),
            CausticsKey::U64(value) => Box::new(value as u32),
            CausticsKey::USize(value) => Box::new(value as u32),
            CausticsKey::F32(value) => Box::new(value as u32),
            CausticsKey::F64(value) => Box::new(value as u32),
            CausticsKey::String(value) => Box::new(value.parse::<u32>().unwrap_or(0)),
            CausticsKey::Bool(value) => Box::new(if value { 1u32 } else { 0u32 }),
            _ => Box::new(0u32),
        },
        "u64" => match key {
            CausticsKey::I8(value) => Box::new(value as u64),
            CausticsKey::I16(value) => Box::new(value as u64),
            CausticsKey::I32(value) => Box::new(value as u64),
            CausticsKey::I64(value) => Box::new(value as u64),
            CausticsKey::ISize(value) => Box::new(value as u64),
            CausticsKey::U8(value) => Box::new(value as u64),
            CausticsKey::U16(value) => Box::new(value as u64),
            CausticsKey::U32(value) => Box::new(value as u64),
            CausticsKey::U64(value) => Box::new(value),
            CausticsKey::USize(value) => Box::new(value as u64),
            CausticsKey::F32(value) => Box::new(value as u64),
            CausticsKey::F64(value) => Box::new(value as u64),
            CausticsKey::String(value) => Box::new(value.parse::<u64>().unwrap_or(0)),
            CausticsKey::Bool(value) => Box::new(if value { 1u64 } else { 0u64 }),
            _ => Box::new(0u64),
        },
        "usize" => match key {
            CausticsKey::I8(value) => Box::new(value as usize),
            CausticsKey::I16(value) => Box::new(value as usize),
            CausticsKey::I32(value) => Box::new(value as usize),
            CausticsKey::I64(value) => Box::new(value as usize),
            CausticsKey::ISize(value) => Box::new(value as usize),
            CausticsKey::U8(value) => Box::new(value as usize),
            CausticsKey::U16(value) => Box::new(value as usize),
            CausticsKey::U32(value) => Box::new(value as usize),
            CausticsKey::U64(value) => Box::new(value as usize),
            CausticsKey::USize(value) => Box::new(value),
            CausticsKey::F32(value) => Box::new(value as usize),
            CausticsKey::F64(value) => Box::new(value as usize),
            CausticsKey::String(value) => Box::new(value.parse::<usize>().unwrap_or(0)),
            CausticsKey::Bool(value) => Box::new(if value { 1usize } else { 0usize }),
            _ => Box::new(0usize),
        },
        "f32" => match key {
            CausticsKey::I8(value) => Box::new(value as f32),
            CausticsKey::I16(value) => Box::new(value as f32),
            CausticsKey::I32(value) => Box::new(value as f32),
            CausticsKey::I64(value) => Box::new(value as f32),
            CausticsKey::ISize(value) => Box::new(value as f32),
            CausticsKey::U8(value) => Box::new(value as f32),
            CausticsKey::U16(value) => Box::new(value as f32),
            CausticsKey::U32(value) => Box::new(value as f32),
            CausticsKey::U64(value) => Box::new(value as f32),
            CausticsKey::USize(value) => Box::new(value as f32),
            CausticsKey::F32(value) => Box::new(value),
            CausticsKey::F64(value) => Box::new(value as f32),
            CausticsKey::String(value) => Box::new(value.parse::<f32>().unwrap_or(0.0)),
            CausticsKey::Bool(value) => Box::new(if value { 1.0f32 } else { 0.0f32 }),
            _ => Box::new(0.0f32),
        },
        "f64" => match key {
            CausticsKey::I8(value) => Box::new(value as f64),
            CausticsKey::I16(value) => Box::new(value as f64),
            CausticsKey::I32(value) => Box::new(value as f64),
            CausticsKey::I64(value) => Box::new(value as f64),
            CausticsKey::ISize(value) => Box::new(value as f64),
            CausticsKey::U8(value) => Box::new(value as f64),
            CausticsKey::U16(value) => Box::new(value as f64),
            CausticsKey::U32(value) => Box::new(value as f64),
            CausticsKey::U64(value) => Box::new(value as f64),
            CausticsKey::USize(value) => Box::new(value as f64),
            CausticsKey::F32(value) => Box::new(value as f64),
            CausticsKey::F64(value) => Box::new(value),
            CausticsKey::String(value) => Box::new(value.parse::<f64>().unwrap_or(0.0)),
            CausticsKey::Bool(value) => Box::new(if value { 1.0f64 } else { 0.0f64 }),
            _ => Box::new(0.0f64),
        },
        "String" | "str" => match key {
            CausticsKey::String(value) => Box::new(value),
            CausticsKey::I8(value) => Box::new(value.to_string()),
            CausticsKey::I16(value) => Box::new(value.to_string()),
            CausticsKey::I32(value) => Box::new(value.to_string()),
            CausticsKey::I64(value) => Box::new(value.to_string()),
            CausticsKey::ISize(value) => Box::new(value.to_string()),
            CausticsKey::U8(value) => Box::new(value.to_string()),
            CausticsKey::U16(value) => Box::new(value.to_string()),
            CausticsKey::U32(value) => Box::new(value.to_string()),
            CausticsKey::U64(value) => Box::new(value.to_string()),
            CausticsKey::USize(value) => Box::new(value.to_string()),
            CausticsKey::F32(value) => Box::new(value.to_string()),
            CausticsKey::F64(value) => Box::new(value.to_string()),
            CausticsKey::Bool(value) => Box::new(value.to_string()),
            CausticsKey::Uuid(value) => Box::new(value.to_string()),
            CausticsKey::DateTimeUtc(value) => Box::new(value.to_rfc3339()),
            CausticsKey::NaiveDateTime(value) => {
                Box::new(value.format("%Y-%m-%d %H:%M:%S").to_string())
            }
            CausticsKey::NaiveDate(value) => Box::new(value.format("%Y-%m-%d").to_string()),
            CausticsKey::NaiveTime(value) => Box::new(value.format("%H:%M:%S").to_string()),
            CausticsKey::Json(value) => Box::new(value.to_string()),
            // Composite keys - convert to string representation
            CausticsKey::Composite(fields) => {
                let field_strings: Vec<String> = fields.iter()
                    .map(|(name, key)| format!("{}:{}", name, key))
                    .collect();
                Box::new(format!("Composite({})", field_strings.join(", ")))
            }
            CausticsKey::OptionalComposite(Some(fields)) => {
                let field_strings: Vec<String> = fields.iter()
                    .map(|(name, key)| format!("{}:{}", name, key))
                    .collect();
                Box::new(format!("OptionalComposite({})", field_strings.join(", ")))
            }
            CausticsKey::OptionalComposite(None) => Box::new("OptionalComposite(None)".to_string()),
        },
        "bool" => match key {
            CausticsKey::Bool(value) => Box::new(value),
            CausticsKey::I8(value) => Box::new(value != 0),
            CausticsKey::I16(value) => Box::new(value != 0),
            CausticsKey::I32(value) => Box::new(value != 0),
            CausticsKey::I64(value) => Box::new(value != 0),
            CausticsKey::ISize(value) => Box::new(value != 0),
            CausticsKey::U8(value) => Box::new(value != 0),
            CausticsKey::U16(value) => Box::new(value != 0),
            CausticsKey::U32(value) => Box::new(value != 0),
            CausticsKey::U64(value) => Box::new(value != 0),
            CausticsKey::USize(value) => Box::new(value != 0),
            CausticsKey::F32(value) => Box::new(value != 0.0),
            CausticsKey::F64(value) => Box::new(value != 0.0),
            CausticsKey::String(value) => Box::new(value.parse::<bool>().unwrap_or(false)),
            _ => Box::new(false),
        },
        "uuid::Uuid" | "Uuid" => match key {
            CausticsKey::Uuid(value) => Box::new(value),
            CausticsKey::String(value) => {
                Box::new(value.parse::<Uuid>().unwrap_or_else(|_| Uuid::new_v4()))
            }
            _ => Box::new(Uuid::new_v4()),
        },
        "chrono::DateTime<chrono::Utc>" | "caustics::chrono::DateTime<caustics::chrono::Utc>" => match key {
            CausticsKey::DateTimeUtc(value) => Box::new(value),
            CausticsKey::String(value) => Box::new(
                value
                    .parse::<chrono::DateTime<chrono::Utc>>()
                    .unwrap_or_else(|_| chrono::Utc::now()),
            ),
            _ => Box::new(chrono::Utc::now()),
        },
        "chrono::NaiveDateTime" | "caustics::chrono::NaiveDateTime" => match key {
            CausticsKey::NaiveDateTime(value) => Box::new(value),
            CausticsKey::String(value) => {
                Box::new(value.parse::<chrono::NaiveDateTime>().unwrap_or_else(|_| {
                    chrono::DateTime::from_timestamp(0, 0).unwrap().naive_utc()
                }))
            }
            _ => Box::new(chrono::DateTime::from_timestamp(0, 0).unwrap().naive_utc()),
        },
        "chrono::NaiveDate" | "caustics::chrono::NaiveDate" => match key {
            CausticsKey::NaiveDate(value) => Box::new(value),
            CausticsKey::String(value) => Box::new(
                value
                    .parse::<chrono::NaiveDate>()
                    .unwrap_or_else(|_| chrono::NaiveDate::from_ymd_opt(1970, 1, 1).unwrap()),
            ),
            _ => Box::new(chrono::NaiveDate::from_ymd_opt(1970, 1, 1).unwrap()),
        },
        "chrono::NaiveTime" | "caustics::chrono::NaiveTime" => match key {
            CausticsKey::NaiveTime(value) => Box::new(value),
            CausticsKey::String(value) => Box::new(
                value
                    .parse::<chrono::NaiveTime>()
                    .unwrap_or_else(|_| chrono::NaiveTime::from_hms_opt(0, 0, 0).unwrap()),
            ),
            _ => Box::new(chrono::NaiveTime::from_hms_opt(0, 0, 0).unwrap()),
        },
        "serde_json::Value" | "caustics::serde_json::Value" => match key {
            CausticsKey::Json(value) => Box::new(value),
            CausticsKey::String(value) => Box::new(
                serde_json::from_str::<serde_json::Value>(&value)
                    .unwrap_or(serde_json::Value::Null),
            ),
            CausticsKey::I8(value) => {
                Box::new(serde_json::Value::Number(serde_json::Number::from(value)))
            }
            CausticsKey::I16(value) => {
                Box::new(serde_json::Value::Number(serde_json::Number::from(value)))
            }
            CausticsKey::I32(value) => {
                Box::new(serde_json::Value::Number(serde_json::Number::from(value)))
            }
            CausticsKey::I64(value) => {
                Box::new(serde_json::Value::Number(serde_json::Number::from(value)))
            }
            CausticsKey::U8(value) => {
                Box::new(serde_json::Value::Number(serde_json::Number::from(value)))
            }
            CausticsKey::U16(value) => {
                Box::new(serde_json::Value::Number(serde_json::Number::from(value)))
            }
            CausticsKey::U32(value) => {
                Box::new(serde_json::Value::Number(serde_json::Number::from(value)))
            }
            CausticsKey::U64(value) => {
                Box::new(serde_json::Value::Number(serde_json::Number::from(value)))
            }
            CausticsKey::F32(value) => Box::new(serde_json::Value::Number(
                serde_json::Number::from_f64(value as f64).unwrap_or(serde_json::Number::from(0)),
            )),
            CausticsKey::F64(value) => Box::new(serde_json::Value::Number(
                serde_json::Number::from_f64(value).unwrap_or(serde_json::Number::from(0)),
            )),
            CausticsKey::Bool(value) => Box::new(serde_json::Value::Bool(value)),
            CausticsKey::Uuid(value) => Box::new(serde_json::Value::String(value.to_string())),
            _ => Box::new(serde_json::Value::Null),
        },
        _ => {
            // Unknown type, try to convert to the most appropriate type
            match key {
                CausticsKey::String(value) => Box::new(value),
                CausticsKey::I32(value) => Box::new(value),
                CausticsKey::I64(value) => Box::new(value),
                CausticsKey::U32(value) => Box::new(value),
                CausticsKey::U64(value) => Box::new(value),
                CausticsKey::F64(value) => Box::new(value),
                CausticsKey::Bool(value) => Box::new(value),
                CausticsKey::Uuid(value) => Box::new(value),
                _ => Box::new(0i32), // Default fallback
            }
        }
    }
}

pub fn convert_key_to_type<T: 'static + Default + Send + Sync>(
    key: CausticsKey,
    target_type_id: std::any::TypeId,
) -> Box<dyn std::any::Any + Send + Sync> {
    match target_type_id {
        // Integer types
        type_id if type_id == std::any::TypeId::of::<i8>() => match key {
            CausticsKey::I8(value) => Box::new(value),
            CausticsKey::I16(value) => Box::new(value as i8),
            CausticsKey::I32(value) => Box::new(value as i8),
            CausticsKey::I64(value) => Box::new(value as i8),
            CausticsKey::ISize(value) => Box::new(value as i8),
            CausticsKey::U8(value) => Box::new(value as i8),
            CausticsKey::U16(value) => Box::new(value as i8),
            CausticsKey::U32(value) => Box::new(value as i8),
            CausticsKey::U64(value) => Box::new(value as i8),
            CausticsKey::USize(value) => Box::new(value as i8),
            CausticsKey::F32(value) => Box::new(value as i8),
            CausticsKey::F64(value) => Box::new(value as i8),
            CausticsKey::String(value) => Box::new(value.parse::<i8>().unwrap_or(0)),
            CausticsKey::Bool(value) => Box::new(if value { 1i8 } else { 0i8 }),
            _ => Box::new(0i8),
        },
        type_id if type_id == std::any::TypeId::of::<i16>() => match key {
            CausticsKey::I8(value) => Box::new(value as i16),
            CausticsKey::I16(value) => Box::new(value),
            CausticsKey::I32(value) => Box::new(value as i16),
            CausticsKey::I64(value) => Box::new(value as i16),
            CausticsKey::ISize(value) => Box::new(value as i16),
            CausticsKey::U8(value) => Box::new(value as i16),
            CausticsKey::U16(value) => Box::new(value as i16),
            CausticsKey::U32(value) => Box::new(value as i16),
            CausticsKey::U64(value) => Box::new(value as i16),
            CausticsKey::USize(value) => Box::new(value as i16),
            CausticsKey::F32(value) => Box::new(value as i16),
            CausticsKey::F64(value) => Box::new(value as i16),
            CausticsKey::String(value) => Box::new(value.parse::<i16>().unwrap_or(0)),
            CausticsKey::Bool(value) => Box::new(if value { 1i16 } else { 0i16 }),
            _ => Box::new(0i16),
        },
        type_id if type_id == std::any::TypeId::of::<i32>() => match key {
            CausticsKey::I8(value) => Box::new(value as i32),
            CausticsKey::I16(value) => Box::new(value as i32),
            CausticsKey::I32(value) => Box::new(value),
            CausticsKey::I64(value) => Box::new(value as i32),
            CausticsKey::ISize(value) => Box::new(value as i32),
            CausticsKey::U8(value) => Box::new(value as i32),
            CausticsKey::U16(value) => Box::new(value as i32),
            CausticsKey::U32(value) => Box::new(value as i32),
            CausticsKey::U64(value) => Box::new(value as i32),
            CausticsKey::USize(value) => Box::new(value as i32),
            CausticsKey::F32(value) => Box::new(value as i32),
            CausticsKey::F64(value) => Box::new(value as i32),
            CausticsKey::String(value) => Box::new(value.parse::<i32>().unwrap_or(0)),
            CausticsKey::Bool(value) => Box::new(if value { 1i32 } else { 0i32 }),
            _ => Box::new(0i32),
        },
        type_id if type_id == std::any::TypeId::of::<i64>() => match key {
            CausticsKey::I8(value) => Box::new(value as i64),
            CausticsKey::I16(value) => Box::new(value as i64),
            CausticsKey::I32(value) => Box::new(value as i64),
            CausticsKey::I64(value) => Box::new(value),
            CausticsKey::ISize(value) => Box::new(value as i64),
            CausticsKey::U8(value) => Box::new(value as i64),
            CausticsKey::U16(value) => Box::new(value as i64),
            CausticsKey::U32(value) => Box::new(value as i64),
            CausticsKey::U64(value) => Box::new(value as i64),
            CausticsKey::USize(value) => Box::new(value as i64),
            CausticsKey::F32(value) => Box::new(value as i64),
            CausticsKey::F64(value) => Box::new(value as i64),
            CausticsKey::String(value) => Box::new(value.parse::<i64>().unwrap_or(0)),
            CausticsKey::Bool(value) => Box::new(if value { 1i64 } else { 0i64 }),
            _ => Box::new(0i64),
        },
        type_id if type_id == std::any::TypeId::of::<isize>() => match key {
            CausticsKey::I8(value) => Box::new(value as isize),
            CausticsKey::I16(value) => Box::new(value as isize),
            CausticsKey::I32(value) => Box::new(value as isize),
            CausticsKey::I64(value) => Box::new(value as isize),
            CausticsKey::ISize(value) => Box::new(value),
            CausticsKey::U8(value) => Box::new(value as isize),
            CausticsKey::U16(value) => Box::new(value as isize),
            CausticsKey::U32(value) => Box::new(value as isize),
            CausticsKey::U64(value) => Box::new(value as isize),
            CausticsKey::USize(value) => Box::new(value as isize),
            CausticsKey::F32(value) => Box::new(value as isize),
            CausticsKey::F64(value) => Box::new(value as isize),
            CausticsKey::String(value) => Box::new(value.parse::<isize>().unwrap_or(0)),
            CausticsKey::Bool(value) => Box::new(if value { 1isize } else { 0isize }),
            _ => Box::new(0isize),
        },
        // Unsigned integer types
        type_id if type_id == std::any::TypeId::of::<u8>() => match key {
            CausticsKey::I8(value) => Box::new(value as u8),
            CausticsKey::I16(value) => Box::new(value as u8),
            CausticsKey::I32(value) => Box::new(value as u8),
            CausticsKey::I64(value) => Box::new(value as u8),
            CausticsKey::ISize(value) => Box::new(value as u8),
            CausticsKey::U8(value) => Box::new(value),
            CausticsKey::U16(value) => Box::new(value as u8),
            CausticsKey::U32(value) => Box::new(value as u8),
            CausticsKey::U64(value) => Box::new(value as u8),
            CausticsKey::USize(value) => Box::new(value as u8),
            CausticsKey::F32(value) => Box::new(value as u8),
            CausticsKey::F64(value) => Box::new(value as u8),
            CausticsKey::String(value) => Box::new(value.parse::<u8>().unwrap_or(0)),
            CausticsKey::Bool(value) => Box::new(if value { 1u8 } else { 0u8 }),
            _ => Box::new(0u8),
        },
        type_id if type_id == std::any::TypeId::of::<u16>() => match key {
            CausticsKey::I8(value) => Box::new(value as u16),
            CausticsKey::I16(value) => Box::new(value as u16),
            CausticsKey::I32(value) => Box::new(value as u16),
            CausticsKey::I64(value) => Box::new(value as u16),
            CausticsKey::ISize(value) => Box::new(value as u16),
            CausticsKey::U8(value) => Box::new(value as u16),
            CausticsKey::U16(value) => Box::new(value),
            CausticsKey::U32(value) => Box::new(value as u16),
            CausticsKey::U64(value) => Box::new(value as u16),
            CausticsKey::USize(value) => Box::new(value as u16),
            CausticsKey::F32(value) => Box::new(value as u16),
            CausticsKey::F64(value) => Box::new(value as u16),
            CausticsKey::String(value) => Box::new(value.parse::<u16>().unwrap_or(0)),
            CausticsKey::Bool(value) => Box::new(if value { 1u16 } else { 0u16 }),
            _ => Box::new(0u16),
        },
        type_id if type_id == std::any::TypeId::of::<u32>() => match key {
            CausticsKey::I8(value) => Box::new(value as u32),
            CausticsKey::I16(value) => Box::new(value as u32),
            CausticsKey::I32(value) => Box::new(value as u32),
            CausticsKey::I64(value) => Box::new(value as u32),
            CausticsKey::ISize(value) => Box::new(value as u32),
            CausticsKey::U8(value) => Box::new(value as u32),
            CausticsKey::U16(value) => Box::new(value as u32),
            CausticsKey::U32(value) => Box::new(value),
            CausticsKey::U64(value) => Box::new(value as u32),
            CausticsKey::USize(value) => Box::new(value as u32),
            CausticsKey::F32(value) => Box::new(value as u32),
            CausticsKey::F64(value) => Box::new(value as u32),
            CausticsKey::String(value) => Box::new(value.parse::<u32>().unwrap_or(0)),
            CausticsKey::Bool(value) => Box::new(if value { 1u32 } else { 0u32 }),
            _ => Box::new(0u32),
        },
        type_id if type_id == std::any::TypeId::of::<u64>() => match key {
            CausticsKey::I8(value) => Box::new(value as u64),
            CausticsKey::I16(value) => Box::new(value as u64),
            CausticsKey::I32(value) => Box::new(value as u64),
            CausticsKey::I64(value) => Box::new(value as u64),
            CausticsKey::ISize(value) => Box::new(value as u64),
            CausticsKey::U8(value) => Box::new(value as u64),
            CausticsKey::U16(value) => Box::new(value as u64),
            CausticsKey::U32(value) => Box::new(value as u64),
            CausticsKey::U64(value) => Box::new(value),
            CausticsKey::USize(value) => Box::new(value as u64),
            CausticsKey::F32(value) => Box::new(value as u64),
            CausticsKey::F64(value) => Box::new(value as u64),
            CausticsKey::String(value) => Box::new(value.parse::<u64>().unwrap_or(0)),
            CausticsKey::Bool(value) => Box::new(if value { 1u64 } else { 0u64 }),
            _ => Box::new(0u64),
        },
        type_id if type_id == std::any::TypeId::of::<usize>() => match key {
            CausticsKey::I8(value) => Box::new(value as usize),
            CausticsKey::I16(value) => Box::new(value as usize),
            CausticsKey::I32(value) => Box::new(value as usize),
            CausticsKey::I64(value) => Box::new(value as usize),
            CausticsKey::ISize(value) => Box::new(value as usize),
            CausticsKey::U8(value) => Box::new(value as usize),
            CausticsKey::U16(value) => Box::new(value as usize),
            CausticsKey::U32(value) => Box::new(value as usize),
            CausticsKey::U64(value) => Box::new(value as usize),
            CausticsKey::USize(value) => Box::new(value),
            CausticsKey::F32(value) => Box::new(value as usize),
            CausticsKey::F64(value) => Box::new(value as usize),
            CausticsKey::String(value) => Box::new(value.parse::<usize>().unwrap_or(0)),
            CausticsKey::Bool(value) => Box::new(if value { 1usize } else { 0usize }),
            _ => Box::new(0usize),
        },
        // Floating point types
        type_id if type_id == std::any::TypeId::of::<f32>() => match key {
            CausticsKey::I8(value) => Box::new(value as f32),
            CausticsKey::I16(value) => Box::new(value as f32),
            CausticsKey::I32(value) => Box::new(value as f32),
            CausticsKey::I64(value) => Box::new(value as f32),
            CausticsKey::ISize(value) => Box::new(value as f32),
            CausticsKey::U8(value) => Box::new(value as f32),
            CausticsKey::U16(value) => Box::new(value as f32),
            CausticsKey::U32(value) => Box::new(value as f32),
            CausticsKey::U64(value) => Box::new(value as f32),
            CausticsKey::USize(value) => Box::new(value as f32),
            CausticsKey::F32(value) => Box::new(value),
            CausticsKey::F64(value) => Box::new(value as f32),
            CausticsKey::String(value) => Box::new(value.parse::<f32>().unwrap_or(0.0)),
            CausticsKey::Bool(value) => Box::new(if value { 1.0f32 } else { 0.0f32 }),
            _ => Box::new(0.0f32),
        },
        type_id if type_id == std::any::TypeId::of::<f64>() => match key {
            CausticsKey::I8(value) => Box::new(value as f64),
            CausticsKey::I16(value) => Box::new(value as f64),
            CausticsKey::I32(value) => Box::new(value as f64),
            CausticsKey::I64(value) => Box::new(value as f64),
            CausticsKey::ISize(value) => Box::new(value as f64),
            CausticsKey::U8(value) => Box::new(value as f64),
            CausticsKey::U16(value) => Box::new(value as f64),
            CausticsKey::U32(value) => Box::new(value as f64),
            CausticsKey::U64(value) => Box::new(value as f64),
            CausticsKey::USize(value) => Box::new(value as f64),
            CausticsKey::F32(value) => Box::new(value as f64),
            CausticsKey::F64(value) => Box::new(value),
            CausticsKey::String(value) => Box::new(value.parse::<f64>().unwrap_or(0.0)),
            CausticsKey::Bool(value) => Box::new(if value { 1.0f64 } else { 0.0f64 }),
            _ => Box::new(0.0f64),
        },
        // String type
        type_id if type_id == std::any::TypeId::of::<String>() => match key {
            CausticsKey::String(value) => Box::new(value),
            CausticsKey::I8(value) => Box::new(value.to_string()),
            CausticsKey::I16(value) => Box::new(value.to_string()),
            CausticsKey::I32(value) => Box::new(value.to_string()),
            CausticsKey::I64(value) => Box::new(value.to_string()),
            CausticsKey::ISize(value) => Box::new(value.to_string()),
            CausticsKey::U8(value) => Box::new(value.to_string()),
            CausticsKey::U16(value) => Box::new(value.to_string()),
            CausticsKey::U32(value) => Box::new(value.to_string()),
            CausticsKey::U64(value) => Box::new(value.to_string()),
            CausticsKey::USize(value) => Box::new(value.to_string()),
            CausticsKey::F32(value) => Box::new(value.to_string()),
            CausticsKey::F64(value) => Box::new(value.to_string()),
            CausticsKey::Bool(value) => Box::new(value.to_string()),
            CausticsKey::Uuid(value) => Box::new(value.to_string()),
            CausticsKey::DateTimeUtc(value) => Box::new(value.to_rfc3339()),
            CausticsKey::NaiveDateTime(value) => Box::new(value.to_string()),
            CausticsKey::NaiveDate(value) => Box::new(value.to_string()),
            CausticsKey::NaiveTime(value) => Box::new(value.to_string()),
            CausticsKey::Json(value) => Box::new(value.to_string()),
            // Composite keys - convert to string representation
            CausticsKey::Composite(fields) => {
                let field_strings: Vec<String> = fields.iter()
                    .map(|(name, key)| format!("{}:{}", name, key))
                    .collect();
                Box::new(format!("Composite({})", field_strings.join(", ")))
            }
            CausticsKey::OptionalComposite(Some(fields)) => {
                let field_strings: Vec<String> = fields.iter()
                    .map(|(name, key)| format!("{}:{}", name, key))
                    .collect();
                Box::new(format!("OptionalComposite({})", field_strings.join(", ")))
            }
            CausticsKey::OptionalComposite(None) => Box::new("OptionalComposite(None)".to_string()),
        },
        // Boolean type
        type_id if type_id == std::any::TypeId::of::<bool>() => match key {
            CausticsKey::Bool(value) => Box::new(value),
            CausticsKey::I8(value) => Box::new(value != 0),
            CausticsKey::I16(value) => Box::new(value != 0),
            CausticsKey::I32(value) => Box::new(value != 0),
            CausticsKey::I64(value) => Box::new(value != 0),
            CausticsKey::ISize(value) => Box::new(value != 0),
            CausticsKey::U8(value) => Box::new(value != 0),
            CausticsKey::U16(value) => Box::new(value != 0),
            CausticsKey::U32(value) => Box::new(value != 0),
            CausticsKey::U64(value) => Box::new(value != 0),
            CausticsKey::USize(value) => Box::new(value != 0),
            CausticsKey::F32(value) => Box::new(value != 0.0),
            CausticsKey::F64(value) => Box::new(value != 0.0),
            CausticsKey::String(value) => Box::new(value.parse::<bool>().unwrap_or(false)),
            _ => Box::new(false),
        },
        // UUID type
        type_id if type_id == std::any::TypeId::of::<uuid::Uuid>() => match key {
            CausticsKey::Uuid(value) => Box::new(value),
            CausticsKey::String(value) => {
                Box::new(value.parse::<uuid::Uuid>().unwrap_or(uuid::Uuid::nil()))
            }
            _ => Box::new(uuid::Uuid::nil()),
        },
        // DateTime types
        type_id if type_id == std::any::TypeId::of::<chrono::DateTime<chrono::Utc>>() => {
            match key {
                CausticsKey::DateTimeUtc(value) => Box::new(value),
                CausticsKey::String(value) => Box::new(
                    value
                        .parse::<chrono::DateTime<chrono::Utc>>()
                        .unwrap_or_else(|_| chrono::Utc::now()),
                ),
                _ => Box::new(chrono::Utc::now()),
            }
        }
        type_id if type_id == std::any::TypeId::of::<chrono::NaiveDateTime>() => match key {
            CausticsKey::NaiveDateTime(value) => Box::new(value),
            CausticsKey::String(value) => {
                Box::new(value.parse::<chrono::NaiveDateTime>().unwrap_or_else(|_| {
                    chrono::DateTime::from_timestamp(0, 0).unwrap().naive_utc()
                }))
            }
            _ => Box::new(chrono::DateTime::from_timestamp(0, 0).unwrap().naive_utc()),
        },
        type_id if type_id == std::any::TypeId::of::<chrono::NaiveDate>() => match key {
            CausticsKey::NaiveDate(value) => Box::new(value),
            CausticsKey::String(value) => Box::new(
                value
                    .parse::<chrono::NaiveDate>()
                    .unwrap_or_else(|_| chrono::NaiveDate::from_ymd_opt(1970, 1, 1).unwrap()),
            ),
            _ => Box::new(chrono::NaiveDate::from_ymd_opt(1970, 1, 1).unwrap()),
        },
        type_id if type_id == std::any::TypeId::of::<chrono::NaiveTime>() => match key {
            CausticsKey::NaiveTime(value) => Box::new(value),
            CausticsKey::String(value) => Box::new(
                value
                    .parse::<chrono::NaiveTime>()
                    .unwrap_or_else(|_| chrono::NaiveTime::from_hms_opt(0, 0, 0).unwrap()),
            ),
            _ => Box::new(chrono::NaiveTime::from_hms_opt(0, 0, 0).unwrap()),
        },
        // JSON type
        type_id if type_id == std::any::TypeId::of::<serde_json::Value>() => match key {
            CausticsKey::Json(value) => Box::new(value),
            CausticsKey::String(value) => Box::new(
                serde_json::from_str::<serde_json::Value>(&value)
                    .unwrap_or(serde_json::Value::Null),
            ),
            CausticsKey::I8(value) => {
                Box::new(serde_json::Value::Number(serde_json::Number::from(value)))
            }
            CausticsKey::I16(value) => {
                Box::new(serde_json::Value::Number(serde_json::Number::from(value)))
            }
            CausticsKey::I32(value) => {
                Box::new(serde_json::Value::Number(serde_json::Number::from(value)))
            }
            CausticsKey::I64(value) => {
                Box::new(serde_json::Value::Number(serde_json::Number::from(value)))
            }
            CausticsKey::U8(value) => {
                Box::new(serde_json::Value::Number(serde_json::Number::from(value)))
            }
            CausticsKey::U16(value) => {
                Box::new(serde_json::Value::Number(serde_json::Number::from(value)))
            }
            CausticsKey::U32(value) => {
                Box::new(serde_json::Value::Number(serde_json::Number::from(value)))
            }
            CausticsKey::U64(value) => {
                Box::new(serde_json::Value::Number(serde_json::Number::from(value)))
            }
            CausticsKey::F32(value) => Box::new(serde_json::Value::Number(
                serde_json::Number::from_f64(value as f64).unwrap_or(serde_json::Number::from(0)),
            )),
            CausticsKey::F64(value) => Box::new(serde_json::Value::Number(
                serde_json::Number::from_f64(value).unwrap_or(serde_json::Number::from(0)),
            )),
            CausticsKey::Bool(value) => Box::new(serde_json::Value::Bool(value)),
            CausticsKey::Uuid(value) => Box::new(serde_json::Value::String(value.to_string())),
            _ => Box::new(serde_json::Value::Null),
        },
        _ => {
            // Unknown type, try to convert to the most appropriate type
            match key {
                CausticsKey::String(value) => Box::new(value),
                CausticsKey::I32(value) => Box::new(value),
                CausticsKey::I64(value) => Box::new(value),
                CausticsKey::U32(value) => Box::new(value),
                CausticsKey::U64(value) => Box::new(value),
                CausticsKey::F64(value) => Box::new(value),
                CausticsKey::Bool(value) => Box::new(value),
                CausticsKey::Uuid(value) => Box::new(value),
                _ => Box::new(0i32), // Default fallback
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_key_creation() {
        let int_key = CausticsKey::from_i32(42);
        assert_eq!(int_key, CausticsKey::I32(42));

        let string_key = CausticsKey::from_string("test".to_string());
        assert_eq!(string_key, CausticsKey::String("test".to_string()));

        let uuid = Uuid::new_v4();
        let uuid_key = CausticsKey::from_uuid(uuid);
        assert_eq!(uuid_key, CausticsKey::Uuid(uuid));
    }

    #[test]
    fn test_key_conversion() {
        let key: CausticsKey = 42i32.into();
        assert_eq!(key, CausticsKey::I32(42));

        let key: CausticsKey = "test".into();
        assert_eq!(key, CausticsKey::String("test".to_string()));

        let uuid = Uuid::new_v4();
        let key: CausticsKey = uuid.into();
        assert_eq!(key, CausticsKey::Uuid(uuid));
    }

    #[test]
    fn test_db_value_conversion() {
        let key = CausticsKey::I32(42);
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
        assert_eq!(key, CausticsKey::I32(42));

        let converted: i32 =
            i32::from_caustics_key(key).expect("Failed to convert CausticsKey to i32");
        assert_eq!(converted, 42);
    }
}
