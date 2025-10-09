//! Centralized primary key utilities to replace hardcoded "id" assumptions
//! and provide reliable, factored functions for primary key access.

use syn::{Field, Type};

/// Check if a field is marked as auto-increment (shared implementation)
pub fn is_auto_increment_field_impl(field: &Field) -> bool {
    // First check if it's explicitly marked as primary key
    let is_primary_key = field.attrs.iter().any(|attr| {
        if let syn::Meta::List(meta) = &attr.meta {
            meta.path.is_ident("sea_orm") && meta.tokens.to_string().contains("primary_key")
        } else {
            false
        }
    });
    
    if !is_primary_key {
        return false;
    }
    
    // For primary keys, check if auto_increment is explicitly set to false
    let has_explicit_auto_increment_false = field.attrs.iter().any(|attr| {
        if let syn::Meta::List(meta) = &attr.meta {
            if meta.path.is_ident("sea_orm") {
                let tokens = meta.tokens.to_string();
                tokens.contains("auto_increment = false")
            } else {
                false
            }
        } else {
            false
        }
    });
    
    // If auto_increment is explicitly set to false, it's not auto-increment
    if has_explicit_auto_increment_false {
        return false;
    }
    
    // For primary keys without explicit auto_increment = false, they are auto-increment by default
    true
}

/// Check if a field is marked with #[sea_orm(caustics_default)]
pub fn has_caustics_default_attr(field: &Field) -> bool {
    field.attrs.iter().any(|attr| {
        if let syn::Meta::List(meta) = &attr.meta {
            if meta.path.is_ident("sea_orm") {
                meta.tokens.to_string().contains("caustics_default")
            } else {
                false
            }
        } else {
            false
        }
    })
}

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
                        // Look for column_name = "..." in the attribute tokens
                        let tokens = meta.tokens.to_string();
                        if let Some(start) = tokens.find("column_name = \"") {
                            let start = start + "column_name = \"".len();
                            if let Some(end) = tokens[start..].find('"') {
                                let column_name = &tokens[start..start + end];
                                Some(column_name.to_string())
                            } else {
                                None
                            }
                        } else {
                            None
                        }
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
        is_auto_increment_field_impl(field)
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

    // No primary key found - return None instead of fallback assumptions
    // This forces explicit configuration
    None
}

/// Extract ALL primary key information from a list of fields (for composite primary keys)
pub fn extract_all_primary_key_info(fields: &[&Field]) -> Vec<PrimaryKeyInfo> {
    fields
        .iter()
        .filter(|field| {
            field.attrs.iter().any(|attr| {
                if let syn::Meta::List(meta) = &attr.meta {
                    (meta.path.is_ident("sea_orm") && meta.tokens.to_string().contains("primary_key"))
                        || meta.path.is_ident("primary_key")
                } else {
                    false
                }
            })
        })
        .map(|field| PrimaryKeyInfo::from_field(field))
        .collect()
}

/// Get primary key field name - panics if no primary key is found
pub fn get_primary_key_field_name(fields: &[&Field]) -> String {
    extract_primary_key_info(fields)
        .map(|info| info.field_name().to_string())
        .expect("No primary key field found in entity. Please ensure at least one field is marked as primary key or named 'id'.")
}

/// Get primary key column name - panics if no primary key is found
pub fn get_primary_key_column_name(fields: &[&Field]) -> String {
    extract_primary_key_info(fields)
        .map(|info| info.column_name().to_string())
        .expect("No primary key field found in entity. Please ensure at least one field is marked as primary key or named 'id'.")
}

/// Get primary key field identifier - panics if no primary key is found
pub fn get_primary_key_field_ident(fields: &[&Field]) -> proc_macro2::Ident {
    extract_primary_key_info(fields)
        .map(|info| info.field_ident().clone())
        .expect("No primary key field found in entity. Please ensure at least one field is marked as primary key or named 'id'.")
}

/// Get all primary key field names for composite primary keys
pub fn get_all_primary_key_field_names(fields: &[&Field]) -> Vec<String> {
    extract_all_primary_key_info(fields)
        .into_iter()
        .map(|info| info.field_name().to_string())
        .collect()
}

/// Get all primary key field identifiers for composite primary keys
pub fn get_all_primary_key_field_idents(fields: &[&Field]) -> Vec<proc_macro2::Ident> {
    extract_all_primary_key_info(fields)
        .into_iter()
        .map(|info| info.field_ident().clone())
        .collect()
}

/// Check if entity has composite primary keys (more than one primary key)
pub fn has_composite_primary_key(fields: &[&Field]) -> bool {
    extract_all_primary_key_info(fields).len() > 1
}

/// Check if entity has any auto-increment primary keys
pub fn has_auto_increment_primary_key(fields: &[&Field]) -> bool {
    extract_all_primary_key_info(fields)
        .iter()
        .any(|info| info.is_auto_increment())
}
