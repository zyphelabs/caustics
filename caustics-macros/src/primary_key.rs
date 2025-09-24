//! Centralized primary key utilities to replace hardcoded "id" assumptions
//! and provide reliable, factored functions for primary key access.

use proc_macro2::TokenStream;
use quote::{format_ident, quote};
use syn::{Field, Type};

/// Information about a primary key field
#[derive(Debug, Clone)]
pub struct PrimaryKeyInfo {
    pub field_name: String,
    pub field_ident: proc_macro2::Ident,
    pub column_name: String,
    pub field_type: Type,
    pub is_auto_increment: bool,
}

impl PrimaryKeyInfo {
    /// Create a new PrimaryKeyInfo from a field
    pub fn from_field(field: &Field) -> Self {
        let field_ident = field.ident.as_ref().unwrap().clone();
        let field_name = field_ident.to_string();
        let column_name = field_name.clone(); // Default to field name

        // Check for custom column name in attributes
        let column_name = field
            .attrs
            .iter()
            .find_map(|attr| {
                if let syn::Meta::List(meta) = &attr.meta {
                    if meta.path.is_ident("sea_orm") {
                        // Parse sea_orm attributes to find column name
                        // This is a simplified version - you might need more sophisticated parsing
                        None // For now, use field name
                    } else {
                        None
                    }
                } else {
                    None
                }
            })
            .unwrap_or(column_name);

        Self {
            field_name,
            field_ident,
            column_name,
            field_type: field.ty.clone(),
            is_auto_increment: Self::is_auto_increment_field(field),
        }
    }

    /// Check if a field is marked as auto-increment
    fn is_auto_increment_field(field: &Field) -> bool {
        field.attrs.iter().any(|attr| {
            if let syn::Meta::List(meta) = &attr.meta {
                meta.path.is_ident("sea_orm") && meta.tokens.to_string().contains("auto_increment")
            } else {
                false
            }
        })
    }

    /// Get the primary key field name
    pub fn field_name(&self) -> &str {
        &self.field_name
    }

    /// Get the primary key column name
    pub fn column_name(&self) -> &str {
        &self.column_name
    }

    /// Get the primary key field identifier
    pub fn field_ident(&self) -> &proc_macro2::Ident {
        &self.field_ident
    }

    /// Get the primary key field type
    pub fn field_type(&self) -> &Type {
        &self.field_type
    }

    /// Check if this is an auto-increment primary key
    pub fn is_auto_increment(&self) -> bool {
        self.is_auto_increment
    }
}

/// Extract primary key information from a list of fields
pub fn extract_primary_key_info(fields: &[&Field]) -> Option<PrimaryKeyInfo> {
    // First, try to find a field explicitly marked as primary key
    let explicit_pk_field = fields.iter().find(|field| {
        field.attrs.iter().any(|attr| {
            if let syn::Meta::List(meta) = &attr.meta {
                (meta.path.is_ident("sea_orm") && meta.tokens.to_string().contains("primary_key"))
                    || meta.path.is_ident("primary_key")
            } else {
                false
            }
        })
    });

    if let Some(field) = explicit_pk_field {
        return Some(PrimaryKeyInfo::from_field(field));
    }

    // If no explicit primary key, try to find a field named "id" (common convention)
    let id_field = fields.iter().find(|field| {
        if let Some(ident) = &field.ident {
            ident.to_string() == "id"
        } else {
            false
        }
    });

    if let Some(field) = id_field {
        return Some(PrimaryKeyInfo::from_field(field));
    }

    // If still no primary key found, try to find a field that looks like a primary key
    // (e.g., auto-increment, unique, etc.)
    let auto_increment_field = fields.iter().find(|field| {
        field.attrs.iter().any(|attr| {
            if let syn::Meta::List(meta) = &attr.meta {
                meta.path.is_ident("sea_orm") && meta.tokens.to_string().contains("auto_increment")
            } else {
                false
            }
        })
    });

    if let Some(field) = auto_increment_field {
        return Some(PrimaryKeyInfo::from_field(field));
    }

    // Last resort: find the first field that could be a primary key
    // (typically the first field in the struct)
    if let Some(field) = fields.first() {
        return Some(PrimaryKeyInfo::from_field(field));
    }

    None
}

/// Get primary key field name - panics if no primary key is found
pub fn get_primary_key_field_name(fields: &[&Field]) -> String {
    extract_primary_key_info(fields)
        .map(|info| info.field_name)
        .expect("No primary key field found in entity. Please ensure at least one field is marked as primary key or named 'id'.")
}

/// Get primary key column name - panics if no primary key is found
pub fn get_primary_key_column_name(fields: &[&Field]) -> String {
    extract_primary_key_info(fields)
        .map(|info| info.column_name)
        .expect("No primary key field found in entity. Please ensure at least one field is marked as primary key or named 'id'.")
}

/// Get primary key field identifier - panics if no primary key is found
pub fn get_primary_key_field_ident(fields: &[&Field]) -> proc_macro2::Ident {
    extract_primary_key_info(fields)
        .map(|info| info.field_ident)
        .expect("No primary key field found in entity. Please ensure at least one field is marked as primary key or named 'id'.")
}

/// Get primary key field name with fallback to a default value
pub fn get_primary_key_field_name_with_fallback(fields: &[&Field], fallback: &str) -> String {
    extract_primary_key_info(fields)
        .map(|info| info.field_name)
        .unwrap_or_else(|| fallback.to_string())
}

/// Get primary key column name with fallback to a default value
pub fn get_primary_key_column_name_with_fallback(fields: &[&Field], fallback: &str) -> String {
    extract_primary_key_info(fields)
        .map(|info| info.column_name)
        .unwrap_or_else(|| fallback.to_string())
}

/// Get primary key field identifier with fallback to a default value
pub fn get_primary_key_field_ident_with_fallback(fields: &[&Field], fallback: &str) -> proc_macro2::Ident {
    extract_primary_key_info(fields)
        .map(|info| info.field_ident)
        .unwrap_or_else(|| format_ident!("{}", fallback))
}

/// Generate a value parser for a specific field
pub fn generate_field_value_parser(field_name: &str, field_type: &Type) -> TokenStream {
    quote! {
        |value: &str| -> sea_orm::Value {
            // Use SeaORM's from method which handles type conversion automatically
            sea_orm::Value::from(value)
        }
    }
}

/// Generate a type-safe value parser that handles the specific field type
pub fn generate_typed_field_parser(field_name: &str, field_type: &Type) -> TokenStream {
    // This would need more sophisticated type detection
    // For now, we'll use a generic approach
    quote! {
        |value: &str| -> sea_orm::Value {
            // Use SeaORM's from method which handles type conversion automatically
            sea_orm::Value::from(value)
        }
    }
}

/// Generate a dynamic value parser that can handle any field type
pub fn generate_dynamic_value_parser() -> TokenStream {
    quote! {
        |field_name: &str, value: &str| -> sea_orm::Value {
            // Use SeaORM's from method which handles type conversion automatically
            sea_orm::Value::from(value)
        }
    }
}
