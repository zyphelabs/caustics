//! Centralized entity name resolution system
//!
//! This module provides a robust way to resolve entity names across different contexts
//! (table names, entity names, module names, etc.) without fragile string manipulation.

use heck::{ToPascalCase, ToSnakeCase};
use proc_macro2::TokenStream;
use quote::{format_ident, quote};
use syn::Ident;

/// Entity name context that contains all the different naming conventions
/// for a single entity, ensuring consistency across the codebase.
#[derive(Debug, Clone)]
pub struct EntityNameContext {
    /// The entity name in PascalCase (e.g., "User", "Post")
    pub entity_name: String,
    /// The entity name in snake_case (e.g., "user", "post")
    pub entity_name_snake: String,
    /// The table name (e.g., "users", "posts")
    pub table_name: String,
    /// The module name (e.g., "user", "post")
    pub module_name: String,
}

impl EntityNameContext {
    /// Create a new EntityNameContext from a table name
    /// This is the most robust way since table names are explicitly defined
    pub fn from_table_name(table_name: &str) -> Self {
        // Table name is typically plural (e.g., "users", "posts")
        // Convert to singular for entity name
        let entity_name_snake = if table_name.ends_with('s') && table_name.len() > 1 {
            // Simple pluralization: remove 's' suffix
            table_name[..table_name.len() - 1].to_string()
        } else {
            table_name.to_string()
        };

        let entity_name = entity_name_snake.to_pascal_case();
        let module_name = entity_name_snake.clone();

        Self {
            entity_name,
            entity_name_snake,
            table_name: table_name.to_string(),
            module_name,
        }
    }

    /// Create a new EntityNameContext from an entity name (PascalCase)
    pub fn from_entity_name(entity_name: &str) -> Self {
        let entity_name_snake = entity_name.to_snake_case();
        let table_name = format!("{}s", entity_name_snake); // Simple pluralization
        let module_name = entity_name_snake.clone();

        Self {
            entity_name: entity_name.to_string(),
            entity_name_snake,
            table_name,
            module_name,
        }
    }

    /// Create a new EntityNameContext from a module name (snake_case)
    pub fn from_module_name(module_name: &str) -> Self {
        let entity_name = module_name.to_pascal_case();
        let entity_name_snake = module_name.to_string();
        let table_name = format!("{}s", module_name); // Simple pluralization

        Self {
            entity_name,
            entity_name_snake,
            table_name,
            module_name: module_name.to_string(),
        }
    }

    /// Get the entity name for registry lookups (PascalCase)
    pub fn registry_name(&self) -> &str {
        &self.entity_name
    }

    /// Get the entity name for fetcher lookups (snake_case)
    pub fn fetcher_name(&self) -> &str {
        &self.entity_name_snake
    }

    /// Get the table name for database operations
    pub fn table_name(&self) -> &str {
        &self.table_name
    }

    /// Get the module name for code generation
    pub fn module_name(&self) -> &str {
        &self.module_name
    }
}

/// Generate helper functions for entity name resolution in the generated client code
pub fn generate_entity_name_helpers() -> TokenStream {
    quote! {
        /// Convert table name to entity name for registry lookups
        pub fn table_name_to_entity_name(table_name: &str) -> String {
            // Table name is typically plural (e.g., "users", "posts")
            // Convert to singular for entity name
            let entity_name_snake = if table_name.ends_with('s') && table_name.len() > 1 {
                // Simple pluralization: remove 's' suffix
                &table_name[..table_name.len() - 1]
            } else {
                table_name
            };

            // Convert to PascalCase
            let mut result = String::new();
            let mut capitalize = true;
            for c in entity_name_snake.chars() {
                if c == '_' {
                    capitalize = true;
                } else if capitalize {
                    result.push(c.to_ascii_uppercase());
                    capitalize = false;
                } else {
                    result.push(c);
                }
            }
            result
        }

        /// Convert entity name to table name for database operations
        pub fn entity_name_to_table_name(entity_name: &str) -> String {
            let snake_case = entity_name.to_lowercase();
            format!("{}s", snake_case) // Simple pluralization
        }

        /// Convert entity name to fetcher name for registry lookups
        pub fn entity_name_to_fetcher_name(entity_name: &str) -> String {
            entity_name.to_lowercase()
        }
    }
}

/// Generate entity name resolution for a specific entity
pub fn generate_entity_name_resolution(entity_context: &EntityNameContext) -> TokenStream {
    let entity_name = &entity_context.entity_name;
    let entity_name_snake = &entity_context.entity_name_snake;
    let table_name = &entity_context.table_name;
    let module_name = &entity_context.module_name;

    quote! {
        pub const ENTITY_NAME: &str = #entity_name;
        pub const ENTITY_NAME_SNAKE: &str = #entity_name_snake;
        pub const TABLE_NAME: &str = #table_name;
        pub const MODULE_NAME: &str = #module_name;

        /// Get the entity name for registry lookups
        pub fn get_entity_name() -> &'static str {
            ENTITY_NAME
        }

        /// Get the entity name for fetcher lookups
        pub fn get_fetcher_name() -> &'static str {
            ENTITY_NAME_SNAKE
        }

        /// Get the table name for database operations
        pub fn get_table_name() -> &'static str {
            TABLE_NAME
        }
    }
}
