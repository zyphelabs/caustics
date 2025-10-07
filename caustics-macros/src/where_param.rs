// Logic for generating field variants for WhereParam enum and their filter functions.
// This module will support all types and will be long.

use heck::{ToPascalCase, ToSnakeCase};
use quote::{format_ident, quote};

/// Generate field variants, match arms, and field operator modules for WhereParam enum and filters.
pub fn generate_where_param_logic(
    fields: &[&syn::Field],
    unique_fields: &[&syn::Field],
    primary_key_fields: &[&syn::Field],
    full_mod_path: &syn::Path,
    relations: &[crate::entity::Relation],
    entity_name: &str,
) -> (
    Vec<proc_macro2::TokenStream>,
    Vec<proc_macro2::TokenStream>,
    Vec<proc_macro2::TokenStream>,
) {
    let mut where_field_variants: Vec<proc_macro2::TokenStream> = Vec::new();
    let mut field_ops: Vec<proc_macro2::TokenStream> = Vec::new();
    for field in fields.iter() {
        let name = field.ident.as_ref().unwrap();
        let pascal_name = format_ident!("{}", name.to_string().to_pascal_case());
        let ty = &field.ty;
        let is_unique = unique_fields
            .iter()
            .any(|unique_field| unique_field.ident.as_ref().unwrap() == name);

        let is_primary_key = primary_key_fields
            .iter()
            .any(|pk_field| pk_field.ident.as_ref().unwrap() == name);

        // Detect field type for appropriate operation generation
        let field_type = detect_field_type(ty);

        // WhereParam variant uses FieldOp directly with sea_orm::Value
        where_field_variants.push(quote! { #pascal_name(caustics::FieldOp) });

        // Field operator module
        let set_fn = if !is_unique {
            quote! {
                pub fn set<T: Into<#ty>>(value: T) -> super::SetParam {
                    super::SetParam::#pascal_name(sea_orm::ActiveValue::Set(value.into()))
                }
            }
        } else {
            quote! {}
        };

        // Unique where function
        let unique_where_fn = if is_unique {
            let equals_variant = format_ident!("{}Equals", pascal_name);
            if is_primary_key {
                // For primary key fields that are also unique, accept CausticsKey directly
                quote! {
                    pub fn equals<T: From<Equals>>(value: impl Into<caustics::CausticsKey>) -> T {
                        let key = value.into();
                        Equals(key).into()
                    }
                    pub struct Equals(pub caustics::CausticsKey);
                    impl From<Equals> for super::UniqueWhereParam {
                        fn from(Equals(v): Equals) -> Self {
                            super::UniqueWhereParam::#equals_variant(v.clone())
                        }
                    }
                    impl From<Equals> for super::WhereParam {
                        fn from(Equals(v): Equals) -> Self {
                            super::WhereParam::#pascal_name(caustics::FieldOp::equals(v))
                        }
                    }
                }
            } else {
                // For other unique fields, use the field's actual type
                quote! {
                    pub fn equals<T: From<Equals>>(value: impl Into<#ty>) -> T {
                        Equals(value.into()).into()
                    }
                    pub struct Equals(pub #ty);
                    impl From<Equals> for super::UniqueWhereParam {
                        fn from(Equals(v): Equals) -> Self {
                            super::UniqueWhereParam::#equals_variant(v)
                        }
                    }
                    impl From<Equals> for super::WhereParam {
                        fn from(Equals(v): Equals) -> Self {
                            super::WhereParam::#pascal_name(caustics::FieldOp::equals(v))
                        }
                    }
                }
            }
        } else {
            quote! {}
        };

        // Order by function
        let order_fn = quote! {
            pub fn order(sort_order: caustics::SortOrder) -> super::OrderByParam {
                super::OrderByParam::#pascal_name(sort_order)
            }
            pub fn order_nulls(sort_order: caustics::SortOrder, nulls: caustics::NullsOrder) -> (super::OrderByParam, caustics::NullsOrder) {
                (super::OrderByParam::#pascal_name(sort_order), nulls)
            }
        };

        // Relation-aggregate helper: count order (returns SortOrder to feed relation::order_by)
        let count_fn = quote! {
            pub fn count(order: caustics::SortOrder) -> caustics::SortOrder { order }
        };

        // Generate type-specific operations
        let type_specific_ops = if is_primary_key {
            // For primary key fields, generate operations that accept CausticsKey
            generate_primary_key_operations(&field_type, &pascal_name, ty)
        } else if !is_unique {
            generate_type_specific_operations(&field_type, &pascal_name, ty)
        } else {
            quote! {} // Don't generate equals for unique fields since unique_where_fn already has it
        };

        // Field-level `not` alias: maps to NotEquals with same value type
        // Skip for primary key fields (handled by generate_primary_key_operations)
        let field_not_alias = if !is_primary_key {
            quote! {
                pub fn not<T: caustics::ToSeaOrmValue>(value: T) -> WhereParam {
                    WhereParam::#pascal_name(caustics::FieldOp::not_equals(value))
                }
            }
        } else {
            quote! {}
        };

        // String ops (only for string types)
        let string_ops = match field_type {
            FieldType::String | FieldType::OptionString => {
                quote! {
                    pub fn contains<T: Into<String>>(value: T) -> WhereParam {
                        WhereParam::#pascal_name(caustics::FieldOp::Contains(value.into()))
                    }
                    pub fn starts_with<T: Into<String>>(value: T) -> WhereParam {
                        WhereParam::#pascal_name(caustics::FieldOp::StartsWith(value.into()))
                    }
                    pub fn ends_with<T: Into<String>>(value: T) -> WhereParam {
                        WhereParam::#pascal_name(caustics::FieldOp::EndsWith(value.into()))
                    }
                }
            }
            _ => quote! {},
        };

        // Common comparison operations (for most types except boolean)
        // Skip for primary key fields (handled by generate_primary_key_operations)
        let comparison_ops = if !is_primary_key
            && !matches!(
                field_type,
                FieldType::Boolean
                    | FieldType::OptionBoolean
                    | FieldType::Json
                    | FieldType::OptionJson
                    | FieldType::Uuid
                    | FieldType::OptionUuid
            ) {
            quote! {
            pub fn not_equals<T: caustics::ToSeaOrmValue>(value: T) -> WhereParam {
                WhereParam::#pascal_name(caustics::FieldOp::not_equals(value))
            }
            pub fn gt<T: caustics::ToSeaOrmValue>(value: T) -> WhereParam {
                WhereParam::#pascal_name(caustics::FieldOp::gt(value))
            }
            pub fn lt<T: caustics::ToSeaOrmValue>(value: T) -> WhereParam {
                WhereParam::#pascal_name(caustics::FieldOp::lt(value))
            }
            pub fn gte<T: caustics::ToSeaOrmValue>(value: T) -> WhereParam {
                WhereParam::#pascal_name(caustics::FieldOp::gte(value))
            }
            pub fn lte<T: caustics::ToSeaOrmValue>(value: T) -> WhereParam {
                WhereParam::#pascal_name(caustics::FieldOp::lte(value))
            }
            }
        } else if !is_primary_key {
            // For boolean, UUID, and JSON fields, only provide equals/not_equals
            quote! {
                pub fn not_equals<T: caustics::ToSeaOrmValue>(value: T) -> WhereParam {
                    WhereParam::#pascal_name(caustics::FieldOp::not_equals(value))
                }
            }
        } else {
            quote! {}
        };

        // Collection operations (for all types)
        // Skip for primary key fields (handled by generate_primary_key_operations)
        let collection_ops = if !is_primary_key {
            quote! {
                pub fn in_vec<T: caustics::ToSeaOrmValue>(values: Vec<T>) -> WhereParam {
                    WhereParam::#pascal_name(caustics::FieldOp::in_vec(values))
                }
                pub fn not_in_vec<T: caustics::ToSeaOrmValue>(values: Vec<T>) -> WhereParam {
                    WhereParam::#pascal_name(caustics::FieldOp::not_in_vec(values))
                }
            }
        } else {
            quote! {}
        };

        // Null operations (only for nullable types)
        let null_ops = match field_type {
            FieldType::OptionString
            | FieldType::OptionInteger
            | FieldType::OptionFloat
            | FieldType::OptionBoolean
            | FieldType::OptionDateTime
            | FieldType::OptionUuid
            | FieldType::OptionJson => {
                quote! {
                    pub fn is_null() -> WhereParam {
                        WhereParam::#pascal_name(caustics::FieldOp::IsNull)
                    }
                    pub fn is_not_null() -> WhereParam {
                        WhereParam::#pascal_name(caustics::FieldOp::IsNotNull)
                    }
                }
            }
            _ => quote! {},
        };

        // JSON-specific operations (only for JSON types)
        let json_ops = match field_type {
            FieldType::Json => {
                quote! {
                    pub fn path(path: Vec<String>) -> WhereParam {
                        WhereParam::#pascal_name(caustics::FieldOp::JsonPath(path))
                    }
                    pub fn json_string_contains(value: String) -> WhereParam {
                        WhereParam::#pascal_name(caustics::FieldOp::JsonStringContains(value))
                    }
                    pub fn json_string_starts_with(value: String) -> WhereParam {
                        WhereParam::#pascal_name(caustics::FieldOp::JsonStringStartsWith(value))
                    }
                    pub fn json_string_ends_with(value: String) -> WhereParam {
                        WhereParam::#pascal_name(caustics::FieldOp::JsonStringEndsWith(value))
                    }
                    pub fn json_array_contains(value: serde_json::Value) -> WhereParam {
                        WhereParam::#pascal_name(caustics::FieldOp::JsonArrayContains(value))
                    }
                    pub fn json_array_starts_with(value: serde_json::Value) -> WhereParam {
                        WhereParam::#pascal_name(caustics::FieldOp::JsonArrayStartsWith(value))
                    }
                    pub fn json_array_ends_with(value: serde_json::Value) -> WhereParam {
                        WhereParam::#pascal_name(caustics::FieldOp::JsonArrayEndsWith(value))
                    }
                    pub fn json_object_contains(key: String) -> WhereParam {
                        WhereParam::#pascal_name(caustics::FieldOp::JsonObjectContains(key))
                    }
                    pub fn db_null() -> WhereParam { WhereParam::#pascal_name(caustics::FieldOp::JsonNull(caustics::JsonNullValueFilter::DbNull)) }
                    pub fn json_null() -> WhereParam { WhereParam::#pascal_name(caustics::FieldOp::JsonNull(caustics::JsonNullValueFilter::JsonNull)) }
                    pub fn any_null() -> WhereParam { WhereParam::#pascal_name(caustics::FieldOp::JsonNull(caustics::JsonNullValueFilter::AnyNull)) }
                }
            }
            FieldType::OptionJson => {
                quote! {
                    pub fn path(path: Vec<String>) -> WhereParam {
                        WhereParam::#pascal_name(caustics::FieldOp::JsonPath(path))
                    }
                    pub fn json_string_contains(value: String) -> WhereParam {
                        WhereParam::#pascal_name(caustics::FieldOp::JsonStringContains(value))
                    }
                    pub fn json_string_starts_with(value: String) -> WhereParam {
                        WhereParam::#pascal_name(caustics::FieldOp::JsonStringStartsWith(value))
                    }
                    pub fn json_string_ends_with(value: String) -> WhereParam {
                        WhereParam::#pascal_name(caustics::FieldOp::JsonStringEndsWith(value))
                    }
                    pub fn json_array_contains(value: serde_json::Value) -> WhereParam {
                        WhereParam::#pascal_name(caustics::FieldOp::JsonArrayContains(value))
                    }
                    pub fn json_array_starts_with(value: serde_json::Value) -> WhereParam {
                        WhereParam::#pascal_name(caustics::FieldOp::JsonArrayStartsWith(value))
                    }
                    pub fn json_array_ends_with(value: serde_json::Value) -> WhereParam {
                        WhereParam::#pascal_name(caustics::FieldOp::JsonArrayEndsWith(value))
                    }
                    pub fn json_object_contains(key: String) -> WhereParam {
                        WhereParam::#pascal_name(caustics::FieldOp::JsonObjectContains(key))
                    }
                    pub fn db_null() -> WhereParam { WhereParam::#pascal_name(caustics::FieldOp::JsonNull(caustics::JsonNullValueFilter::DbNull)) }
                    pub fn json_null() -> WhereParam { WhereParam::#pascal_name(caustics::FieldOp::JsonNull(caustics::JsonNullValueFilter::JsonNull)) }
                    pub fn any_null() -> WhereParam { WhereParam::#pascal_name(caustics::FieldOp::JsonNull(caustics::JsonNullValueFilter::AnyNull)) }
                }
            }
            _ => quote! {},
        };

        // Atomic operations (only for numeric types)
        let atomic_ops = if !is_unique
            && matches!(
                field_type,
                FieldType::Integer
                    | FieldType::OptionInteger
                    | FieldType::Float
                    | FieldType::OptionFloat
            ) {
            // Extract the inner type for Option<T>
            let inner_ty = crate::common::extract_inner_type_from_option(ty);

            let increment_name = format_ident!("{}Increment", pascal_name);
            let decrement_name = format_ident!("{}Decrement", pascal_name);
            let multiply_name = format_ident!("{}Multiply", pascal_name);
            let divide_name = format_ident!("{}Divide", pascal_name);

            quote! {
                pub fn increment<T: Into<#inner_ty>>(value: T) -> super::SetParam {
                    super::SetParam::#increment_name(value.into())
                }
                pub fn decrement<T: Into<#inner_ty>>(value: T) -> super::SetParam {
                    super::SetParam::#decrement_name(value.into())
                }
                pub fn multiply<T: Into<#inner_ty>>(value: T) -> super::SetParam {
                    super::SetParam::#multiply_name(value.into())
                }
                pub fn divide<T: Into<#inner_ty>>(value: T) -> super::SetParam {
                    super::SetParam::#divide_name(value.into())
                }
            }
        } else {
            quote! {}
        };

        let mut field_mod_items = vec![
            set_fn,
            unique_where_fn,
            order_fn,
            count_fn,
            type_specific_ops,
            field_not_alias,
            string_ops,
            comparison_ops,
            collection_ops,
            null_ops,
            json_ops,
            atomic_ops,
        ];

        // If this is a string field, add a Mode variant and mode function
        if matches!(field_type, FieldType::String | FieldType::OptionString) {
            let mode_variant = format_ident!("{}Mode", pascal_name);
            where_field_variants.push(quote! { #mode_variant(caustics::QueryMode) });
            field_mod_items.push(quote! {
            pub fn mode(mode: caustics::QueryMode) -> WhereParam {
                WhereParam::#mode_variant(mode)
                                            }
                                        });
        }
        field_ops.push(quote! {
                    pub mod #name {
                        use chrono::{NaiveDate, NaiveDateTime, DateTime, FixedOffset};
                        use uuid::Uuid;
                        use std::vec::Vec;
                        use super::*;
                #(#field_mod_items)*
            }
        });
    }
    // Logical operator helpers
    field_ops.push(quote! {
        pub fn and(params: Vec<WhereParam>) -> WhereParam {
            WhereParam::And(params)
        }
    });
    field_ops.push(quote! {
        pub fn or(params: Vec<WhereParam>) -> WhereParam {
            WhereParam::Or(params)
        }
    });
    field_ops.push(quote! {
        pub fn not(params: Vec<WhereParam>) -> WhereParam {
            WhereParam::Not(params)
        }
    });

    // Use unqualified name for logical operator variants
    where_field_variants.push(quote! { And(Vec<WhereParam>) });
    where_field_variants.push(quote! { Or(Vec<WhereParam>) });
    where_field_variants.push(quote! { Not(Vec<WhereParam>) });

    // Add relation condition variant for advanced relation operations (only if there are relations)
    if !relations.is_empty() {
        where_field_variants.push(quote! { RelationCondition(caustics::RelationCondition) });
    }

    // Generate a function that processes all WhereParams together, properly handling QueryMode
    let where_params_to_condition_fn = generate_where_params_to_condition_function(
        fields,
        primary_key_fields,
        relations,
        entity_name,
    );

    let where_match_arms: Vec<proc_macro2::TokenStream> = vec![where_params_to_condition_fn];
    (where_field_variants, where_match_arms, field_ops)
}

/// Generate a function that converts Vec<WhereParam> to Condition, properly handling QueryMode
fn generate_where_params_to_condition_function(
    fields: &[&syn::Field],
    primary_key_fields: &[&syn::Field],
    relations: &[crate::entity::Relation],
    entity_name: &str,
) -> proc_macro2::TokenStream {
    let mut field_handlers = Vec::new();
    let mut mode_handlers = Vec::new();

    for field in fields.iter() {
        let name = field.ident.as_ref().unwrap();
        let pascal_name = format_ident!("{}", name.to_string().to_pascal_case());
        let ty = &field.ty;

        // Comprehensive type detection
        let field_type = detect_field_type(ty);

        // Check if this is a primary key field
        let is_primary_key = primary_key_fields
            .iter()
            .any(|pk_field| pk_field.ident.as_ref().unwrap() == name);

        // Generate field operation handler based on type
        match field_type {
            FieldType::String => {
                field_handlers.push(generate_string_field_handler(
                    &pascal_name,
                    false,
                    is_primary_key,
                ));
                mode_handlers.push(generate_mode_handler(&pascal_name, name));
            }
            FieldType::OptionString => {
                field_handlers.push(generate_string_field_handler(
                    &pascal_name,
                    true,
                    is_primary_key,
                ));
                mode_handlers.push(generate_mode_handler(&pascal_name, name));
            }
            FieldType::Integer => {
                field_handlers.push(generate_numeric_field_handler(
                    &pascal_name,
                    false,
                    is_primary_key,
                    entity_name,
                ));
            }
            FieldType::OptionInteger => {
                field_handlers.push(generate_numeric_field_handler(
                    &pascal_name,
                    true,
                    is_primary_key,
                    entity_name,
                ));
            }
            FieldType::Float => {
                field_handlers.push(generate_numeric_field_handler(
                    &pascal_name,
                    false,
                    is_primary_key,
                    entity_name,
                ));
            }
            FieldType::OptionFloat => {
                field_handlers.push(generate_numeric_field_handler(
                    &pascal_name,
                    true,
                    is_primary_key,
                    entity_name,
                ));
            }
            FieldType::Boolean => {
                field_handlers.push(generate_boolean_field_handler(
                    &pascal_name,
                    false,
                    is_primary_key,
                ));
            }
            FieldType::OptionBoolean => {
                field_handlers.push(generate_boolean_field_handler(
                    &pascal_name,
                    true,
                    is_primary_key,
                ));
            }
            FieldType::DateTime => {
                field_handlers.push(generate_datetime_field_handler(
                    &pascal_name,
                    false,
                    is_primary_key,
                ));
            }
            FieldType::OptionDateTime => {
                field_handlers.push(generate_datetime_field_handler(
                    &pascal_name,
                    true,
                    is_primary_key,
                ));
            }
            FieldType::Uuid => {
                field_handlers.push(generate_uuid_field_handler(
                    &pascal_name,
                    false,
                    is_primary_key,
                    entity_name,
                ));
            }
            FieldType::OptionUuid => {
                field_handlers.push(generate_uuid_field_handler(
                    &pascal_name,
                    true,
                    is_primary_key,
                    entity_name,
                ));
            }
            FieldType::Json => {
                field_handlers.push(generate_json_field_handler(
                    &pascal_name,
                    false,
                    is_primary_key,
                ));
            }
            FieldType::OptionJson => {
                field_handlers.push(generate_json_field_handler(
                    &pascal_name,
                    true,
                    is_primary_key,
                ));
            }
            FieldType::Other => {
                field_handlers.push(generate_generic_field_handler(&pascal_name, is_primary_key));
            }
        }
    }

    // Generate dynamic relation match arms
    let mut relation_match_arms = Vec::new();

    for relation in relations {
        let relation_name = &relation.name;
        let relation_name_str = relation_name.to_snake_case();
        let target = &relation.target;

        // Get the foreign key column identifier
        let foreign_key_column_ident = match &relation.foreign_key_column {
            Some(fk_col) => format_ident!("{}", fk_col.to_pascal_case()),
            None => {
                panic!("No foreign key column specified for relation '{}'.\n\nPlease add 'to' attribute with target column.\n\nExample:\n    #[sea_orm(\n        has_many = \"super::post::Entity\",\n        from = \"Column::UserId\",\n        to = \"super::post::Column::AuthorId\"\n    )]\n    posts: Vec<Post>,", relation.name)
            }
        };

        // Get the foreign key column name as string
        let foreign_key_column_str = if let Some(fk_col) = &relation.foreign_key_column {
            // Convert PascalCase to snake_case for database column name
            fk_col.to_string().to_snake_case()
        } else if let Some(pk_field) = &relation.primary_key_field {
            // Use the relation's primary key field if available
            pk_field.to_string()
        } else {
            // This should be configured in the relation definition
            panic!("No foreign key column or primary key field specified for relation '{}'. Please specify either foreign_key_column or primary_key_field in the relation definition.", relation.name)
        };

        // Get table names from relation metadata
        // Use the resolved target table name from build-time metadata
        let target_table_name_str = relation_name_str.clone();
        let current_table_name_str = relation
            .current_table_name
            .as_ref()
            .unwrap_or_else(|| {
                panic!("Missing current table name for relation '{}'. This indicates a bug in relation extraction.\n\nPlease ensure the relation is properly configured with all required attributes.", relation.name)
            })
            .to_string();

        // Generate match arm for this relation
        let relation_name_lit = syn::LitStr::new(&relation_name_str, proc_macro2::Span::call_site());
        let relation_match_arm = quote! {
            #relation_name_lit => {
                match relation_condition.operation {
                    caustics::FieldOp::Some(()) => {
                        // Phase 3: Use SeaORM query builder instead of raw SQL
                        let subquery = #target::Entity::find()
                            .select_only()
                            .column(#target::Column::#foreign_key_column_ident)
                            .filter(sea_query::Expr::cust_with_values(
                                &format!("\"{}\".\"{}\" = \"{}\".\"id\"", #target_table_name_str, #foreign_key_column_str, #current_table_name_str),
                                Vec::<sea_orm::Value>::new()
                            ));

                        // Apply relation condition filters
                        let mut filtered_subquery = subquery;
                        for filter in &relation_condition.filters {
                            // Convert Filter to SeaORM condition
                            let condition = convert_filter_to_condition::<#target::Entity>(filter, #target_table_name_str);
                            filtered_subquery = filtered_subquery.filter(condition);
                        }

                        Condition::all().add(sea_query::Expr::exists(filtered_subquery.into_query()))
                    },
                    caustics::FieldOp::Every(()) => {
                        // Phase 3: Use SeaORM query builder instead of raw SQL
                        // For 'every', we need: NOT EXISTS (SELECT 1 FROM target WHERE fk = current.id AND NOT (filter))
                        let subquery = #target::Entity::find()
                            .select_only()
                            .column(#target::Column::#foreign_key_column_ident)
                            .filter(sea_query::Expr::cust_with_values(
                                &format!("\"{}\".\"{}\" = \"{}\".\"id\"", #target_table_name_str, #foreign_key_column_str, #current_table_name_str),
                                Vec::<sea_orm::Value>::new()
                            ));

                        // Apply filters for 'every' operation
                        // We need to find records where there are NO related records that DON'T match the filter
                        // This means: NOT EXISTS (SELECT 1 FROM target WHERE fk = current.id AND NOT (filter))
                        let mut filtered_subquery = subquery;
                        for filter in &relation_condition.filters {
                            let condition = convert_filter_to_condition::<#target::Entity>(filter, #target_table_name_str);
                            // For 'every', we want records where there are NO related records that DON'T match
                            // So we add the condition as-is, then negate the entire EXISTS
                            filtered_subquery = filtered_subquery.filter(condition.not());
                        }

                        Condition::all().add(sea_query::Expr::exists(filtered_subquery.into_query()).not())
                    },
                    caustics::FieldOp::None(()) => {
                        // Phase 3: Use SeaORM query builder instead of raw SQL
                        let subquery = #target::Entity::find()
                            .select_only()
                            .column(#target::Column::#foreign_key_column_ident)
                            .filter(sea_query::Expr::cust_with_values(
                                &format!("\"{}\".\"{}\" = \"{}\".\"id\"", #target_table_name_str, #foreign_key_column_str, #current_table_name_str),
                                Vec::<sea_orm::Value>::new()
                            ));

                        // Apply filters for 'none' operation
                        let mut filtered_subquery = subquery;
                        for filter in &relation_condition.filters {
                            let condition = convert_filter_to_condition::<#target::Entity>(filter, #target_table_name_str);
                            filtered_subquery = filtered_subquery.filter(condition);
                        }

                        Condition::all().add(sea_query::Expr::exists(filtered_subquery.into_query()).not())
                    },
                    // Catch-all for unsupported relation operations: no-op condition
                    _ => Condition::all(),
                }
            }
        };

        relation_match_arms.push(relation_match_arm);
    }

    // Generate the RelationCondition match arm only if there are relations
    let relation_condition_arm = if !relations.is_empty() {
        let arms = &relation_match_arms;
        quote! {
            WhereParam::RelationCondition(relation_condition) => {
                match relation_condition.relation_name {
                    #(
                        #arms
                    )*
                    _ => panic!("Unknown relation: {}", relation_condition.relation_name),
                }
            },
        }
    } else {
        quote! {}
    };

    quote! {
        /// Convert CausticsKey to the appropriate type for SeaORM operations
        fn convert_caustics_key_to_type<T>(key: caustics::CausticsKey) -> T
        where
            T: From<String> + From<i32> + From<uuid::Uuid>,
        {
            match key {
                caustics::CausticsKey::String(s) => T::from(s),
                caustics::CausticsKey::I32(i) => T::from(i),
                caustics::CausticsKey::Uuid(u) => T::from(u),
                _ => panic!("Unsupported CausticsKey variant for conversion"),
            }
        }

        /// Convert a Filter to a SeaORM condition for the target entity
        fn convert_filter_to_condition<T: EntityTrait>(filter: &caustics::Filter, table_name: &str) -> sea_query::Condition {
            use sea_orm::{EntityTrait, ColumnTrait};
            use sea_query::Condition;

            // Type-safe field handling - direct FieldOp matching
            // Return no-op Condition for unsupported operations
            match &filter.operation {
                caustics::FieldOp::Equals(value) => {
                    Condition::all().add(sea_query::Expr::cust_with_values(
                        &format!("\"{}\".{} = ?", table_name, filter.field),
                        [value.clone()]
                    ))
                },
                caustics::FieldOp::NotEquals(value) => {
                    Condition::all().add(sea_query::Expr::cust_with_values(
                        &format!("\"{}\".{} != ?", table_name, filter.field),
                        [value.clone()]
                    ))
                },
                caustics::FieldOp::Gt(value) => {
                    Condition::all().add(sea_query::Expr::cust_with_values(
                        &format!("\"{}\".{} > ?", table_name, filter.field),
                        [value.clone()]
                    ))
                },
                caustics::FieldOp::Lt(value) => {
                    Condition::all().add(sea_query::Expr::cust_with_values(
                        &format!("\"{}\".{} < ?", table_name, filter.field),
                        [value.clone()]
                    ))
                },
                caustics::FieldOp::Gte(value) => {
                    Condition::all().add(sea_query::Expr::cust_with_values(
                        &format!("\"{}\".{} >= ?", table_name, filter.field),
                        [value.clone()]
                    ))
                },
                caustics::FieldOp::Lte(value) => {
                    Condition::all().add(sea_query::Expr::cust_with_values(
                        &format!("\"{}\".{} <= ?", table_name, filter.field),
                        [value.clone()]
                    ))
                },
                caustics::FieldOp::Contains(value) => {
                    Condition::all().add(sea_query::Expr::cust_with_values(
                        &format!("\"{}\".{} LIKE ?", table_name, filter.field),
                        [sea_orm::Value::from(format!("%{}%", value))]
                    ))
                },
                caustics::FieldOp::StartsWith(value) => {
                    Condition::all().add(sea_query::Expr::cust_with_values(
                        &format!("\"{}\".{} LIKE ?", table_name, filter.field),
                        [sea_orm::Value::from(format!("{}%", value))]
                    ))
                },
                caustics::FieldOp::EndsWith(value) => {
                    Condition::all().add(sea_query::Expr::cust_with_values(
                        &format!("\"{}\".{} LIKE ?", table_name, filter.field),
                        [sea_orm::Value::from(format!("%{}", value))]
                    ))
                },
                caustics::FieldOp::InVec(values) => {
                    Condition::all().add(sea_query::Expr::cust_with_values(
                        &format!("\"{}\".{} IN ({})", table_name, filter.field,
                            values.iter().map(|_| "?").collect::<Vec<_>>().join(",")),
                        values.iter().map(|v| v.clone()).collect::<Vec<_>>()
                    ))
                },
                caustics::FieldOp::NotInVec(values) => {
                    Condition::all().add(sea_query::Expr::cust_with_values(
                        &format!("\"{}\".{} NOT IN ({})", table_name, filter.field,
                            values.iter().map(|_| "?").collect::<Vec<_>>().join(",")),
                        values.iter().map(|v| v.clone()).collect::<Vec<_>>()
                    ))
                },
                caustics::FieldOp::IsNull => {
                    Condition::all().add(sea_query::Expr::cust_with_values(
                        &format!("\"{}\".{} IS NULL", table_name, filter.field),
                        Vec::<sea_orm::Value>::new()
                    ))
                },
                caustics::FieldOp::IsNotNull => {
                    Condition::all().add(sea_query::Expr::cust_with_values(
                        &format!("\"{}\".{} IS NOT NULL", table_name, filter.field),
                        Vec::<sea_orm::Value>::new()
                    ))
                },
                // JSON operations
                caustics::FieldOp::JsonPath(path) => {
                    let json_path = path.join(".");
                    Condition::all().add(sea_query::Expr::cust_with_values(
                        &format!("json_extract(\"{}\".{}, ?) IS NOT NULL", table_name, filter.field),
                        [format!("$.{}", json_path)]
                    ))
                },
                caustics::FieldOp::JsonStringContains(value) => {
                    Condition::all().add(sea_query::Expr::cust_with_values(
                        &format!("json_extract(\"{}\".{}, '$') LIKE ?", table_name, filter.field),
                        [format!("%{}%", value)]
                    ))
                },
                caustics::FieldOp::JsonStringStartsWith(value) => {
                    Condition::all().add(sea_query::Expr::cust_with_values(
                        &format!("json_extract(\"{}\".{}, '$') LIKE ?", table_name, filter.field),
                        [format!("{}%", value)]
                    ))
                },
                caustics::FieldOp::JsonStringEndsWith(value) => {
                    Condition::all().add(sea_query::Expr::cust_with_values(
                        &format!("json_extract(\"{}\".{}, '$') LIKE ?", table_name, filter.field),
                        [format!("%{}", value)]
                    ))
                },
                caustics::FieldOp::JsonArrayContains(value) => {
                    Condition::all().add(sea_query::Expr::cust_with_values(
                        &format!("EXISTS (SELECT 1 FROM json_each(\"{}\".{}) WHERE value = ?)", table_name, filter.field),
                        [value.to_string()]
                    ))
                },
                caustics::FieldOp::JsonArrayStartsWith(value) => {
                    Condition::all().add(sea_query::Expr::cust_with_values(
                        &format!("json_extract(\"{}\".{}, '$[0]') = ?", table_name, filter.field),
                        [value.to_string()]
                    ))
                },
                caustics::FieldOp::JsonArrayEndsWith(value) => {
                    Condition::all().add(sea_query::Expr::cust_with_values(
                        &format!("json_extract(\"{}\".{}, '$[#-1]') = ?", table_name, filter.field),
                        [value.to_string()]
                    ))
                },
                caustics::FieldOp::JsonObjectContains(key) => {
                    Condition::all().add(sea_query::Expr::cust_with_values(
                        &format!("json_extract(\"{}\".{}, ?) IS NOT NULL", table_name, filter.field),
                        [format!("$.{}", key)]
                    ))
                },
                caustics::FieldOp::JsonNull(flag) => {
                    match flag {
                        caustics::JsonNullValueFilter::DbNull => Condition::all().add(sea_query::Expr::cust_with_values(
                            &format!("\"{}\".{} IS NULL", table_name, filter.field), Vec::<sea_orm::Value>::new())),
                        caustics::JsonNullValueFilter::JsonNull => Condition::all().add(sea_query::Expr::cust_with_values(
                            &format!("json_type(\"{}\".{}, '$') = 'null'", table_name, filter.field), Vec::<sea_orm::Value>::new())),
                        caustics::JsonNullValueFilter::AnyNull => Condition::all().add(sea_query::Expr::cust_with_values(
                            &format!("(\"{}\".{} IS NULL OR json_type(\"{}\".{}, '$') = 'null')", table_name, filter.field, table_name, filter.field), Vec::<sea_orm::Value>::new())),
                    }
                },
                // Relation operations (should not be used in field mappings) -> no-op
                caustics::FieldOp::Some(_) | caustics::FieldOp::Every(_) | caustics::FieldOp::None(_) => Condition::all(),
                _ => Condition::all(),
            }
        }

        /// Convert a vector of WhereParams to a SeaORM Condition, properly handling QueryMode
        pub fn where_params_to_condition(params: Vec<WhereParam>, database_backend: sea_orm::DatabaseBackend) -> sea_query::Condition {
            use std::collections::HashMap;
            use sea_orm::{EntityTrait, ColumnTrait, QuerySelect, QueryTrait};
            let database_backend = database_backend; // ensure variable in scope for nested closures
            use sea_query::Condition;

            let mut final_condition = Condition::all();
            let mut query_modes: HashMap<String, caustics::QueryMode> = HashMap::new();

            // Process params in two passes: first collect modes, then apply conditions
            let mut deferred_params = Vec::new();

            for param in params {
                match param {
                    #(#mode_handlers)*
                    other => deferred_params.push(other),
                }
            }

            // Second pass: apply conditions with collected query modes
            for param in deferred_params {
                let condition = match param {
                    #(#field_handlers)*
                    WhereParam::And(params) => {
                        let mut cond = Condition::all();
                        for p in params {
                            cond = cond.add(where_params_to_condition(vec![p], database_backend));
                        }
                        cond
                    },
                    WhereParam::Or(params) => {
                        let mut cond = Condition::any();
                        for p in params {
                            cond = cond.add(where_params_to_condition(vec![p], database_backend));
                        }
                        cond
                    },
                    WhereParam::Not(params) => {
                        let mut cond = Condition::all();
                        for p in params {
                            cond = cond.add(where_params_to_condition(vec![p], database_backend));
                        }
                        cond.not()
                    },
                    #relation_condition_arm
                    _ => panic!("Unhandled WhereParam variant"),
                };
                final_condition = final_condition.add(condition);
            }

            final_condition
        }
    }
}

#[derive(Debug, Clone)]
pub enum FieldType {
    String,
    OptionString,
    Integer,
    OptionInteger,
    Float,
    OptionFloat,
    Boolean,
    OptionBoolean,
    DateTime,
    OptionDateTime,
    Uuid,
    OptionUuid,
    Json,
    OptionJson,
    Other,
}

/// Detect the field type from the syn::Type
pub fn detect_field_type(ty: &syn::Type) -> FieldType {
    match ty {
        syn::Type::Path(path) => {
            if let Some(segment) = path.path.segments.last() {
                match segment.ident.to_string().as_str() {
                    "String" => FieldType::String,
                    "bool" => FieldType::Boolean,
                    "i8" | "i16" | "i32" | "i64" | "u8" | "u16" | "u32" | "u64" | "isize"
                    | "usize" => FieldType::Integer,
                    "f32" | "f64" => FieldType::Float,
                    "Uuid" => FieldType::Uuid,
                    "DateTime" => FieldType::DateTime,
                    "NaiveDateTime" => FieldType::DateTime,
                    "NaiveDate" => FieldType::DateTime,
                    "Value" => FieldType::Json, // serde_json::Value
                    "Option" => {
                        // Handle Option<T> types
                        if let syn::PathArguments::AngleBracketed(args) = &segment.arguments {
                            if let Some(syn::GenericArgument::Type(inner_ty)) = args.args.first() {
                                match detect_field_type(inner_ty) {
                                    FieldType::String => FieldType::OptionString,
                                    FieldType::Integer => FieldType::OptionInteger,
                                    FieldType::Float => FieldType::OptionFloat,
                                    FieldType::Boolean => FieldType::OptionBoolean,
                                    FieldType::DateTime => FieldType::OptionDateTime,
                                    FieldType::Uuid => FieldType::OptionUuid,
                                    FieldType::Json => FieldType::OptionJson,
                                    _ => FieldType::Other,
                                }
                            } else {
                                FieldType::Other
                            }
                        } else {
                            FieldType::Other
                        }
                    }
                    _ => FieldType::Other,
                }
            } else {
                FieldType::Other
            }
        }
        _ => FieldType::Other,
    }
}

/// Generate database-agnostic string field handler
fn generate_string_field_handler(
    pascal_name: &proc_macro2::Ident,
    is_nullable: bool,
    is_primary_key: bool,
) -> proc_macro2::TokenStream {
    let field_name_str = pascal_name.to_string().to_lowercase();

    if is_nullable {
        quote! {
            WhereParam::#pascal_name(op) => {
                let query_mode = query_modes.get(#field_name_str).copied().unwrap_or(caustics::QueryMode::Default);
                match op {
                    caustics::FieldOp::Equals(v) => {
                        if query_mode == caustics::QueryMode::Insensitive {
                            // Database-agnostic case insensitive equality
                            match database_backend {
                                sea_orm::DatabaseBackend::Postgres => {
                                    Condition::all().add(
                                        sea_query::Expr::cust_with_values(
                                            &format!("UPPER({}) = UPPER(?)", <Entity as EntityTrait>::Column::#pascal_name.to_string()),
                                            [v.clone()]
                                        )
                                    )
                                },
                                _ => {
                                    // MySQL, MariaDB, SQLite - use UPPER() for consistency
                                    Condition::all().add(
                                        sea_query::Expr::cust_with_values(
                                            &format!("UPPER({}) = UPPER(?)", <Entity as EntityTrait>::Column::#pascal_name.to_string()),
                                            [v.clone()]
                                        )
                                    )
                                }
                            }
                        } else {
                            Condition::all().add(<Entity as EntityTrait>::Column::#pascal_name.eq(v))
                        }
                    },
                    caustics::FieldOp::NotEquals(v) => {
                        if query_mode == caustics::QueryMode::Insensitive {
                            match database_backend {
                                sea_orm::DatabaseBackend::Postgres => {
                                Condition::all().add(
                                    sea_query::Expr::cust_with_values(
                                        &format!("UPPER({}) != UPPER(?)", <Entity as EntityTrait>::Column::#pascal_name.to_string()),
                                        [v.clone()]
                                    )
                                )
                                },
                                _ => {
                                    Condition::all().add(
                                        sea_query::Expr::cust_with_values(
                                            &format!("UPPER({}) != UPPER(?)", <Entity as EntityTrait>::Column::#pascal_name.to_string()),
                                            [v.clone()]
                                        )
                                    )
                                }
                            }
                        } else {
                            Condition::all().add(<Entity as EntityTrait>::Column::#pascal_name.ne(v))
                        }
                    },
                    caustics::FieldOp::Contains(s) => {
                        if query_mode == caustics::QueryMode::Insensitive {
                            match database_backend {
                                sea_orm::DatabaseBackend::Postgres => {
                                    // PostgreSQL: column ILIKE '%value%'
                                Condition::all().add(
                                    sea_query::Expr::cust_with_values(
                                            &format!("{} ILIKE ?", <Entity as EntityTrait>::Column::#pascal_name.to_string()),
                                            [format!("%{}%", s)]
                                        )
                                    )
                                },
                                _ => {
                                    // MySQL, MariaDB, SQLite: UPPER(column) LIKE UPPER('%value%')
                                    Condition::all().add(
                                            sea_query::Expr::cust_with_values(
                                            &format!("UPPER({}) LIKE UPPER(?)", <Entity as EntityTrait>::Column::#pascal_name.to_string()),
                                            [format!("%{}%", s)]
                                        )
                                    )
                                }
                            }
                        } else {
                            // Use SeaORM's built-in contains method
                            Condition::all().add(<Entity as EntityTrait>::Column::#pascal_name.contains(s))
                        }
                    },
                    caustics::FieldOp::StartsWith(s) => {
                        if query_mode == caustics::QueryMode::Insensitive {
                            match database_backend {
                                sea_orm::DatabaseBackend::Postgres => {
                                    Condition::all().add(
                                        sea_query::Expr::cust_with_values(
                                            &format!("{} ILIKE ?", <Entity as EntityTrait>::Column::#pascal_name.to_string()),
                                            [format!("{}%", s)]
                                        )
                                    )
                                },
                                _ => {
                                    Condition::all().add(
                                        sea_query::Expr::cust_with_values(
                                            &format!("UPPER({}) LIKE UPPER(?)", <Entity as EntityTrait>::Column::#pascal_name.to_string()),
                                            [format!("{}%", s)]
                                        )
                                    )
                                }
                            }
                        } else {
                            // Use SeaORM's built-in starts_with method
                            Condition::all().add(<Entity as EntityTrait>::Column::#pascal_name.starts_with(s))
                        }
                    },
                    caustics::FieldOp::EndsWith(s) => {
                        if query_mode == caustics::QueryMode::Insensitive {
                            // Database-agnostic case insensitive ends with using UPPER()
                                Condition::all().add(
                                    sea_query::Expr::cust_with_values(
                                        &format!("UPPER({}) LIKE UPPER(?)", <Entity as EntityTrait>::Column::#pascal_name.to_string()),
                                        [format!("%{}", s)]
                                    )
                                )
                        } else {
                            // Use SeaORM's built-in ends_with method
                            Condition::all().add(<Entity as EntityTrait>::Column::#pascal_name.ends_with(s))
                        }
                    },
                    caustics::FieldOp::Gt(v) => {
                        Condition::all().add(<Entity as EntityTrait>::Column::#pascal_name.gt(v))
                    },
                    caustics::FieldOp::Lt(v) => {
                        Condition::all().add(<Entity as EntityTrait>::Column::#pascal_name.lt(v))
                    },
                    caustics::FieldOp::Gte(v) => {
                        Condition::all().add(<Entity as EntityTrait>::Column::#pascal_name.gte(v))
                    },
                    caustics::FieldOp::Lte(v) => {
                        Condition::all().add(<Entity as EntityTrait>::Column::#pascal_name.lte(v))
                    },
                    caustics::FieldOp::InVec(v) => Condition::all().add(<Entity as EntityTrait>::Column::#pascal_name.is_in(v.to_vec())),
                    caustics::FieldOp::NotInVec(v) => Condition::all().add(<Entity as EntityTrait>::Column::#pascal_name.is_not_in(v.to_vec())),
                    caustics::FieldOp::IsNull => Condition::all().add(<Entity as EntityTrait>::Column::#pascal_name.is_null()),
                    caustics::FieldOp::IsNotNull => Condition::all().add(<Entity as EntityTrait>::Column::#pascal_name.is_not_null()),
                    // Catch-all for unsupported operations
                    _ => panic!("Unsupported FieldOp operation for this field type"),
                }
            }
        }
    } else {
        quote! {
            WhereParam::#pascal_name(op) => {
                let query_mode = query_modes.get(#field_name_str).copied().unwrap_or(caustics::QueryMode::Default);
                match op {
                    caustics::FieldOp::Equals(val) => {
                        if query_mode == caustics::QueryMode::Insensitive {
                            // Database-agnostic case insensitive equality using UPPER()
                                Condition::all().add(
                                    sea_query::Expr::cust_with_values(
                                        &format!("UPPER({}) = UPPER(?)", <Entity as EntityTrait>::Column::#pascal_name.to_string()),
                                        [val]
                                    )
                                )
                            } else {
                            Condition::all().add(<Entity as EntityTrait>::Column::#pascal_name.eq(val))
                        }
                    },
                    caustics::FieldOp::NotEquals(val) => {
                        if query_mode == caustics::QueryMode::Insensitive {
                            // Database-agnostic case insensitive inequality using UPPER()
                                Condition::all().add(
                                    sea_query::Expr::cust_with_values(
                                        &format!("UPPER({}) != UPPER(?)", <Entity as EntityTrait>::Column::#pascal_name.to_string()),
                                        [val]
                                    )
                                )
                            } else {
                            Condition::all().add(<Entity as EntityTrait>::Column::#pascal_name.ne(val))
                        }
                    },
                    caustics::FieldOp::Contains(s) => {
                        if query_mode == caustics::QueryMode::Insensitive {
                            // Database-agnostic case insensitive contains using UPPER()
                                Condition::all().add(
                                    sea_query::Expr::cust_with_values(
                                        &format!("UPPER({}) LIKE UPPER(?)", <Entity as EntityTrait>::Column::#pascal_name.to_string()),
                                        [format!("%{}%", s)]
                                    )
                                )
                        } else {
                            // Use SeaORM's built-in contains method
                            Condition::all().add(<Entity as EntityTrait>::Column::#pascal_name.contains(s))
                        }
                    },
                    caustics::FieldOp::StartsWith(s) => {
                        if query_mode == caustics::QueryMode::Insensitive {
                            // Database-agnostic case insensitive starts with using UPPER()
                                Condition::all().add(
                                    sea_query::Expr::cust_with_values(
                                        &format!("UPPER({}) LIKE UPPER(?)", <Entity as EntityTrait>::Column::#pascal_name.to_string()),
                                        [format!("{}%", s)]
                                    )
                                )
                        } else {
                            // Use SeaORM's built-in starts_with method
                            Condition::all().add(<Entity as EntityTrait>::Column::#pascal_name.starts_with(s))
                        }
                    },
                    caustics::FieldOp::EndsWith(s) => {
                        if query_mode == caustics::QueryMode::Insensitive {
                            // Database-agnostic case insensitive ends with using UPPER()
                                Condition::all().add(
                                    sea_query::Expr::cust_with_values(
                                        &format!("UPPER({}) LIKE UPPER(?)", <Entity as EntityTrait>::Column::#pascal_name.to_string()),
                                        [format!("%{}", s)]
                                    )
                                )
                        } else {
                            // Use SeaORM's built-in ends_with method
                            Condition::all().add(<Entity as EntityTrait>::Column::#pascal_name.ends_with(s))
                        }
                    },
                    caustics::FieldOp::Gt(val) => Condition::all().add(<Entity as EntityTrait>::Column::#pascal_name.gt(val)),
                    caustics::FieldOp::Lt(val) => Condition::all().add(<Entity as EntityTrait>::Column::#pascal_name.lt(val)),
                    caustics::FieldOp::Gte(val) => Condition::all().add(<Entity as EntityTrait>::Column::#pascal_name.gte(val)),
                    caustics::FieldOp::Lte(val) => Condition::all().add(<Entity as EntityTrait>::Column::#pascal_name.lte(val)),
                    caustics::FieldOp::InVec(vs) => Condition::all().add(<Entity as EntityTrait>::Column::#pascal_name.is_in(vs.clone())),
                    caustics::FieldOp::NotInVec(vs) => Condition::all().add(<Entity as EntityTrait>::Column::#pascal_name.is_not_in(vs.clone())),
                    // Catch-all for unsupported operations
                    _ => panic!("Unsupported FieldOp operation for this field type"),
                }
            }
        }
    }
}

/// Generate QueryMode handler for string fields
fn generate_mode_handler(
    pascal_name: &proc_macro2::Ident,
    name: &syn::Ident,
) -> proc_macro2::TokenStream {
    let mode_variant = format_ident!("{}Mode", pascal_name);
    quote! {
        WhereParam::#mode_variant(mode) => {
            query_modes.insert(stringify!(#name).to_string(), mode);
            continue; // Skip adding condition, this just sets the mode
        }
    }
}

/// Generate numeric field handler (integers and floats)
fn generate_numeric_field_handler(
    pascal_name: &proc_macro2::Ident,
    is_nullable: bool,
    is_primary_key: bool,
    entity_name: &str,
) -> proc_macro2::TokenStream {
    if is_primary_key {
        // For primary key fields, FieldOp is generic over CausticsKey, so convert using registry
        let field_name_snake = pascal_name.to_string().to_snake_case();
        quote! {
            WhereParam::#pascal_name(op) => match op {
                caustics::FieldOp::Equals(v) => {
                    Condition::all().add(<Entity as EntityTrait>::Column::#pascal_name.eq(v))
                },
                caustics::FieldOp::NotEquals(v) => {
                    Condition::all().add(<Entity as EntityTrait>::Column::#pascal_name.ne(v))
                },
                caustics::FieldOp::InVec(vs) => {
                    Condition::all().add(<Entity as EntityTrait>::Column::#pascal_name.is_in(vs.clone()))
                },
                caustics::FieldOp::NotInVec(vs) => {
                    Condition::all().add(<Entity as EntityTrait>::Column::#pascal_name.is_not_in(vs.clone()))
                },
                // Catch-all for unsupported operations
                _ => panic!("Unsupported FieldOp operation for this field type"),
            }
        }
    } else if is_nullable {
        quote! {
            WhereParam::#pascal_name(op) => match op {
                caustics::FieldOp::Equals(v) => {
                    Condition::all().add(<Entity as EntityTrait>::Column::#pascal_name.eq(v))
                },
                caustics::FieldOp::NotEquals(v) => {
                    Condition::all().add(<Entity as EntityTrait>::Column::#pascal_name.ne(v))
                },
                caustics::FieldOp::Gt(v) => {
                    Condition::all().add(<Entity as EntityTrait>::Column::#pascal_name.gt(v))
                },
                caustics::FieldOp::Lt(v) => {
                    Condition::all().add(<Entity as EntityTrait>::Column::#pascal_name.lt(v))
                },
                caustics::FieldOp::Gte(v) => {
                    Condition::all().add(<Entity as EntityTrait>::Column::#pascal_name.gte(v))
                },
                caustics::FieldOp::Lte(v) => {
                    Condition::all().add(<Entity as EntityTrait>::Column::#pascal_name.lte(v))
                },
                caustics::FieldOp::InVec(vs) => {
                    Condition::all().add(<Entity as EntityTrait>::Column::#pascal_name.is_in(vs.clone()))
                },
                caustics::FieldOp::NotInVec(vs) => {
                    Condition::all().add(<Entity as EntityTrait>::Column::#pascal_name.is_not_in(vs.clone()))
                },
                caustics::FieldOp::IsNull => {
                    Condition::all().add(<Entity as EntityTrait>::Column::#pascal_name.is_null())
                },
                caustics::FieldOp::IsNotNull => {
                    Condition::all().add(<Entity as EntityTrait>::Column::#pascal_name.is_not_null())
                },
                // Catch-all for unsupported operations
                _ => panic!("Unsupported FieldOp operation for this field type"),
            }
        }
    } else {
        quote! {
            WhereParam::#pascal_name(op) => match op {
                caustics::FieldOp::Equals(v) => Condition::all().add(<Entity as EntityTrait>::Column::#pascal_name.eq(v)),
                caustics::FieldOp::NotEquals(v) => Condition::all().add(<Entity as EntityTrait>::Column::#pascal_name.ne(v)),
                caustics::FieldOp::Gt(v) => Condition::all().add(<Entity as EntityTrait>::Column::#pascal_name.gt(v)),
                caustics::FieldOp::Lt(v) => Condition::all().add(<Entity as EntityTrait>::Column::#pascal_name.lt(v)),
                caustics::FieldOp::Gte(v) => Condition::all().add(<Entity as EntityTrait>::Column::#pascal_name.gte(v)),
                caustics::FieldOp::Lte(v) => Condition::all().add(<Entity as EntityTrait>::Column::#pascal_name.lte(v)),
                caustics::FieldOp::InVec(vs) => Condition::all().add(<Entity as EntityTrait>::Column::#pascal_name.is_in(vs)),
                caustics::FieldOp::NotInVec(vs) => Condition::all().add(<Entity as EntityTrait>::Column::#pascal_name.is_not_in(vs)),
                // Catch-all for unsupported operations
                _ => panic!("Unsupported FieldOp operation for this field type"),
            }
        }
    }
}

/// Generate boolean field handler
fn generate_boolean_field_handler(
    pascal_name: &proc_macro2::Ident,
    is_nullable: bool,
    is_primary_key: bool,
) -> proc_macro2::TokenStream {
    if is_nullable {
        quote! {
            WhereParam::#pascal_name(op) => match op {
                caustics::FieldOp::Equals(v) => {
                    Condition::all().add(<Entity as EntityTrait>::Column::#pascal_name.eq(v))
                },
                caustics::FieldOp::NotEquals(v) => {
                    Condition::all().add(<Entity as EntityTrait>::Column::#pascal_name.ne(v))
                },
                caustics::FieldOp::InVec(vs) => Condition::all().add(<Entity as EntityTrait>::Column::#pascal_name.is_in(vs)),
                caustics::FieldOp::NotInVec(vs) => Condition::all().add(<Entity as EntityTrait>::Column::#pascal_name.is_not_in(vs)),
                caustics::FieldOp::IsNull => Condition::all().add(<Entity as EntityTrait>::Column::#pascal_name.is_null()),
                caustics::FieldOp::IsNotNull => Condition::all().add(<Entity as EntityTrait>::Column::#pascal_name.is_not_null()),
                // Catch-all for unsupported operations
                _ => panic!("Unsupported FieldOp operation for this field type"),
            }
        }
    } else {
        quote! {
            WhereParam::#pascal_name(op) => match op {
                caustics::FieldOp::Equals(v) => Condition::all().add(<Entity as EntityTrait>::Column::#pascal_name.eq(v)),
                caustics::FieldOp::NotEquals(v) => Condition::all().add(<Entity as EntityTrait>::Column::#pascal_name.ne(v)),
                caustics::FieldOp::InVec(vs) => Condition::all().add(<Entity as EntityTrait>::Column::#pascal_name.is_in(vs)),
                caustics::FieldOp::NotInVec(vs) => Condition::all().add(<Entity as EntityTrait>::Column::#pascal_name.is_not_in(vs)),
                // Catch-all for unsupported operations
                _ => panic!("Unsupported FieldOp operation for this field type"),
            }
        }
    }
}

/// Generate DateTime field handler
fn generate_datetime_field_handler(
    pascal_name: &proc_macro2::Ident,
    is_nullable: bool,
    is_primary_key: bool,
) -> proc_macro2::TokenStream {
    if is_nullable {
        quote! {
            WhereParam::#pascal_name(op) => match op {
                caustics::FieldOp::Equals(v) => {
                    Condition::all().add(<Entity as EntityTrait>::Column::#pascal_name.eq(v))
                },
                caustics::FieldOp::NotEquals(v) => {
                    Condition::all().add(<Entity as EntityTrait>::Column::#pascal_name.ne(v))
                },
                caustics::FieldOp::Gt(v) => {
                    Condition::all().add(<Entity as EntityTrait>::Column::#pascal_name.gt(v))
                },
                caustics::FieldOp::Lt(v) => {
                    Condition::all().add(<Entity as EntityTrait>::Column::#pascal_name.lt(v))
                },
                caustics::FieldOp::Gte(v) => {
                    Condition::all().add(<Entity as EntityTrait>::Column::#pascal_name.gte(v))
                },
                caustics::FieldOp::Lte(v) => {
                    Condition::all().add(<Entity as EntityTrait>::Column::#pascal_name.lte(v))
                },
                caustics::FieldOp::InVec(vs) => Condition::all().add(<Entity as EntityTrait>::Column::#pascal_name.is_in(vs)),
                caustics::FieldOp::NotInVec(vs) => Condition::all().add(<Entity as EntityTrait>::Column::#pascal_name.is_not_in(vs)),
                caustics::FieldOp::IsNull => Condition::all().add(<Entity as EntityTrait>::Column::#pascal_name.is_null()),
                caustics::FieldOp::IsNotNull => Condition::all().add(<Entity as EntityTrait>::Column::#pascal_name.is_not_null()),
                // Catch-all for unsupported operations
                _ => panic!("Unsupported FieldOp operation for this field type"),
            }
        }
    } else {
        quote! {
            WhereParam::#pascal_name(op) => match op {
                caustics::FieldOp::Equals(v) => Condition::all().add(<Entity as EntityTrait>::Column::#pascal_name.eq(v)),
                caustics::FieldOp::NotEquals(v) => Condition::all().add(<Entity as EntityTrait>::Column::#pascal_name.ne(v)),
                caustics::FieldOp::Gt(v) => Condition::all().add(<Entity as EntityTrait>::Column::#pascal_name.gt(v)),
                caustics::FieldOp::Lt(v) => Condition::all().add(<Entity as EntityTrait>::Column::#pascal_name.lt(v)),
                caustics::FieldOp::Gte(v) => Condition::all().add(<Entity as EntityTrait>::Column::#pascal_name.gte(v)),
                caustics::FieldOp::Lte(v) => Condition::all().add(<Entity as EntityTrait>::Column::#pascal_name.lte(v)),
                caustics::FieldOp::InVec(vs) => Condition::all().add(<Entity as EntityTrait>::Column::#pascal_name.is_in(vs)),
                caustics::FieldOp::NotInVec(vs) => Condition::all().add(<Entity as EntityTrait>::Column::#pascal_name.is_not_in(vs)),
                // Catch-all for unsupported operations
                _ => panic!("Unsupported FieldOp operation for this field type"),
            }
        }
    }
}

/// Generate UUID field handler
fn generate_uuid_field_handler(
    pascal_name: &proc_macro2::Ident,
    is_nullable: bool,
    is_primary_key: bool,
    entity_name: &str,
) -> proc_macro2::TokenStream {
    if is_primary_key {
        // For primary key fields, FieldOp is generic over CausticsKey, so convert using registry
        let field_name_snake = pascal_name.to_string().to_snake_case();
        quote! {
            WhereParam::#pascal_name(op) => match op {
                caustics::FieldOp::Equals(v) => {
                    Condition::all().add(<Entity as EntityTrait>::Column::#pascal_name.eq(v))
                },
                caustics::FieldOp::NotEquals(v) => {
                    Condition::all().add(<Entity as EntityTrait>::Column::#pascal_name.ne(v))
                },
                caustics::FieldOp::InVec(vs) => {
                    Condition::all().add(<Entity as EntityTrait>::Column::#pascal_name.is_in(vs.clone()))
                },
                caustics::FieldOp::NotInVec(vs) => {
                    Condition::all().add(<Entity as EntityTrait>::Column::#pascal_name.is_not_in(vs.clone()))
                },
                // Catch-all for unsupported operations
                _ => panic!("Unsupported FieldOp operation for this field type"),
            }
        }
    } else if is_nullable {
        quote! {
            WhereParam::#pascal_name(op) => match op {
                caustics::FieldOp::Equals(v) => {
                    Condition::all().add(<Entity as EntityTrait>::Column::#pascal_name.eq(v))
                },
                caustics::FieldOp::NotEquals(v) => {
                    Condition::all().add(<Entity as EntityTrait>::Column::#pascal_name.ne(v))
                },
                caustics::FieldOp::InVec(vs) => Condition::all().add(<Entity as EntityTrait>::Column::#pascal_name.is_in(vs)),
                caustics::FieldOp::NotInVec(vs) => Condition::all().add(<Entity as EntityTrait>::Column::#pascal_name.is_not_in(vs)),
                caustics::FieldOp::IsNull => Condition::all().add(<Entity as EntityTrait>::Column::#pascal_name.is_null()),
                caustics::FieldOp::IsNotNull => Condition::all().add(<Entity as EntityTrait>::Column::#pascal_name.is_not_null()),
                // Catch-all for unsupported operations
                _ => panic!("Unsupported FieldOp operation for this field type"),
            }
        }
    } else {
        quote! {
            WhereParam::#pascal_name(op) => match op {
                caustics::FieldOp::Equals(v) => Condition::all().add(<Entity as EntityTrait>::Column::#pascal_name.eq(v)),
                caustics::FieldOp::NotEquals(v) => Condition::all().add(<Entity as EntityTrait>::Column::#pascal_name.ne(v)),
                caustics::FieldOp::InVec(vs) => Condition::all().add(<Entity as EntityTrait>::Column::#pascal_name.is_in(vs)),
                caustics::FieldOp::NotInVec(vs) => Condition::all().add(<Entity as EntityTrait>::Column::#pascal_name.is_not_in(vs)),
                // Catch-all for unsupported operations
                _ => panic!("Unsupported FieldOp operation for this field type"),
            }
        }
    }
}

/// Generate database-agnostic JSON field handler
fn generate_json_field_handler(
    pascal_name: &proc_macro2::Ident,
    is_nullable: bool,
    is_primary_key: bool,
) -> proc_macro2::TokenStream {
    if is_nullable {
        quote! {
            WhereParam::#pascal_name(op) => match op {
                caustics::FieldOp::Equals(v) => {
                    Condition::all().add(<Entity as EntityTrait>::Column::#pascal_name.eq(v))
                },
                caustics::FieldOp::NotEquals(v) => {
                    Condition::all().add(<Entity as EntityTrait>::Column::#pascal_name.ne(v))
                },
                caustics::FieldOp::Gt(val) => Condition::all().add(<Entity as EntityTrait>::Column::#pascal_name.gt(val)),
                caustics::FieldOp::Lt(val) => Condition::all().add(<Entity as EntityTrait>::Column::#pascal_name.lt(val)),
                caustics::FieldOp::Gte(val) => Condition::all().add(<Entity as EntityTrait>::Column::#pascal_name.gte(val)),
                caustics::FieldOp::Lte(val) => Condition::all().add(<Entity as EntityTrait>::Column::#pascal_name.lte(val)),
                caustics::FieldOp::InVec(vals) => Condition::all().add(<Entity as EntityTrait>::Column::#pascal_name.is_in(vals)),
                caustics::FieldOp::NotInVec(vals) => Condition::all().add(<Entity as EntityTrait>::Column::#pascal_name.is_not_in(vals)),
                caustics::FieldOp::IsNull => Condition::all().add(<Entity as EntityTrait>::Column::#pascal_name.is_null()),
                caustics::FieldOp::IsNotNull => Condition::all().add(<Entity as EntityTrait>::Column::#pascal_name.is_not_null()),
                // JSON-specific operations - use database-agnostic SQL
                caustics::FieldOp::JsonPath(path) => {
                    let json_path = path.join(".");
                        Condition::all().add(
                            sea_query::Expr::cust_with_values(
                                &format!("json_extract({}, ?) IS NOT NULL", <Entity as EntityTrait>::Column::#pascal_name.to_string()),
                            [format!("$.{}", json_path)]
                            )
                        )
                },
                caustics::FieldOp::JsonStringContains(s) => {
                    Condition::all().add(sea_query::Expr::cust_with_values(
                                &format!("json_extract({}, '$') LIKE ?", <Entity as EntityTrait>::Column::#pascal_name.to_string()),
                                [format!("%{}%", s)]
                    ))
                },
                caustics::FieldOp::JsonStringStartsWith(s) => {
                    Condition::all().add(sea_query::Expr::cust_with_values(
                                &format!("json_extract({}, '$') LIKE ?", <Entity as EntityTrait>::Column::#pascal_name.to_string()),
                                [format!("{}%", s)]
                    ))
                },
                caustics::FieldOp::JsonStringEndsWith(s) => {
                    Condition::all().add(sea_query::Expr::cust_with_values(
                                &format!("json_extract({}, '$') LIKE ?", <Entity as EntityTrait>::Column::#pascal_name.to_string()),
                                [format!("%{}", s)]
                    ))
                },
                caustics::FieldOp::JsonArrayContains(val) => {
                    Condition::all().add(sea_query::Expr::cust_with_values(
                                &format!("EXISTS (SELECT 1 FROM json_each({}) WHERE value = ?)", <Entity as EntityTrait>::Column::#pascal_name.to_string()),
                                [val.to_string()]
                    ))
                },
                caustics::FieldOp::JsonArrayStartsWith(val) => {
                    Condition::all().add(sea_query::Expr::cust_with_values(
                                &format!("json_extract({}, '$[0]') = ?", <Entity as EntityTrait>::Column::#pascal_name.to_string()),
                                [val.to_string()]
                    ))
                },
                caustics::FieldOp::JsonArrayEndsWith(val) => {
                    Condition::all().add(sea_query::Expr::cust_with_values(
                                &format!("json_extract({}, '$[#-1]') = ?", <Entity as EntityTrait>::Column::#pascal_name.to_string()),
                                [val.to_string()]
                    ))
                },
                caustics::FieldOp::JsonObjectContains(key) => {
                    Condition::all().add(sea_query::Expr::cust_with_values(
                                &format!("json_extract({}, ?) IS NOT NULL", <Entity as EntityTrait>::Column::#pascal_name.to_string()),
                        [format!("$.{}", key)]
                    ))
                },
                caustics::FieldOp::JsonNull(flag) => {
                    match flag {
                        caustics::JsonNullValueFilter::DbNull => Condition::all().add(<Entity as EntityTrait>::Column::#pascal_name.is_null()),
                        caustics::JsonNullValueFilter::JsonNull => Condition::all().add(<Entity as EntityTrait>::Column::#pascal_name.eq(serde_json::Value::Null)),
                        caustics::JsonNullValueFilter::AnyNull => Condition::all().add(sea_query::Expr::cust_with_values(
                            &format!("({} IS NULL OR {} = 'null')", <Entity as EntityTrait>::Column::#pascal_name.to_string(), <Entity as EntityTrait>::Column::#pascal_name.to_string()),
                            Vec::<sea_orm::Value>::new()
                        )),
                    }
                },
                // Catch-all for unsupported operations
                _ => panic!("Unsupported FieldOp operation for this field type"),
            }
        }
    } else {
        quote! {
            WhereParam::#pascal_name(op) => match op {
                caustics::FieldOp::Equals(val) => Condition::all().add(<Entity as EntityTrait>::Column::#pascal_name.eq(val)),
                caustics::FieldOp::NotEquals(val) => Condition::all().add(<Entity as EntityTrait>::Column::#pascal_name.ne(val)),
                caustics::FieldOp::Gt(val) => Condition::all().add(<Entity as EntityTrait>::Column::#pascal_name.gt(val)),
                caustics::FieldOp::Lt(val) => Condition::all().add(<Entity as EntityTrait>::Column::#pascal_name.lt(val)),
                caustics::FieldOp::Gte(val) => Condition::all().add(<Entity as EntityTrait>::Column::#pascal_name.gte(val)),
                caustics::FieldOp::Lte(val) => Condition::all().add(<Entity as EntityTrait>::Column::#pascal_name.lte(val)),
                caustics::FieldOp::InVec(vals) => Condition::all().add(<Entity as EntityTrait>::Column::#pascal_name.is_in(vals)),
                caustics::FieldOp::NotInVec(vals) => Condition::all().add(<Entity as EntityTrait>::Column::#pascal_name.is_not_in(vals)),
                // JSON-specific operations - use database-agnostic SQL (same as nullable version)
                caustics::FieldOp::JsonPath(path) => {
                    let json_path = path.join(".");
                        Condition::all().add(
                            sea_query::Expr::cust_with_values(
                                &format!("json_extract({}, ?) IS NOT NULL", <Entity as EntityTrait>::Column::#pascal_name.to_string()),
                            [format!("$.{}", json_path)]
                            )
                        )
                },
                caustics::FieldOp::JsonStringContains(s) => {
                    Condition::all().add(sea_query::Expr::cust_with_values(
                                &format!("json_extract({}, '$') LIKE ?", <Entity as EntityTrait>::Column::#pascal_name.to_string()),
                                [format!("%{}%", s)]
                    ))
                },
                caustics::FieldOp::JsonStringStartsWith(s) => {
                    Condition::all().add(sea_query::Expr::cust_with_values(
                                &format!("json_extract({}, '$') LIKE ?", <Entity as EntityTrait>::Column::#pascal_name.to_string()),
                                [format!("{}%", s)]
                    ))
                },
                caustics::FieldOp::JsonStringEndsWith(s) => {
                    Condition::all().add(sea_query::Expr::cust_with_values(
                                &format!("json_extract({}, '$') LIKE ?", <Entity as EntityTrait>::Column::#pascal_name.to_string()),
                                [format!("%{}", s)]
                    ))
                },
                caustics::FieldOp::JsonArrayContains(val) => {
                    Condition::all().add(sea_query::Expr::cust_with_values(
                                &format!("EXISTS (SELECT 1 FROM json_each({}) WHERE value = ?)", <Entity as EntityTrait>::Column::#pascal_name.to_string()),
                                [val.to_string()]
                    ))
                },
                caustics::FieldOp::JsonArrayStartsWith(val) => {
                    Condition::all().add(sea_query::Expr::cust_with_values(
                                &format!("json_extract({}, '$[0]') = ?", <Entity as EntityTrait>::Column::#pascal_name.to_string()),
                                [val.to_string()]
                    ))
                },
                caustics::FieldOp::JsonArrayEndsWith(val) => {
                    Condition::all().add(sea_query::Expr::cust_with_values(
                                &format!("json_extract({}, '$[#-1]') = ?", <Entity as EntityTrait>::Column::#pascal_name.to_string()),
                                [val.to_string()]
                    ))
                },
                caustics::FieldOp::JsonObjectContains(key) => {
                    Condition::all().add(sea_query::Expr::cust_with_values(
                                &format!("json_extract({}, ?) IS NOT NULL", <Entity as EntityTrait>::Column::#pascal_name.to_string()),
                        [format!("$.{}", key)]
                    ))
                },
                caustics::FieldOp::JsonNull(flag) => {
                    match flag {
                        caustics::JsonNullValueFilter::DbNull => Condition::all().add(<Entity as EntityTrait>::Column::#pascal_name.is_null()),
                        caustics::JsonNullValueFilter::JsonNull => Condition::all().add(<Entity as EntityTrait>::Column::#pascal_name.eq(serde_json::Value::Null)),
                        caustics::JsonNullValueFilter::AnyNull => Condition::all().add(sea_query::Expr::cust_with_values(
                            &format!("({} IS NULL OR {} = 'null')", <Entity as EntityTrait>::Column::#pascal_name.to_string(), <Entity as EntityTrait>::Column::#pascal_name.to_string()),
                            Vec::<sea_orm::Value>::new()
                        )),
                    }
                },
                // Catch-all for unsupported operations
                _ => panic!("Unsupported FieldOp operation for this field type"),
            }
        }
    }
}

/// Generate generic field handler for unknown types
fn generate_generic_field_handler(
    pascal_name: &proc_macro2::Ident,
    is_primary_key: bool,
) -> proc_macro2::TokenStream {
    quote! {
        WhereParam::#pascal_name(op) => match op {
            caustics::FieldOp::Equals(v) => Condition::all().add(<Entity as EntityTrait>::Column::#pascal_name.eq(v)),
            caustics::FieldOp::NotEquals(v) => Condition::all().add(<Entity as EntityTrait>::Column::#pascal_name.ne(v)),
            caustics::FieldOp::Gt(v) => Condition::all().add(<Entity as EntityTrait>::Column::#pascal_name.gt(v)),
            caustics::FieldOp::Lt(v) => Condition::all().add(<Entity as EntityTrait>::Column::#pascal_name.lt(v)),
            caustics::FieldOp::Gte(v) => Condition::all().add(<Entity as EntityTrait>::Column::#pascal_name.gte(v)),
            caustics::FieldOp::Lte(v) => Condition::all().add(<Entity as EntityTrait>::Column::#pascal_name.lte(v)),
            caustics::FieldOp::InVec(vs) => Condition::all().add(<Entity as EntityTrait>::Column::#pascal_name.is_in(vs)),
            caustics::FieldOp::IsNull => Condition::all().add(<Entity as EntityTrait>::Column::#pascal_name.is_null()),
            caustics::FieldOp::IsNotNull => Condition::all().add(<Entity as EntityTrait>::Column::#pascal_name.is_not_null()),
            // Catch-all for unsupported operations
            _ => panic!("Unsupported FieldOp operation for this field type"),
        }
    }
}

/// Generate type-specific operations (like equals for all types)
fn generate_type_specific_operations(
    field_type: &FieldType,
    pascal_name: &proc_macro2::Ident,
    ty: &syn::Type,
) -> proc_macro2::TokenStream {
    // For all field types, generate an equals function that converts to sea_orm::Value
    quote! {
        pub fn equals<T: caustics::ToSeaOrmValue>(value: T) -> WhereParam {
            WhereParam::#pascal_name(caustics::FieldOp::equals(value))
        }
    }
}

/// Generate primary key operations that accept CausticsKey and convert to field type
fn generate_primary_key_operations(
    field_type: &FieldType,
    pascal_name: &proc_macro2::Ident,
    ty: &syn::Type,
) -> proc_macro2::TokenStream {
    // For primary key fields, generate operations that accept CausticsKey and convert to sea_orm::Value
    // Note: equals is handled by unique_where_fn for unique fields, so we don't generate it here
    quote! {
        pub fn not_equals<T: Into<caustics::CausticsKey>>(value: T) -> WhereParam {
            let key = value.into();
            WhereParam::#pascal_name(caustics::FieldOp::not_equals(key))
        }
        pub fn in_vec<T: Into<caustics::CausticsKey>>(values: Vec<T>) -> WhereParam {
            let keys: Vec<caustics::CausticsKey> = values.into_iter().map(|v| v.into()).collect();
            WhereParam::#pascal_name(caustics::FieldOp::in_vec(keys))
        }
        pub fn not_in_vec<T: Into<caustics::CausticsKey>>(values: Vec<T>) -> WhereParam {
            let keys: Vec<caustics::CausticsKey> = values.into_iter().map(|v| v.into()).collect();
            WhereParam::#pascal_name(caustics::FieldOp::not_in_vec(keys))
        }
    }
}

