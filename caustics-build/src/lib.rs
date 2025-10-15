#![allow(clippy::cmp_owned)]
#![allow(clippy::type_complexity)]
#![allow(clippy::too_many_arguments)]
#![allow(clippy::possible_missing_else)]

use heck::{ToPascalCase, ToSnakeCase};
use quote::{format_ident, quote, ToTokens};
use std::fs;
use syn::{parse_file, Attribute, Item, Meta, Type};

#[derive(Debug, Clone)]
struct EntityMetadata {
    name: String,
    table_name: String,
    primary_key_field: String,
    foreign_key_fields: Vec<String>,
    relations: Vec<RelationMetadata>,
    #[allow(dead_code)]
    primary_key_type: String,
    foreign_key_types: Vec<(String, String)>,
}

#[derive(Debug, Clone)]
struct RelationMetadata {
    name: String,
    target_entity: String,
    target_table_name: String,
    foreign_key_field: Option<String>,
    relation_kind: String, // "HasMany" or "BelongsTo"
}

/// Convert a TypeId back to a token stream for code generation
fn type_id_to_string(type_id: std::any::TypeId) -> String {
    if type_id == std::any::TypeId::of::<i8>() {
        "i8".to_string()
    } else if type_id == std::any::TypeId::of::<i16>() {
        "i16".to_string()
    } else if type_id == std::any::TypeId::of::<i32>() {
        "i32".to_string()
    } else if type_id == std::any::TypeId::of::<i64>() {
        "i64".to_string()
    } else if type_id == std::any::TypeId::of::<isize>() {
        "isize".to_string()
    } else if type_id == std::any::TypeId::of::<u8>() {
        "u8".to_string()
    } else if type_id == std::any::TypeId::of::<u16>() {
        "u16".to_string()
    } else if type_id == std::any::TypeId::of::<u32>() {
        "u32".to_string()
    } else if type_id == std::any::TypeId::of::<u64>() {
        "u64".to_string()
    } else if type_id == std::any::TypeId::of::<usize>() {
        "usize".to_string()
    } else if type_id == std::any::TypeId::of::<f32>() {
        "f32".to_string()
    } else if type_id == std::any::TypeId::of::<f64>() {
        "f64".to_string()
    } else if type_id == std::any::TypeId::of::<String>() {
        "String".to_string()
    } else if type_id == std::any::TypeId::of::<str>() {
        "str".to_string()
    } else if type_id == std::any::TypeId::of::<bool>() {
        "bool".to_string()
    } else if type_id == std::any::TypeId::of::<uuid::Uuid>() {
        "uuid::Uuid".to_string()
    } else if type_id == std::any::TypeId::of::<caustics::chrono::DateTime<caustics::chrono::Utc>>() {
        "caustics::chrono::DateTime<caustics::chrono::Utc>".to_string()
    } else if type_id == std::any::TypeId::of::<caustics::chrono::NaiveDateTime>() {
        "caustics::chrono::NaiveDateTime".to_string()
    } else if type_id == std::any::TypeId::of::<caustics::chrono::NaiveDate>() {
        "caustics::chrono::NaiveDate".to_string()
    } else if type_id == std::any::TypeId::of::<caustics::chrono::NaiveTime>() {
        "caustics::chrono::NaiveTime".to_string()
    } else if type_id == std::any::TypeId::of::<caustics::serde_json::Value>() {
        "caustics::serde_json::Value".to_string()
    } else {
        panic!("Unsupported TypeId in code generation: {:?}", type_id);
    }
}

/// Convert a syn::Type to a TypeId for comprehensive database types
fn get_type_id_from_ty(ty: &Type) -> std::any::TypeId {
    match ty {
        Type::Path(type_path) => {
            if let Some(segment) = type_path.path.segments.last() {
                match segment.ident.to_string().as_str() {
                    // Integer types
                    "i8" => std::any::TypeId::of::<i8>(),
                    "i16" => std::any::TypeId::of::<i16>(),
                    "i32" => std::any::TypeId::of::<i32>(),
                    "i64" => std::any::TypeId::of::<i64>(),
                    "isize" => std::any::TypeId::of::<isize>(),
                    "u8" => std::any::TypeId::of::<u8>(),
                    "u16" => std::any::TypeId::of::<u16>(),
                    "u32" => std::any::TypeId::of::<u32>(),
                    "u64" => std::any::TypeId::of::<u64>(),
                    "usize" => std::any::TypeId::of::<usize>(),

                    // Floating point types
                    "f32" => std::any::TypeId::of::<f32>(),
                    "f64" => std::any::TypeId::of::<f64>(),

                    // String and text types
                    "String" => std::any::TypeId::of::<String>(),
                    "str" => std::any::TypeId::of::<str>(),

                    // Boolean type
                    "bool" => std::any::TypeId::of::<bool>(),

                    // UUID type
                    "Uuid" => std::any::TypeId::of::<uuid::Uuid>(),

                    // DateTime types
                    "DateTime" => std::any::TypeId::of::<caustics::chrono::DateTime<caustics::chrono::Utc>>(),
                    "NaiveDateTime" => std::any::TypeId::of::<caustics::chrono::NaiveDateTime>(),
                    "NaiveDate" => std::any::TypeId::of::<caustics::chrono::NaiveDate>(),
                    "NaiveTime" => std::any::TypeId::of::<caustics::chrono::NaiveTime>(),

                    // JSON type
                    "Value" => std::any::TypeId::of::<caustics::serde_json::Value>(),
                    "Json" => std::any::TypeId::of::<caustics::serde_json::Value>(),

                    // Option types - handle Option<T> by extracting the inner type
                    "Option" => {
                        if let syn::PathArguments::AngleBracketed(args) = &segment.arguments {
                            if let Some(syn::GenericArgument::Type(inner_ty)) = args.args.first() {
                                // Recursively get the type ID of the inner type
                                return get_type_id_from_ty(inner_ty);
                            }
                        }
                        // If we can't extract the inner type, panic
                        panic!("Cannot extract inner type from Option<{}>. Please ensure the inner type is supported.", segment.ident);
                    }

                    // For unknown types, treat them as ToSeaOrmValue implementations
                    // This allows any custom type that implements ToSeaOrmValue to work
                    _ => {
                        // For ToSeaOrmValue types, we still use String as the underlying storage
                        // but we'll handle them specially in the code generation
                        std::any::TypeId::of::<String>()
                    }
                }
            } else {
                panic!("Cannot determine type from path with no segments. Please ensure the type is properly specified.");
            }
        }
        _ => {
            // For complex types (generics, references, etc.), panic
            panic!("Unsupported complex type. Please use a supported primitive type or add support for this type.");
        }
    }
}

/// Extract type information from a syn::Type, detecting if it's Option<T> and returning the inner type
fn extract_type_info(ty: &syn::Type) -> (syn::Type, bool) {
    match ty {
        syn::Type::Path(type_path) => {
            if let Some(segment) = type_path.path.segments.last() {
                if segment.ident == "Option" {
                    if let syn::PathArguments::AngleBracketed(args) = &segment.arguments {
                        if let Some(syn::GenericArgument::Type(inner_ty)) = args.args.first() {
                            return (inner_ty.clone(), true);
                        }
                    }
                }
            }
            (ty.clone(), false)
        }
        _ => (ty.clone(), false),
    }
}

/// Find the type of a field in a struct by looking through the struct definition
/// Returns (inner_type, is_optional)
fn find_field_type_in_struct_with_optional(
    file: &syn::File,
    entity_name: &str,
    field_name: &str,
) -> Option<(String, bool)> {
    // Look for the Model struct within the module that matches the entity name
    for item in &file.items {
        if let Item::Mod(module) = item {
            // Check if this module matches our entity
            let module_name = module.ident.to_string().to_lowercase();
            let entity_name_lower = entity_name.to_lowercase();
            if module_name == entity_name_lower {
                if let Some((_, items)) = &module.content {
                    for module_item in items {
                        if let Item::Struct(struct_item) = module_item {
                            // Look for the Model struct
                            if struct_item.ident.to_string() == "Model" {
                                for field in &struct_item.fields {
                                    if let Some(ident) = &field.ident {
                                        // Check if the field name matches directly
                                        if ident.to_string() == field_name {
                                            let (inner_type, is_optional) = extract_type_info(&field.ty);
                                            return Some((type_id_to_string(get_type_id_from_ty(&inner_type)), is_optional));
                                        }

                                        // Also check if the field has a column_name attribute that matches
                                        for attr in &field.attrs {
                                            if attr.path().is_ident("sea_orm") {
                                                let attr_str = attr.to_token_stream().to_string();
                                                if attr_str.contains("column_name") {
                                                    if let Some(start) =
                                                        attr_str.find("column_name")
                                                    {
                                                        if let Some(equals) =
                                                            attr_str[start..].find('=')
                                                        {
                                                            let after_equals =
                                                                &attr_str[start + equals + 1..];
                                                            if let Some(quote_start) =
                                                                after_equals.find('"')
                                                            {
                                                                if let Some(quote_end) =
                                                                    after_equals[quote_start + 1..]
                                                                        .find('"')
                                                                {
                                                                    let column_name = &after_equals
                                                                        [quote_start + 1
                                                                            ..quote_start
                                                                                + 1
                                                                                + quote_end];
                                                                    if column_name == field_name {
                                                                        let (inner_type, is_optional) = extract_type_info(&field.ty);
                                                                        return Some((type_id_to_string(get_type_id_from_ty(&inner_type)), is_optional));
                                                                    }
                                                                }
                                                            }
                                                        }
                                                    }
                                                }
                                            }
                                        }
                                    }
                                }
                            }
                        }
                    }
                }
            }
        }
    }
    None
}

fn extract_entity_metadata(
    file_path: &str,
    entity_name: &str,
    _module_path: &str,
    module_name: &str,
) -> Option<EntityMetadata> {
    let content = match fs::read_to_string(file_path) {
        Ok(c) => c,
        Err(_e) => {
            return None;
        }
    };

    let file = match parse_file(&content) {
        Ok(f) => f,
        Err(_) => return None,
    };

    let mut foreign_key_fields = Vec::new();
    let mut relations = Vec::new();
    let mut primary_key_field = None; // No default fallback - must be detected
    let mut primary_key_type = None; // Must be detected from the actual field
    let mut foreign_key_types = Vec::new();
    let mut table_name = None; // Extract from #[sea_orm(table_name = "...")]

    // Extract foreign key fields from Model struct (look inside modules)
    for item in &file.items {
        if let Item::Mod(module) = item {
            // Only process the module that matches our entity
            if module.ident.to_string() != *module_name {
                continue;
            }
            if let Some((_, items)) = &module.content {
                for module_item in items {
                    if let Item::Struct(struct_item) = module_item {
                        if struct_item.ident == "Model" {
                            // Extract table name from #[sea_orm(table_name = "...")]
                            for attr in &struct_item.attrs {
                                if attr.path().is_ident("sea_orm") {
                                    // Parse the attribute content directly from the token stream
                                    let attr_str = attr.to_token_stream().to_string();

                                    // Look for table_name = "value" pattern
                                    if let Some(start) = attr_str.find("table_name") {
                                        if let Some(equals) = attr_str[start..].find('=') {
                                            let after_equals = &attr_str[start + equals + 1..];
                                            if let Some(quote_start) = after_equals.find('"') {
                                                if let Some(quote_end) =
                                                    after_equals[quote_start + 1..].find('"')
                                                {
                                                    let table_name_value = &after_equals[quote_start
                                                        + 1
                                                        ..quote_start + 1 + quote_end];
                                                    table_name = Some(table_name_value.to_string());
                                                }
                                            }
                                        }
                                    }
                                }
                            }

                            if let syn::Fields::Named(fields) = &struct_item.fields {
                                for field in &fields.named {
                                    if let Some(field_name) = field.ident.as_ref() {
                                        let field_name_str = field_name.to_string();
                                        let field_type_id = get_type_id_from_ty(&field.ty);

                                        // Check if field is marked as primary key
                                        let is_primary_key = field.attrs.iter().any(|attr| {
                                            attr.path().is_ident("sea_orm")
                                                && attr
                                                    .meta
                                                    .to_token_stream()
                                                    .to_string()
                                                    .contains("primary_key")
                                        });

                                        if is_primary_key {
                                            primary_key_field = Some(field_name_str.clone());
                                            primary_key_type = Some(field_type_id);
                                        }

                                        // Foreign key detection is now handled by parsing Relation enum annotations
                                        // This ensures we only detect actual foreign keys, not just fields ending with _id
                                    }
                                }
                            }
                        }
                    }
                }
            }
        }
    }

    // Extract relations from Relation enum (look inside modules)
    for item in &file.items {
        if let Item::Mod(module) = item {
            // Only process the module that matches our entity
            if module.ident.to_string() != *module_name {
                continue;
            }
            if let Some((_, items)) = &module.content {
                for module_item in items {
                    if let Item::Enum(enum_item) = module_item {
                        if enum_item.ident == "Relation" {
                            for variant in &enum_item.variants {
                                let relation_name = variant.ident.to_string();

                                // Parse the relation attributes to extract metadata
                                let mut target_entity = String::new();
                                let mut foreign_key_field = None;
                                let mut relation_kind = String::new();

                                for attr in &variant.attrs {
                                    if attr.path().is_ident("sea_orm") {
                                        // Parse #[sea_orm(has_many/has_one/belongs_to = "...", from = "...", to = "...")]
                                        let attr_str = attr.to_token_stream().to_string();

                                        // Extract relation kind (has_many, belongs_to, or has_one)
                                        if attr_str.contains("has_many") {
                                            relation_kind = "HasMany".to_string();
                                        } else if attr_str.contains("belongs_to") {
                                            relation_kind = "BelongsTo".to_string();
                                        } else if attr_str.contains("has_one") {
                                            relation_kind = "HasOne".to_string();
                                        }

                                        // Extract target entity from has_many/belongs_to = "super::entity::Entity"
                                        if let Some(start) = attr_str.find("has_many") {
                                            if let Some(equals) = attr_str[start..].find('=') {
                                                let after_equals = &attr_str[start + equals + 1..];
                                                if let Some(quote_start) = after_equals.find('"') {
                                                    if let Some(quote_end) =
                                                        after_equals[quote_start + 1..].find('"')
                                                    {
                                                        let target_path = &after_equals[quote_start
                                                            + 1
                                                            ..quote_start + 1 + quote_end];
                                                        // Extract entity name from "super::entity::Entity"
                                                        // The path is like "super::post::Entity", we want "post"
                                                        if let Some(last_colon) =
                                                            target_path.rfind("::")
                                                        {
                                                            let entity_part =
                                                                &target_path[..last_colon];
                                                            if let Some(second_last_colon) =
                                                                entity_part.rfind("::")
                                                            {
                                                                target_entity = entity_part
                                                                    [second_last_colon + 2..]
                                                                    .to_string();
                                                            } else {
                                                                target_entity =
                                                                    entity_part.to_string();
                                                            }
                                                        }
                                                    }
                                                }
                                            }
                                        } else if let Some(start) = attr_str.find("belongs_to") {
                                            if let Some(equals) = attr_str[start..].find('=') {
                                                let after_equals = &attr_str[start + equals + 1..];
                                                if let Some(quote_start) = after_equals.find('"') {
                                                    if let Some(quote_end) =
                                                        after_equals[quote_start + 1..].find('"')
                                                    {
                                                        let target_path = &after_equals[quote_start
                                                            + 1
                                                            ..quote_start + 1 + quote_end];
                                                        // Extract entity name from "super::entity::Entity"
                                                        // The path is like "super::user::Entity", we want "user"
                                                        if let Some(last_colon) =
                                                            target_path.rfind("::")
                                                        {
                                                            let entity_part =
                                                                &target_path[..last_colon];
                                                            if let Some(second_last_colon) =
                                                                entity_part.rfind("::")
                                                            {
                                                                target_entity = entity_part
                                                                    [second_last_colon + 2..]
                                                                    .to_string();
                                                            } else {
                                                                target_entity =
                                                                    entity_part.to_string();
                                                            }
                                                        }
                                                    }
                                                }
                                            }
                                        } else if let Some(start) = attr_str.find("has_one") {
                                            if let Some(equals) = attr_str[start..].find('=') {
                                                let after_equals = &attr_str[start + equals + 1..];
                                                if let Some(quote_start) = after_equals.find('"') {
                                                    if let Some(quote_end) =
                                                        after_equals[quote_start + 1..].find('"')
                                                    {
                                                        let target_path = &after_equals[quote_start
                                                            + 1
                                                            ..quote_start + 1 + quote_end];
                                                        // Extract entity name from "super::entity::Entity"
                                                        // The path is like "super::api_key::Entity", we want "api_key"
                                                        if let Some(last_colon) =
                                                            target_path.rfind("::")
                                                        {
                                                            let entity_part =
                                                                &target_path[..last_colon];
                                                            if let Some(second_last_colon) =
                                                                entity_part.rfind("::")
                                                            {
                                                                target_entity = entity_part
                                                                    [second_last_colon + 2..]
                                                                    .to_string();
                                                            } else {
                                                                target_entity =
                                                                    entity_part.to_string();
                                                            }
                                                        }
                                                    }
                                                }
                                            }
                                        }

                                        // Extract foreign key field from from = "Column::FieldName"
                                        if let Some(start) = attr_str.find("from") {
                                            if let Some(equals) = attr_str[start..].find('=') {
                                                let after_equals = &attr_str[start + equals + 1..];
                                                if let Some(quote_start) = after_equals.find('"') {
                                                    if let Some(quote_end) =
                                                        after_equals[quote_start + 1..].find('"')
                                                    {
                                                        let column_str = &after_equals[quote_start
                                                            + 1
                                                            ..quote_start + 1 + quote_end];
                                                        if let Some(field_name) =
                                                            column_str.split("::").nth(1)
                                                        {
                                                            // Convert PascalCase to snake_case for database field names
                                                            let snake_case_name =
                                                                field_name.to_snake_case();
                                                            foreign_key_field =
                                                                Some(snake_case_name);
                                                        }
                                                    }
                                                }
                                            }
                                        }

                                        // For has_one and has_many relations, extract the 'to' field instead of 'from'
                                        if relation_kind == "HasOne" || relation_kind == "HasMany" {
                                            if let Some(start) = attr_str.find("to") {
                                                if let Some(equals) = attr_str[start..].find('=') {
                                                    let after_equals = &attr_str[start + equals + 1..];
                                                    if let Some(quote_start) = after_equals.find('"') {
                                                        if let Some(quote_end) =
                                                            after_equals[quote_start + 1..].find('"')
                                                        {
                                                            let column_str = &after_equals[quote_start
                                                                + 1
                                                                ..quote_start + 1 + quote_end];
                                                            if let Some(field_name) =
                                                                column_str.split("::").nth(1)
                                                            {
                                                                // Convert PascalCase to snake_case for database field names
                                                                let snake_case_name =
                                                                    field_name.to_snake_case();
                                                                foreign_key_field =
                                                                    Some(snake_case_name);
                                                            }
                                                        }
                                                    }
                                                }
                                            }
                                        }
                                    }
                                }

                                if !target_entity.is_empty() {
                                    // Only collect foreign key fields from belongs_to relations, not has_many or has_one relations
                                    // In has_many relations, the 'from' field refers to the primary key of the current entity
                                    // In has_one relations, the 'from' field refers to the primary key of the current entity
                                    // In belongs_to relations, the 'from' field refers to the foreign key in the current entity
                                    if relation_kind == "BelongsTo" {
                                        if let Some(ref fk_field) = foreign_key_field {
                                            foreign_key_fields.push(fk_field.clone());

                                            // Find the type of this foreign key field by looking at the struct fields
                                            if let Some((field_type, _is_optional)) = find_field_type_in_struct_with_optional(
                                                &file,
                                                module_name,
                                                fk_field,
                                            ) {
                                                // Store the inner type, not the full type with Option<>
                                                // The conversion functions expect just the inner type
                                                foreign_key_types
                                                    .push((fk_field.clone(), field_type));
                                            }
                                        }
                                    }

                                    relations.push(RelationMetadata {
                                        name: relation_name,
                                        target_entity,
                                        target_table_name: String::new(), // Will be resolved later
                                        foreign_key_field: foreign_key_field.map(|s| s.to_string()),
                                        relation_kind,
                                    });
                                }
                            }
                        }
                    }
                }
            }
        }
    }

    let result = EntityMetadata {
        name: entity_name.to_string(),
        table_name: table_name.unwrap_or_else(|| {
            panic!("No table_name found for entity '{}'. Please ensure the Model struct has #[sea_orm(table_name = \"...\")] attribute.", entity_name)
        }),
        primary_key_field: primary_key_field.unwrap_or_else(|| {
            panic!("No primary key field found for entity '{}'. Please ensure at least one field is marked with #[primary_key] attribute.", entity_name)
        }),
        foreign_key_fields,
        relations,
        primary_key_type: type_id_to_string(primary_key_type.unwrap_or_else(|| {
            panic!("No primary key type found for entity '{}'. This should not happen if primary key field was detected.", entity_name)
        })),
        foreign_key_types,
    };

    Some(result)
}

/// Resolve target table names for relations by looking them up in the entity metadata
fn resolve_target_table_names(entities_metadata: &mut [EntityMetadata]) {
    // Create a lookup table from entity name to table name
    let entity_lookup: std::collections::HashMap<String, String> = entities_metadata
        .iter()
        .map(|metadata| (metadata.name.clone(), metadata.table_name.clone()))
        .collect();

    // Resolve target table names for all relations
    for entity_metadata in entities_metadata.iter_mut() {
        for relation in entity_metadata.relations.iter_mut() {
            if relation.target_table_name.is_empty() {
                // Convert target_entity to PascalCase for lookup
                let target_entity_pascal = relation.target_entity.to_pascal_case();
                
                // Look up the target table name from the entity lookup
                relation.target_table_name = entity_lookup
                    .get(&target_entity_pascal)
                    .cloned()
                    .unwrap_or_else(|| {
                        // Fallback to snake_case conversion if not found
                        relation.target_entity.to_lowercase()
                    });
            }
        }
    }
}

fn has_caustics_attribute(attrs: &[Attribute]) -> bool {
    attrs.iter().any(|attr| attr.path().is_ident("caustics"))
}

fn has_caustics_derive(attrs: &[Attribute]) -> bool {
    attrs.iter().any(|attr| {
        if attr.path().is_ident("derive") {
            if let Meta::List(meta) = &attr.meta {
                meta.tokens.to_string().contains("Caustics")
            } else {
                false
            }
        } else {
            false
        }
    })
}

fn generate_client_code(
    entities: &[(String, String)],
    entities_metadata: &[EntityMetadata],
    _include_registry: bool,
) -> String {
    let entity_methods: Vec<_> = entities
        .iter()
        .map(|(name, _module_path)| {
            let method_name = format_ident!("{}", name.to_lowercase());
            let entity_name = name.to_lowercase();
            let entity_client_alias = format_ident!("{}EntityClient", entity_name.to_pascal_case());

            // Use imported EntityClient with alias instead of fully qualified path
            quote! {
                pub fn #method_name(&self) -> #entity_client_alias<'_, DatabaseConnection> {
                    #entity_client_alias::new(&*self.db, self.database_backend)
                }
            }
        })
        .collect();

    let tx_entity_methods: Vec<_> = entities
        .iter()
        .map(|(name, _module_path)| {
            let method_name = format_ident!("{}", name.to_lowercase());
            let entity_name = name.to_lowercase();
            let entity_client_alias = format_ident!("{}EntityClient", entity_name.to_pascal_case());

            // Use imported EntityClient with alias instead of fully qualified path
            quote! {
                pub fn #method_name(&self) -> #entity_client_alias<'_, DatabaseTransaction> {
                    #entity_client_alias::new(&*self.tx, self.database_backend)
                }
            }
        })
        .collect();

    // Generate the composite registry
    let registry_match_arms: Vec<_> = entities
        .iter()
        .map(|(name, _module_path)| {
            let entity_name_lower = name.to_lowercase();
            let entity_name = name.to_lowercase();
            let entity_fetcher_alias =
                format_ident!("{}EntityFetcherImpl", entity_name.to_pascal_case());

            // Use imported EntityFetcherImpl with alias instead of fully qualified path
            quote! {
                #entity_name_lower => Some(&#entity_fetcher_alias),
            }
        })
        .collect();

    // Import statements and prefixes (no longer conditional)
    let (
        imports,
        registry_trait,
        fetcher_trait,
        batch_container,
        batch_query,
        batch_result,
        from_model,
        merge_into,
    ) = (
        quote! {
            use caustics::sea_orm::{DatabaseConnection, DatabaseTransaction, TransactionTrait};
        },
        quote! { caustics::EntityRegistry<C> },
        quote! { caustics::EntityFetcher },
        quote! { caustics::BatchContainer },
        quote! { caustics::BatchQuery },
        quote! { caustics::BatchResult },
        quote! { caustics::FromModel },
        quote! { caustics::MergeInto },
    );

    let hooks_mod = quote! { caustics::hooks };

    let _entity_prelude_uses: Vec<proc_macro2::TokenStream> = Vec::new();

    let (prelude_use, prelude_block) = (quote! {}, quote! {});

    let raw_block = quote! {
        // Raw SQL support (typed bindings and results)
        // Use the Raw type from the main caustics crate
        pub use caustics::Raw;
    };

    // Generate entity metadata
    let entity_metadata_items: Vec<_> = entities_metadata
        .iter()
        .map(|metadata| {
            let entity_name = &metadata.name;
            let foreign_key_fields = &metadata.foreign_key_fields;
            let relations = &metadata.relations;

            let fk_fields_lit = foreign_key_fields
                .iter()
                .map(|f| quote! { #f })
                .collect::<Vec<_>>();
            let relations_lit = relations
                .iter()
                .map(|rel| {
                    let rel_name = &rel.name;
                    let target_entity = &rel.target_entity;
                    let target_table_name = &rel.target_table_name;
                    let fk_field = &rel.foreign_key_field;
                    let relation_kind = &rel.relation_kind;
                    let fk_field_expr = if let Some(fk) = fk_field {
                        quote! { Some(#fk) }
                    } else {
                        quote! { None }
                    };
                    quote! {
                        caustics::EntityRelationMetadata {
                            name: #rel_name,
                            target_entity: #target_entity,
                            target_table_name: #target_table_name,
                            foreign_key_field: #fk_field_expr,
                            relation_kind: #relation_kind,
                        }
                    }
                })
                .collect::<Vec<_>>();

            let primary_key_field_lit =
                syn::LitStr::new(&metadata.primary_key_field, proc_macro2::Span::call_site());
            let primary_key_type_lit = &metadata.primary_key_type;
            let foreign_key_types_lit = metadata
                .foreign_key_types
                .iter()
                .map(|(field, type_id)| {
                    quote! { (#field, #type_id) }
                })
                .collect::<Vec<_>>();

            let table_name_lit = &metadata.table_name;
            quote! {
                caustics::EntityMetadata {
                    name: #entity_name,
                    table_name: #table_name_lit,
                    primary_key_field: #primary_key_field_lit,
                    foreign_key_fields: &[#(#fk_fields_lit),*],
                    relations: &[#(#relations_lit),*],
                    primary_key_type: #primary_key_type_lit,
                    foreign_key_types: &[#(#foreign_key_types_lit),*],
                }
            }
        })
        .collect();

    // Generate import statements for entities with aliases to avoid conflicts
    let entity_imports: Vec<proc_macro2::TokenStream> = entities
        .iter()
        .map(|(name, module_path)| {
            // For module names, preserve underscores but convert to lowercase
            let entity_name = name.to_lowercase();
            let entity_ident = format_ident!("{}", entity_name);
            let path_parts: Vec<&str> = module_path.split("::").collect();

            // Build the import path: crate::module_path::entity_name
            let mut import_path = quote! { crate };
            for part in &path_parts {
                let part_ident = format_ident!("{}", part);
                import_path = quote! { #import_path::#part_ident };
            }
            import_path = quote! { #import_path::#entity_ident };

            let entity_client_alias = format_ident!("{}EntityClient", entity_name.to_pascal_case());
            let entity_fetcher_alias = format_ident!("{}EntityFetcherImpl", entity_name.to_pascal_case());
            quote! {
                use #import_path::{EntityClient as #entity_client_alias, EntityFetcherImpl as #entity_fetcher_alias};
            }
        })
        .collect();

    let client_code = quote! {
        #imports
        // Import entity clients and fetchers
        #(#entity_imports)*

        // Import heck traits for case conversion
        use caustics::prelude::ToPascalCase;
        
        // Import uuid crate for UUID types
        use caustics::uuid;

        // Bring all extension traits into scope automatically (generated)
        #prelude_use
        // Arc is used directly to avoid conflicts with test imports

        // Use caustics::EntityMetadata instead of defining our own

        // Static entity metadata registry
        static ENTITY_METADATA: &[caustics::EntityMetadata] = &[
            #(#entity_metadata_items),*
        ];

        // Helper function to get entity metadata with namespace-aware resolution
        pub fn get_entity_metadata(entity_name: &str) -> Option<&'static caustics::EntityMetadata> {
            // Try exact match first
            if let Some(meta) = ENTITY_METADATA.iter().find(|meta| meta.name == entity_name) {
                return Some(meta);
            }

            // Try namespace-aware resolution
            // 1. Try with namespace prefix (e.g., "blog::Post" -> "Post")
            else if let Some(colon_pos) = entity_name.rfind("::") {
                let name_without_namespace = &entity_name[colon_pos + 2..];
                if let Some(meta) = ENTITY_METADATA.iter().find(|meta| meta.name == name_without_namespace) {
                    return Some(meta);
                }
            }

            // 2. Try PascalCase variations
            let pascal_case = entity_name.to_pascal_case();
            if let Some(meta) = ENTITY_METADATA.iter().find(|meta| meta.name == pascal_case) {
                return Some(meta);
            }

            // 3. Try snake_case to PascalCase conversion
            let snake_case = entity_name.to_lowercase();
            let snake_to_pascal = snake_case.to_pascal_case();
            if let Some(meta) = ENTITY_METADATA.iter().find(|meta| meta.name == snake_to_pascal) {
                return Some(meta);
            }

            None
        }


        #[allow(dead_code)]
        pub struct CausticsClient {
            db: std::sync::Arc<DatabaseConnection>,
            database_backend: caustics::sea_orm::DatabaseBackend,
        }

        #raw_block

        pub struct RawQuery<T> {
            db: std::sync::Arc<DatabaseConnection>,
            backend: caustics::sea_orm::DatabaseBackend,
            raw: Raw,
            _marker: std::marker::PhantomData<T>,
        }

        impl<T> RawQuery<T> {
            pub async fn exec(self) -> Result<Vec<T>, caustics::sea_orm::DbErr>
            where
                T: caustics::sea_orm::FromQueryResult + Send + Sync + 'static,
            {
                use caustics::sea_orm::{Statement, SelectorRaw, SelectModel};
                let stmt = Statement::from_sql_and_values(self.backend, self.raw.sql, self.raw.params);
                let rows = SelectorRaw::<SelectModel<T>>::from_statement(stmt).all(self.db.as_ref()).await?;
                Ok(rows)
            }
        }

        pub struct RawExecute {
            db: std::sync::Arc<DatabaseConnection>,
            backend: caustics::sea_orm::DatabaseBackend,
            raw: Raw,
        }

        impl RawExecute {
            pub async fn exec(self) -> Result<caustics::sea_orm::ExecResult, caustics::sea_orm::DbErr> {
                use caustics::sea_orm::{Statement, ConnectionTrait};
                let stmt = Statement::from_sql_and_values(self.backend, self.raw.sql, self.raw.params);
                let res = self.db.execute(stmt).await?;
                Ok(res)
            }
        }

        #[allow(dead_code)]
        pub struct TransactionCausticsClient {
            tx: std::sync::Arc<DatabaseTransaction>,
            database_backend: caustics::sea_orm::DatabaseBackend,
        }

        pub struct TransactionBuilder {
            db: std::sync::Arc<DatabaseConnection>,
            database_backend: caustics::sea_orm::DatabaseBackend,
        }

        // Composite Entity Registry for relation fetching
        pub struct CompositeEntityRegistry;

        impl<C: caustics::sea_orm::ConnectionTrait> #registry_trait for CompositeEntityRegistry {
            fn get_fetcher(&self, entity_name: &str) -> Option<&dyn #fetcher_trait<C>> {
                match entity_name {
                    #(#registry_match_arms)*
                    _ => None,
                }
            }

        }

        // Implement for reference so &REGISTRY works as a trait object
        impl<C: caustics::sea_orm::ConnectionTrait> #registry_trait for &'static CompositeEntityRegistry {
            fn get_fetcher(&self, entity_name: &str) -> Option<&dyn #fetcher_trait<C>> {
                (**self).get_fetcher(entity_name)
            }

        }

        // Implement EntityTypeRegistry trait for type information
        impl caustics::EntityTypeRegistry for CompositeEntityRegistry {
            fn get_primary_key_type(&self, entity_name: &str) -> Option<&str> {
                if let Some(metadata) = get_entity_metadata(entity_name) {
                    Some(metadata.primary_key_type)
                } else {
                    None
                }
            }

            fn get_foreign_key_type(&self, entity_name: &str, field_name: &str) -> Option<&str> {
                if let Some(metadata) = get_entity_metadata(entity_name) {
                    metadata.foreign_key_types.iter()
                        .find(|(field, _)| *field == field_name)
                        .map(|(_, type_id)| *type_id)
                } else {
                    None
                }
            }

            fn convert_key_for_primary_key(&self, entity: &str, key: caustics::CausticsKey) -> Box<dyn std::any::Any + Send + Sync> {
                // Get the expected type for this entity's primary key
                if let Some(metadata) = get_entity_metadata(entity) {
                    // Use the unified conversion function
                    caustics::convert_key_to_type_from_string::<()>(key, metadata.primary_key_type)
                } else {
                    // No metadata available, return the key as-is
                    match key {
                        caustics::CausticsKey::I8(value) => Box::new(value),
                        caustics::CausticsKey::I16(value) => Box::new(value),
                        caustics::CausticsKey::I32(value) => Box::new(value),
                        caustics::CausticsKey::I64(value) => Box::new(value),
                        caustics::CausticsKey::ISize(value) => Box::new(value),
                        caustics::CausticsKey::U8(value) => Box::new(value),
                        caustics::CausticsKey::U16(value) => Box::new(value),
                        caustics::CausticsKey::U32(value) => Box::new(value),
                        caustics::CausticsKey::U64(value) => Box::new(value),
                        caustics::CausticsKey::USize(value) => Box::new(value),
                        caustics::CausticsKey::F32(value) => Box::new(value),
                        caustics::CausticsKey::F64(value) => Box::new(value),
                        caustics::CausticsKey::String(value) => Box::new(value),
                        caustics::CausticsKey::Bool(value) => Box::new(value),
                        caustics::CausticsKey::Uuid(value) => Box::new(value),
                        caustics::CausticsKey::DateTimeUtc(value) => Box::new(value),
                        caustics::CausticsKey::NaiveDateTime(value) => Box::new(value),
                        caustics::CausticsKey::NaiveDate(value) => Box::new(value),
                        caustics::CausticsKey::NaiveTime(value) => Box::new(value),
                        caustics::CausticsKey::Json(value) => Box::new(value),
                        caustics::CausticsKey::Composite(_) | caustics::CausticsKey::OptionalComposite(_) => {
                            panic!("Composite keys cannot be converted without metadata")
                        }
                    }
                }
            }

            fn convert_key_for_foreign_key(&self, entity: &str, field: &str, key: caustics::CausticsKey) -> Box<dyn std::any::Any + Send + Sync> {
                // Get the expected type for this entity's foreign key field
                if let Some(metadata) = get_entity_metadata(entity) {
                    // Find the type for this specific foreign key field
                    let field_type = metadata.foreign_key_types.iter()
                        .find(|(field_name, _)| *field_name == field)
                        .map(|(_, type_id)| *type_id);

                    match field_type {
                        Some(type_id) => {
                            // Use the unified conversion function for all types
                            caustics::convert_key_to_type_from_string::<()>(key, type_id)
                        },
                        None => {
                            // No type info for this field, use the unified conversion with generic type
                            caustics::convert_key_to_type_from_string::<()>(key, "unknown")
                        }
                    }
                } else {
                    // No metadata available, use the unified conversion with generic type
                    caustics::convert_key_to_type_from_string::<()>(key, "unknown")
                }
            }
        }

        // Use a static registry instance
        static REGISTRY: CompositeEntityRegistry = CompositeEntityRegistry;
        pub fn get_registry() -> &'static CompositeEntityRegistry {
            &REGISTRY
        }

        // Implement EntityMetadataProvider for the generated client
        impl caustics::EntityMetadataProvider for CompositeEntityRegistry {
            fn get_entity_metadata(&self, entity_name: &str) -> Option<&'static caustics::EntityMetadata> {
                get_entity_metadata(entity_name)
            }
        }

        // Helper functions for macro-generated code to use registry-based type conversion
        pub fn __caustics_convert_key_for_primary_key(entity: &str, key: caustics::CausticsKey) -> Box<dyn std::any::Any + Send + Sync> {
            <CompositeEntityRegistry as caustics::EntityTypeRegistry>::convert_key_for_primary_key(&REGISTRY, entity, key)
        }

        pub fn __caustics_convert_key_for_foreign_key(entity: &str, field: &str, key: caustics::CausticsKey) -> Box<dyn std::any::Any + Send + Sync> {
            // Convert PascalCase field name to snake_case for lookup
            let field_snake = field.chars()
                .enumerate()
                .map(|(i, c)| {
                    if c.is_uppercase() && i > 0 {
                        format!("_{}", c.to_lowercase())
                    } else {
                        c.to_lowercase().to_string()
                    }
                })
                .collect::<String>();
            <CompositeEntityRegistry as caustics::EntityTypeRegistry>::convert_key_for_foreign_key(&REGISTRY, entity, &field_snake, key)
        }

        pub fn __caustics_get_primary_key_type(entity: &str) -> Option<&str> {
            <CompositeEntityRegistry as caustics::EntityTypeRegistry>::get_primary_key_type(&REGISTRY, entity)
        }

        pub fn __caustics_get_foreign_key_type<'a>(entity: &'a str, field: &'a str) -> Option<&'static str> {
            // Convert PascalCase field name to snake_case for lookup
            let field_snake = field.chars()
                .enumerate()
                .map(|(i, c)| {
                    if c.is_uppercase() && i > 0 {
                        format!("_{}", c.to_lowercase())
                    } else {
                        c.to_lowercase().to_string()
                    }
                })
                .collect::<String>();
            if let Some(metadata) = get_entity_metadata(entity) {
                metadata.foreign_key_types.iter()
                    .find(|(field_name, _)| *field_name == field_snake)
                    .map(|(_, type_id)| *type_id)
            } else {
                None
            }
        }

        // Helper function to convert CausticsKey to the actual field type dynamically
        pub fn __caustics_convert_key_to_field_type(
            entity: &str,
            field: &str,
            key: caustics::CausticsKey,
        ) -> Result<Box<dyn std::any::Any + Send + Sync>, String> {
            // Get the expected type for this field
            let field_type = if let Some(metadata) = get_entity_metadata(entity) {
                // Check if it's a primary key field
                if field == metadata.primary_key_field {
                    Some(metadata.primary_key_type)
                } else {
                    // Check if it's a foreign key field
                    metadata.foreign_key_types.iter()
                        .find(|(field_name, _)| *field_name == field)
                        .map(|(_, type_id)| *type_id)
                }
            } else {
                None
            };

            match field_type {
                Some(_type_id) => {
                    // Use the registry to convert to the correct type
                    let converted = <CompositeEntityRegistry as caustics::EntityTypeRegistry>::convert_key_for_foreign_key(&REGISTRY, entity, field, key);
                    Ok(converted)
                }
                None => {
                    Err(format!("No type information found for field {} in entity {}", field, entity))
                }
            }
        }

        // Helper function to get the actual field type for downcasting
        pub fn __caustics_get_field_type<'a>(entity: &'a str, field: &'a str) -> Option<&'static str> {
            if let Some(metadata) = get_entity_metadata(entity) {
                // Check if it's a primary key field
                if field == metadata.primary_key_field {
                    Some(metadata.primary_key_type)
                } else {
                    // Check if it's a foreign key field
                    metadata.foreign_key_types.iter()
                        .find(|(field_name, _)| *field_name == field)
                        .map(|(_, type_id)| *type_id)
                }
            } else {
                None
            }
        }

        // Comprehensive helper function to convert CausticsKey to the actual field type and downcast
        pub fn __caustics_convert_and_downcast(
            entity: &str,
            field: &str,
            key: caustics::CausticsKey,
        ) -> Result<Box<dyn std::any::Any + Send + Sync>, String> {
            // Get the field type
            let _field_type = __caustics_get_field_type(entity, field)
                .ok_or_else(|| format!("No type information found for field {} in entity {}", field, entity))?;

            // Convert using the registry
            let converted = if let Some(metadata) = get_entity_metadata(entity) {
                if field == metadata.primary_key_field {
                    <CompositeEntityRegistry as caustics::EntityTypeRegistry>::convert_key_for_primary_key(&REGISTRY, entity, key)
                } else {
                    <CompositeEntityRegistry as caustics::EntityTypeRegistry>::convert_key_for_foreign_key(&REGISTRY, entity, field, key)
                }
            } else {
                return Err(format!("No metadata found for entity {}", entity));
            };

            Ok(converted)
        }

        // Helper function to convert CausticsKey to the actual field type for use in SeaORM operations
        pub fn __caustics_convert_key_for_sea_orm(
            entity: &str,
            field: &str,
            key: caustics::CausticsKey,
        ) -> Result<caustics::sea_orm::Value, String> {
            // Convert using the registry
            let converted = __caustics_convert_and_downcast(entity, field, key)?;

            // Get the field type to determine how to convert to caustics::sea_orm::Value
            let field_type = __caustics_get_field_type(entity, field)
                .ok_or_else(|| format!("No type information found for field {} in entity {}", field, entity))?;

            // Convert to caustics::sea_orm::Value based on the actual field type
            match field_type {
                "i8" => {
                    converted.downcast::<i8>().map(|v| caustics::sea_orm::Value::TinyInt(Some(*v)))
                        .map_err(|_| "Failed to downcast to i8".to_string())
                },
                "i16" => {
                    converted.downcast::<i16>().map(|v| caustics::sea_orm::Value::SmallInt(Some(*v)))
                        .map_err(|_| "Failed to downcast to i16".to_string())
                },
                "i32" => {
                    converted.downcast::<i32>().map(|v| caustics::sea_orm::Value::Int(Some(*v)))
                        .map_err(|_| "Failed to downcast to i32".to_string())
                },
                "i64" => {
                    converted.downcast::<i64>().map(|v| caustics::sea_orm::Value::BigInt(Some(*v)))
                        .map_err(|_| "Failed to downcast to i64".to_string())
                },
                "u8" => {
                    converted.downcast::<u8>().map(|v| caustics::sea_orm::Value::TinyUnsigned(Some(*v)))
                        .map_err(|_| "Failed to downcast to u8".to_string())
                },
                "u16" => {
                    converted.downcast::<u16>().map(|v| caustics::sea_orm::Value::SmallUnsigned(Some(*v)))
                        .map_err(|_| "Failed to downcast to u16".to_string())
                },
                "u32" => {
                    converted.downcast::<u32>().map(|v| caustics::sea_orm::Value::Unsigned(Some(*v)))
                        .map_err(|_| "Failed to downcast to u32".to_string())
                },
                "u64" => {
                    converted.downcast::<u64>().map(|v| caustics::sea_orm::Value::BigUnsigned(Some(*v)))
                        .map_err(|_| "Failed to downcast to u64".to_string())
                },
                "f32" => {
                    converted.downcast::<f32>().map(|v| caustics::sea_orm::Value::Float(Some(*v)))
                        .map_err(|_| "Failed to downcast to f32".to_string())
                },
                "f64" => {
                    converted.downcast::<f64>().map(|v| caustics::sea_orm::Value::Double(Some(*v)))
                        .map_err(|_| "Failed to downcast to f64".to_string())
                },
                "String" | "str" => {
                    converted.downcast::<String>().map(|v| caustics::sea_orm::Value::String(Some(Box::new(*v))))
                        .map_err(|_| "Failed to downcast to String".to_string())
                },
                "bool" => {
                    converted.downcast::<bool>().map(|v| caustics::sea_orm::Value::Bool(Some(*v)))
                        .map_err(|_| "Failed to downcast to bool".to_string())
                },
                "uuid::Uuid" => {
                    converted.downcast::<uuid::Uuid>().map(|v| caustics::sea_orm::Value::Uuid(Some(Box::new(*v))))
                        .map_err(|_| "Failed to downcast to Uuid".to_string())
                },
                "caustics::chrono::DateTime<caustics::chrono::Utc>" => {
                    converted.downcast::<caustics::chrono::DateTime<caustics::chrono::Utc>>().map(|v| caustics::sea_orm::Value::ChronoDateTimeUtc(Some(v)))
                        .map_err(|_| "Failed to downcast to DateTime<Utc>".to_string())
                },
                "caustics::chrono::NaiveDateTime" => {
                    converted.downcast::<caustics::chrono::NaiveDateTime>().map(|v| caustics::sea_orm::Value::ChronoDateTime(Some(v)))
                        .map_err(|_| "Failed to downcast to NaiveDateTime".to_string())
                },
                "caustics::chrono::NaiveDate" => {
                    converted.downcast::<caustics::chrono::NaiveDate>().map(|v| caustics::sea_orm::Value::ChronoDate(Some(Box::new(*v))))
                        .map_err(|_| "Failed to downcast to NaiveDate".to_string())
                },
                "caustics::chrono::NaiveTime" => {
                    converted.downcast::<caustics::chrono::NaiveTime>().map(|v| caustics::sea_orm::Value::ChronoTime(Some(Box::new(*v))))
                        .map_err(|_| "Failed to downcast to NaiveTime".to_string())
                },
                "caustics::serde_json::Value" => {
                    converted.downcast::<caustics::serde_json::Value>().map(|v| caustics::sea_orm::Value::Json(Some(v)))
                        .map_err(|_| "Failed to downcast to Json".to_string())
                },
                _ => {
                    Err(format!("Unsupported field type '{}' for field {} in entity {}", field_type, field, entity))
                }
            }
        }


        // Helper function to convert the converted value to the appropriate ActiveValue type using field type information
        pub fn __caustics_convert_to_active_value_with_type_info(
            entity: &str,
            field: &str,
            converted: Box<dyn std::any::Any + Send + Sync>,
        ) -> Box<dyn std::any::Any + Send + Sync> {
            // Get the expected type for this field
            let field_type = if let Some(metadata) = get_entity_metadata(entity) {
                // Check if it's a primary key field
                if field == metadata.primary_key_field {
                    Some(metadata.primary_key_type)
                } else {
                    // Check if it's a foreign key field
                    metadata.foreign_key_types.iter()
                        .find(|(field_name, _)| *field_name == field)
                        .map(|(_, type_id)| *type_id)
                }
            } else {
                None
            };

            match field_type {
                Some(type_id) => {
                    // Convert based on the actual field type
                    match type_id {
                        "i8" => {
                        if let Ok(v) = converted.downcast::<i8>() {
                            Box::new(caustics::sea_orm::ActiveValue::Set(*v))
                        } else {
                            panic!("Failed to downcast to i8 for field {}", field);
                        }
                        },
                        "i16" => {
                        if let Ok(v) = converted.downcast::<i16>() {
                            Box::new(caustics::sea_orm::ActiveValue::Set(*v))
                        } else {
                            panic!("Failed to downcast to i16 for field {}", field);
                        }
                        },
                        "i32" => {
                        if let Ok(v) = converted.downcast::<i32>() {
                            Box::new(caustics::sea_orm::ActiveValue::Set(*v))
                        } else {
                            panic!("Failed to downcast to i32 for field {}", field);
                        }
                        },
                        "i64" => {
                        if let Ok(v) = converted.downcast::<i64>() {
                            Box::new(caustics::sea_orm::ActiveValue::Set(*v))
                        } else {
                            panic!("Failed to downcast to i64 for field {}", field);
                        }
                        },
                        "u8" => {
                        if let Ok(v) = converted.downcast::<u8>() {
                            Box::new(caustics::sea_orm::ActiveValue::Set(*v))
                        } else {
                            panic!("Failed to downcast to u8 for field {}", field);
                        }
                        },
                        "u16" => {
                        if let Ok(v) = converted.downcast::<u16>() {
                            Box::new(caustics::sea_orm::ActiveValue::Set(*v))
                        } else {
                            panic!("Failed to downcast to u16 for field {}", field);
                        }
                        },
                        "u32" => {
                        if let Ok(v) = converted.downcast::<u32>() {
                            Box::new(caustics::sea_orm::ActiveValue::Set(*v))
                        } else {
                            panic!("Failed to downcast to u32 for field {}", field);
                        }
                        },
                        "u64" => {
                        if let Ok(v) = converted.downcast::<u64>() {
                            Box::new(caustics::sea_orm::ActiveValue::Set(*v))
                        } else {
                            panic!("Failed to downcast to u64 for field {}", field);
                        }
                        },
                        "String" | "str" => {
                        if let Ok(v) = converted.downcast::<String>() {
                                let string_value = *v;
                                Box::new(caustics::sea_orm::ActiveValue::Set(string_value))
                        } else {
                            panic!("Failed to downcast to String for field {}", field);
                        }
                        },
                        "uuid::Uuid" => {
                        if let Ok(v) = converted.downcast::<uuid::Uuid>() {
                            Box::new(caustics::sea_orm::ActiveValue::Set(*v))
                        } else {
                            panic!("Failed to downcast to Uuid for field {}", field);
                        }
                        },
                        "Uuid" => {
                        if let Ok(v) = converted.downcast::<uuid::Uuid>() {
                            Box::new(caustics::sea_orm::ActiveValue::Set(*v))
                        } else {
                            panic!("Failed to downcast to Uuid for field {}", field);
                        }
                        },
                        _ => {
                            panic!("Unsupported field type '{}' for field {} in entity {}", type_id, field, entity);
                        }
                    }
                }
                None => {
                    panic!("No type information found for field {} in entity {}", field, entity);
                }
            }
        }

        // Encapsulated helper function to convert CausticsKey to ActiveValue with dynamic type resolution
        pub fn __caustics_convert_key_to_active_value(
            entity: &str,
            field: &str,
            key: caustics::CausticsKey,
        ) -> Box<dyn std::any::Any + Send + Sync> {
            let converted = __caustics_convert_key_for_foreign_key(entity, field, key);
            let field_type = __caustics_get_foreign_key_type(entity, field)
                .expect("Failed to get field type information");

            // Return the appropriate ActiveValue based on field type
            match field_type {
                "i8" => {
                Box::new(caustics::sea_orm::ActiveValue::Set(*converted.downcast::<i8>().expect("Failed to convert to i8")))
                },
                "i16" => {
                Box::new(caustics::sea_orm::ActiveValue::Set(*converted.downcast::<i16>().expect("Failed to convert to i16")))
                },
                "i32" => {
                Box::new(caustics::sea_orm::ActiveValue::Set(*converted.downcast::<i32>().expect("Failed to convert to i32")))
                },
                "i64" => {
                Box::new(caustics::sea_orm::ActiveValue::Set(*converted.downcast::<i64>().expect("Failed to convert to i64")))
                },
                "u8" => {
                Box::new(caustics::sea_orm::ActiveValue::Set(*converted.downcast::<u8>().expect("Failed to convert to u8")))
                },
                "u16" => {
                Box::new(caustics::sea_orm::ActiveValue::Set(*converted.downcast::<u16>().expect("Failed to convert to u16")))
                },
                "u32" => {
                Box::new(caustics::sea_orm::ActiveValue::Set(*converted.downcast::<u32>().expect("Failed to convert to u32")))
                },
                "u64" => {
                Box::new(caustics::sea_orm::ActiveValue::Set(*converted.downcast::<u64>().expect("Failed to convert to u64")))
                },
                "String" | "str" => {
                    let string_value = *converted.downcast::<String>().expect("Failed to convert to String");
                    Box::new(caustics::sea_orm::ActiveValue::Set(string_value))
                },
                "uuid::Uuid" => {
                    Box::new(caustics::sea_orm::ActiveValue::Set(*converted.downcast::<uuid::Uuid>().expect("Failed to convert to Uuid")))
                },
                "Uuid" => {
                    Box::new(caustics::sea_orm::ActiveValue::Set(*converted.downcast::<uuid::Uuid>().expect("Failed to convert to Uuid")))
                },
                _ => {
                    panic!("Unsupported foreign key type '{}' for field {} in entity {}", field_type, field, entity);
                }
            }
        }

        // Helper function for optional foreign keys (wraps in Some)
        pub fn __caustics_convert_key_to_active_value_optional(
            entity: &str,
            field: &str,
            key: caustics::CausticsKey,
        ) -> Box<dyn std::any::Any + Send + Sync> {
            let converted = __caustics_convert_key_for_foreign_key(entity, field, key);
            let field_type = __caustics_get_foreign_key_type(entity, field)
                .expect("Failed to get field type information");

            // Return the appropriate ActiveValue with Some() wrapper for optional fields
            match field_type {
                "i8" => {
                Box::new(caustics::sea_orm::ActiveValue::Set(Some(*converted.downcast::<i8>().expect("Failed to convert to i8"))))
                },
                "i16" => {
                Box::new(caustics::sea_orm::ActiveValue::Set(Some(*converted.downcast::<i16>().expect("Failed to convert to i16"))))
                },
                "i32" => {
                Box::new(caustics::sea_orm::ActiveValue::Set(Some(*converted.downcast::<i32>().expect("Failed to convert to i32"))))
                },
                "i64" => {
                Box::new(caustics::sea_orm::ActiveValue::Set(Some(*converted.downcast::<i64>().expect("Failed to convert to i64"))))
                },
                "u8" => {
                Box::new(caustics::sea_orm::ActiveValue::Set(Some(*converted.downcast::<u8>().expect("Failed to convert to u8"))))
                },
                "u16" => {
                Box::new(caustics::sea_orm::ActiveValue::Set(Some(*converted.downcast::<u16>().expect("Failed to convert to u16"))))
                },
                "u32" => {
                Box::new(caustics::sea_orm::ActiveValue::Set(Some(*converted.downcast::<u32>().expect("Failed to convert to u32"))))
                },
                "u64" => {
                Box::new(caustics::sea_orm::ActiveValue::Set(Some(*converted.downcast::<u64>().expect("Failed to convert to u64"))))
                },
                "String" | "str" => {
                    let string_value = *converted.downcast::<String>().expect("Failed to convert to String");
                    Box::new(caustics::sea_orm::ActiveValue::Set(Some(string_value)))
                },
                "uuid::Uuid" => {
                Box::new(caustics::sea_orm::ActiveValue::Set(Some(*converted.downcast::<uuid::Uuid>().expect("Failed to convert to Uuid"))))
                },
                "Uuid" => {
                Box::new(caustics::sea_orm::ActiveValue::Set(Some(*converted.downcast::<uuid::Uuid>().expect("Failed to convert to Uuid"))))
                },
                _ => {
                    panic!("Unsupported foreign key type '{}' for field {} in entity {}", field_type, field, entity);
                }
            }
        }

        #[allow(dead_code)]
        impl CausticsClient {
            pub fn new(db: DatabaseConnection) -> Self {
                use caustics::sea_orm::ConnectionTrait;
                let database_backend = db.get_database_backend();
                Self {
                    db: std::sync::Arc::new(db),
                    database_backend,
                }
            }

            pub fn db(&self) -> std::sync::Arc<DatabaseConnection> {
                self.db.clone()
            }

            pub fn database_backend(&self) -> caustics::sea_orm::DatabaseBackend {
                self.database_backend
            }

            pub fn _transaction(&self) -> TransactionBuilder {
                TransactionBuilder {
                    db: self.db.clone(),
                    database_backend: self.database_backend,
                }
            }

            // Prisma-style name (without $): alias to _transaction
            pub fn transaction(&self) -> TransactionBuilder {
                self._transaction()
            }

            // Raw SQL APIs
            pub fn _query_raw<T>(&self, raw: Raw) -> RawQuery<T> {
                RawQuery { db: self.db.clone(), backend: self.database_backend, raw, _marker: std::marker::PhantomData }
            }

            pub fn _execute_raw(&self, raw: Raw) -> RawExecute {
                RawExecute { db: self.db.clone(), backend: self.database_backend, raw }
            }

            pub async fn _batch<'a, Entity, ActiveModel, ModelWithRelations, T, Container>(
                &self,
                queries: Container,
            ) -> Result<Container::ReturnType, caustics::sea_orm::DbErr>
            where
                Entity: caustics::sea_orm::EntityTrait,
                ActiveModel: caustics::sea_orm::ActiveModelTrait<Entity = Entity> + caustics::sea_orm::ActiveModelBehavior + Send + 'static,
                ModelWithRelations: #from_model<<Entity as caustics::sea_orm::EntityTrait>::Model>
                    + caustics::HasRelationMetadata<ModelWithRelations>
                    + caustics::ApplyNestedIncludes<caustics::sea_orm::DatabaseTransaction>,
                T: #merge_into<ActiveModel>,
                <Entity as caustics::sea_orm::EntityTrait>::Model: caustics::sea_orm::IntoActiveModel<ActiveModel>,
                Container: #batch_container<'a, caustics::sea_orm::DatabaseConnection, Entity, ActiveModel, ModelWithRelations, T>,
            {
                let txn = self.db.begin().await?;
                let batch_queries = Container::into_queries(queries);
                let mut results = Vec::with_capacity(batch_queries.len());

                for query in batch_queries {
                    let res = match query {
                        #batch_query::Insert(q) => {
                            // For Insert, use exec_in_txn to use the transaction
                            let result = q.exec_in_txn(&txn).await?;
                            #batch_result::Insert(result)
                        }
                        #batch_query::Update(q) => {
                            let result = q.exec_in_txn(&txn).await?;
                            #batch_result::Update(result)
                        }
                        #batch_query::Delete(q) => {
                            let result = q.exec_in_txn(&txn).await?;
                            #batch_result::Delete(result)
                        }
                        #batch_query::Upsert(q) => {
                            // For Upsert, use exec_in_txn to use the transaction
                            let result = q.exec_in_txn(&txn).await?;
                            #batch_result::Upsert(result)
                        }
                    };
                    results.push(res);
                }

                txn.commit().await?;
                Ok(Container::from_results(results))
            }

            #(#entity_methods)*
        }

        // Crate-level prelude that re-exports all entity extension traits collected from entity modules
        #prelude_block

        #[allow(dead_code)]
        impl TransactionCausticsClient {
            pub fn new(tx: std::sync::Arc<DatabaseTransaction>, database_backend: caustics::sea_orm::DatabaseBackend) -> Self {
                Self { tx, database_backend }
            }

            #(#tx_entity_methods)*

            // Raw SQL APIs within a transaction
            pub fn _query_raw<T>(&self, raw: Raw) -> TxRawQuery<T> {
                TxRawQuery { tx: self.tx.clone(), backend: self.database_backend, raw, _marker: std::marker::PhantomData }
            }

            pub fn _execute_raw(&self, raw: Raw) -> TxRawExecute {
                TxRawExecute { tx: self.tx.clone(), backend: self.database_backend, raw }
            }

            // Transaction-scoped hook installer (overrides global while running in this thread)
            pub fn with_hook<F, Fut, T>(&self, hook: std::sync::Arc<dyn #hooks_mod::QueryHook>, f: F) -> std::pin::Pin<Box<dyn std::future::Future<Output=Result<T, caustics::sea_orm::DbErr>> + Send + '_>>
            where
                F: FnOnce(Self) -> Fut + Send + 'static,
                Fut: std::future::Future<Output = Result<T, caustics::sea_orm::DbErr>> + Send + 'static,
                T: Send + 'static,
            {
                Box::pin(async move {
                    #hooks_mod::set_thread_hook(Some(hook));
                    let _corr = #hooks_mod::set_new_correlation_id();
                    let res = f(TransactionCausticsClient::new(self.tx.clone(), self.database_backend)).await;
                    #hooks_mod::set_thread_hook(None);
                    #hooks_mod::set_thread_correlation_id(None);
                    res
                })
            }
        }

        pub struct TxRawQuery<T> {
            tx: std::sync::Arc<DatabaseTransaction>,
            backend: caustics::sea_orm::DatabaseBackend,
            raw: Raw,
            _marker: std::marker::PhantomData<T>,
        }

        impl<T> TxRawQuery<T> {
            pub async fn exec(self) -> Result<Vec<T>, caustics::sea_orm::DbErr>
            where
                T: caustics::sea_orm::FromQueryResult + Send + Sync + 'static,
            {
                use caustics::sea_orm::{Statement, SelectorRaw, SelectModel};
                let stmt = Statement::from_sql_and_values(self.backend, self.raw.sql, self.raw.params);
                let rows = SelectorRaw::<SelectModel<T>>::from_statement(stmt).all(self.tx.as_ref()).await?;
                Ok(rows)
            }
        }

        pub struct TxRawExecute {
            tx: std::sync::Arc<DatabaseTransaction>,
            backend: caustics::sea_orm::DatabaseBackend,
            raw: Raw,
        }

        impl TxRawExecute {
            pub async fn exec(self) -> Result<caustics::sea_orm::ExecResult, caustics::sea_orm::DbErr> {
                use caustics::sea_orm::{Statement, ConnectionTrait};
                let stmt = Statement::from_sql_and_values(self.backend, self.raw.sql, self.raw.params);
                let res = self.tx.execute(stmt).await?;
                Ok(res)
            }
        }

        impl TransactionBuilder {
            pub async fn run<F, Fut, T>(&self, f: F) -> Result<T, caustics::sea_orm::DbErr>
            where
                F: FnOnce(TransactionCausticsClient) -> Fut,
                Fut: std::future::Future<Output = Result<T, caustics::sea_orm::DbErr>>,
            {
                let tx = self.db.begin().await?;
                let tx_arc = std::sync::Arc::new(tx);
                let tx_client = TransactionCausticsClient::new(tx_arc.clone(), self.database_backend);
                let result = f(tx_client).await;
                let tx = std::sync::Arc::try_unwrap(tx_arc).expect("Transaction Arc should be unique");
                match result {
                    Ok(val) => {
                        tx.commit().await?;
                        Ok(val)
                    }
                    Err(e) => {
                        tx.rollback().await?;
                        Err(e)
                    }
                }
            }
        }

    };

    client_code.to_string()
}

/// Generate client code for external projects (like examples)
pub fn generate_caustics_client(
    dirs: &[&str],
    out_file: &str,
) -> Result<(), Box<dyn std::error::Error>> {
    let out_dir = std::env::var("OUT_DIR")?;
    let out_path = std::path::Path::new(&out_dir).join(out_file);

    let mut entities: Vec<(String, String, String, String)> = Vec::new(); // (entity_name, module_path, module_name, source_file)

    for dir in dirs {
        for entry in walkdir::WalkDir::new(dir) {
            let entry = entry?;
            if entry.path().extension().is_some_and(|ext| ext == "rs") {
                let content = std::fs::read_to_string(entry.path())?;
                let file = syn::parse_file(&content)?;

                for item in file.items {
                    if let syn::Item::Mod(module) = &item {
                        let module_name = module.ident.to_string();
                        if let Some((_, items)) = &module.content {
                            let has_caustics_attr = has_caustics_attribute(&module.attrs);

                            for item in items {
                                if let syn::Item::Struct(struct_item) = item {
                                    if struct_item.ident == "Model"
                                        && (has_caustics_attr
                                            || has_caustics_derive(&struct_item.attrs))
                                        {
                                            let entity_name = module_name.to_pascal_case();

                                            // Construct the full module path from the file structure
                                            let file_path = entry.path();
                                            let dir_path = std::path::Path::new(dir);
                                            let relative_path = file_path
                                                .strip_prefix(dir_path)
                                                .unwrap_or(file_path);

                                            // Convert file path to module path
                                            let path_str = relative_path.to_string_lossy();
                                            let module_path = if path_str.ends_with(".rs") {
                                                // Remove .rs extension and convert path separators to ::
                                                let without_ext = path_str.trim_end_matches(".rs");
                                                without_ext.replace(std::path::MAIN_SEPARATOR, "::")
                                            } else {
                                                module_name.clone()
                                            };
                                            let source_file =
                                                entry.path().to_string_lossy().to_string();
                                            entities.push((
                                                entity_name,
                                                module_path,
                                                module_name.clone(),
                                                source_file,
                                            ));
                                        }
                                }
                            }

                            // Note: Entity is already added above when Model struct is found with caustics attributes
                        }
                    }
                }
            }
        }
    }

    // Extract metadata for all entities
    let mut entities_metadata = Vec::new();
    for (entity_name, module_path, module_name, source_file) in &entities {
        if let Some(metadata) =
            extract_entity_metadata(source_file, entity_name, module_path, module_name)
        {
            entities_metadata.push(metadata);
        }
    }

    // Resolve target table names for relations
    resolve_target_table_names(&mut entities_metadata);

    // Convert entities to the format expected by generate_client_code
    let entities_for_codegen: Vec<(String, String)> = entities
        .iter()
        .map(|(_name, path, module_name, _)| {
            // Use the original module name for import paths, not the PascalCase entity name
            (module_name.clone(), path.clone())
        })
        .collect();

    let client_code = generate_client_code(&entities_for_codegen, &entities_metadata, true);
    std::fs::write(out_path, client_code)?;

    Ok(())
}
