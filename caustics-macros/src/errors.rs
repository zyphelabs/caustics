//! Error types for macro compilation failures
//! These provide clear, actionable error messages when configuration is missing

use proc_macro2::Span;
use syn::Error;

/// Macro compilation errors with clear guidance
#[derive(Debug, thiserror::Error)]
pub enum CausticsError {
    #[error("No primary key field found in entity '{entity_name}'.\n\nPlease add #[sea_orm(primary_key)] to a field or ensure a field is named 'id'.\n\nExample:\n    #[sea_orm(primary_key)]\n    user_id: i32,\n\nOr use the conventional 'id' field name:\n    id: i32,")]
    NoPrimaryKey { entity_name: String },

    #[error("Multiple primary key fields found in entity '{entity_name}'. Please specify exactly one primary key field.")]
    MultiplePrimaryKeys { entity_name: String },

    #[error("No foreign key column specified for relation '{relation_name}'.\n\nPlease add 'to' attribute with target column.\n\nExample:\n    #[sea_orm(\n        has_many = \"super::post::Entity\",\n        from = \"Column::UserId\",\n        to = \"super::post::Column::AuthorId\"\n    )]\n    posts: Vec<Post>,")]
    NoForeignKeyColumn { relation_name: String },

    #[error("No primary key field specified for relation '{relation_name}'.\n\nPlease add 'primary_key_field' attribute.\n\nExample:\n    #[sea_orm(\n        has_many = \"super::post::Entity\",\n        from = \"Column::UserId\",\n        to = \"super::post::Column::AuthorId\",\n        primary_key_field = \"post_id\"\n    )]\n    posts: Vec<Post>,")]
    NoRelationPrimaryKey { relation_name: String },

    #[error("No table name specified for entity '{entity_name}'.\n\nPlease add #[sea_orm(table_name = \"table_name\")] attribute.\n\nExample:\n    #[sea_orm(table_name = \"users\")]\n    #[derive(Caustics)]\n    struct User {{ ... }}")]
    NoTableName { entity_name: String },

    #[error("Invalid relation target '{target}' for relation '{relation_name}'. Target must be a valid entity path.")]
    InvalidRelationTarget {
        relation_name: String,
        target: String,
    },

    #[error("Unsupported field type '{type_name}' for field '{field_name}'. Supported types: String, i32, i64, bool, DateTime, Uuid, Option<T>.")]
    UnsupportedFieldType {
        field_name: String,
        type_name: String,
    },

    #[error("Missing required attribute '{attribute}' for field '{field_name}' in entity '{entity_name}'.")]
    MissingRequiredAttribute {
        entity_name: String,
        field_name: String,
        attribute: String,
    },

    #[error("Missing #[derive(Caustics)] on Relation enum for entity '{entity_name}'.\n\nPlease add #[derive(Caustics)] to your Relation enum.\n\nExample:\n    #[derive(Caustics, Copy, Clone, Debug, EnumIter, DeriveRelation)]\n    pub enum Relation {{\n        // your relations here\n    }}")]
    MissingCausticsOnRelation { entity_name: String },
}

impl CausticsError {
    /// Convert to syn::Error for compilation
    pub fn to_compile_error(&self, span: Span) -> proc_macro2::TokenStream {
        Error::new(span, self.to_string()).to_compile_error()
    }

    /// Create error for missing primary key
    pub fn no_primary_key(entity_name: &str, span: Span) -> proc_macro2::TokenStream {
        Self::NoPrimaryKey {
            entity_name: entity_name.to_string(),
        }
        .to_compile_error(span)
    }

    /// Create error for missing foreign key column
    pub fn no_foreign_key_column(relation_name: &str, span: Span) -> proc_macro2::TokenStream {
        Self::NoForeignKeyColumn {
            relation_name: relation_name.to_string(),
        }
        .to_compile_error(span)
    }

    /// Create error for missing relation primary key
    pub fn no_relation_primary_key(relation_name: &str, span: Span) -> proc_macro2::TokenStream {
        Self::NoRelationPrimaryKey {
            relation_name: relation_name.to_string(),
        }
        .to_compile_error(span)
    }

    /// Create error for missing table name
    pub fn no_table_name(entity_name: &str, span: Span) -> proc_macro2::TokenStream {
        Self::NoTableName {
            entity_name: entity_name.to_string(),
        }
        .to_compile_error(span)
    }

    /// Create error for missing Caustics derive on Relation enum
    pub fn missing_caustics_on_relation(entity_name: &str, span: Span) -> proc_macro2::TokenStream {
        Self::MissingCausticsOnRelation {
            entity_name: entity_name.to_string(),
        }
        .to_compile_error(span)
    }
}
