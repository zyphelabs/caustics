// Logic for generating field variants for WhereParam enum and their filter functions.
// This module will support all types and will be long.

use heck::{ToPascalCase, ToSnakeCase};
use quote::{format_ident, quote};

/// Generate field variants, match arms, and field operator modules for WhereParam enum and filters.
pub fn generate_where_param_logic(
    fields: &[&syn::Field],
    unique_fields: &[&syn::Field],
    full_mod_path: &syn::Path,
    relations: &[crate::entity::Relation],
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

        // Detect field type for appropriate operation generation
        let field_type = detect_field_type(ty);

        // WhereParam variant uses FieldOp<T>
        where_field_variants.push(quote! { #pascal_name(FieldOp<#ty>) });

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
                        super::WhereParam::#pascal_name(caustics::FieldOp::Equals(v))
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
        };

        // Generate type-specific operations
        let type_specific_ops = if !is_unique {
            generate_type_specific_operations(&field_type, &pascal_name, ty)
        } else {
            quote! {} // Don't generate equals for unique fields since unique_where_fn already has it
        };

        // Field-level `not` alias (PCR-style): maps to NotEquals with same value type
        let field_not_alias = quote! {
            pub fn not<T: Into<#ty>>(value: T) -> WhereParam {
                WhereParam::#pascal_name(caustics::FieldOp::NotEquals(value.into()))
            }
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
        let comparison_ops = if !matches!(
            field_type,
            FieldType::Boolean
                | FieldType::OptionBoolean
                | FieldType::Json
                | FieldType::OptionJson
                | FieldType::Uuid
                | FieldType::OptionUuid
        ) {
            quote! {
            pub fn not_equals<T: Into<#ty>>(value: T) -> WhereParam {
                WhereParam::#pascal_name(caustics::FieldOp::NotEquals(value.into()))
            }
            pub fn gt<T: Into<#ty>>(value: T) -> WhereParam {
                WhereParam::#pascal_name(caustics::FieldOp::Gt(value.into()))
            }
            pub fn lt<T: Into<#ty>>(value: T) -> WhereParam {
                WhereParam::#pascal_name(caustics::FieldOp::Lt(value.into()))
            }
            pub fn gte<T: Into<#ty>>(value: T) -> WhereParam {
                WhereParam::#pascal_name(caustics::FieldOp::Gte(value.into()))
            }
            pub fn lte<T: Into<#ty>>(value: T) -> WhereParam {
                WhereParam::#pascal_name(caustics::FieldOp::Lte(value.into()))
            }
            }
        } else {
            // For boolean, UUID, and JSON fields, only provide equals/not_equals
            quote! {
                pub fn not_equals<T: Into<#ty>>(value: T) -> WhereParam {
                    WhereParam::#pascal_name(caustics::FieldOp::NotEquals(value.into()))
                }
            }
        };

        // Collection operations (for all types)
        let collection_ops = quote! {
            pub fn in_vec<T: Into<#ty>>(values: Vec<T>) -> WhereParam {
                WhereParam::#pascal_name(caustics::FieldOp::InVec(values.into_iter().map(|v| v.into()).collect()))
            }
            pub fn not_in_vec<T: Into<#ty>>(values: Vec<T>) -> WhereParam {
                WhereParam::#pascal_name(caustics::FieldOp::NotInVec(values.into_iter().map(|v| v.into()).collect()))
            }
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
            FieldType::Json | FieldType::OptionJson => {
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

    // Add relation condition variant for advanced relation operations
    where_field_variants.push(quote! { RelationCondition(caustics::RelationCondition) });

    // Generate a function that processes all WhereParams together, properly handling QueryMode
    let where_params_to_condition_fn =
        generate_where_params_to_condition_function(&fields, relations);

    let where_match_arms: Vec<proc_macro2::TokenStream> = vec![where_params_to_condition_fn];
    (where_field_variants, where_match_arms, field_ops)
}

/// Generate a function that converts Vec<WhereParam> to Condition, properly handling QueryMode
fn generate_where_params_to_condition_function(
    fields: &[&syn::Field],
    relations: &[crate::entity::Relation],
) -> proc_macro2::TokenStream {
    let mut field_handlers = Vec::new();
    let mut mode_handlers = Vec::new();

    for field in fields.iter() {
        let name = field.ident.as_ref().unwrap();
        let pascal_name = format_ident!("{}", name.to_string().to_pascal_case());
        let ty = &field.ty;

        // Comprehensive type detection
        let field_type = detect_field_type(ty);

        // Generate field operation handler based on type
        match field_type {
            FieldType::String => {
                field_handlers.push(generate_string_field_handler(&pascal_name, false));
                mode_handlers.push(generate_mode_handler(&pascal_name, &name));
            }
            FieldType::OptionString => {
                field_handlers.push(generate_string_field_handler(&pascal_name, true));
                mode_handlers.push(generate_mode_handler(&pascal_name, &name));
            }
            FieldType::Integer => {
                field_handlers.push(generate_numeric_field_handler(&pascal_name, false));
            }
            FieldType::OptionInteger => {
                field_handlers.push(generate_numeric_field_handler(&pascal_name, true));
            }
            FieldType::Float => {
                field_handlers.push(generate_numeric_field_handler(&pascal_name, false));
            }
            FieldType::OptionFloat => {
                field_handlers.push(generate_numeric_field_handler(&pascal_name, true));
            }
            FieldType::Boolean => {
                field_handlers.push(generate_boolean_field_handler(&pascal_name, false));
            }
            FieldType::OptionBoolean => {
                field_handlers.push(generate_boolean_field_handler(&pascal_name, true));
            }
            FieldType::DateTime => {
                field_handlers.push(generate_datetime_field_handler(&pascal_name, false));
            }
            FieldType::OptionDateTime => {
                field_handlers.push(generate_datetime_field_handler(&pascal_name, true));
            }
            FieldType::Uuid => {
                field_handlers.push(generate_uuid_field_handler(&pascal_name, false));
            }
            FieldType::OptionUuid => {
                field_handlers.push(generate_uuid_field_handler(&pascal_name, true));
            }
            FieldType::Json => {
                field_handlers.push(generate_json_field_handler(&pascal_name, false));
            }
            FieldType::OptionJson => {
                field_handlers.push(generate_json_field_handler(&pascal_name, true));
            }
            FieldType::Other => {
                field_handlers.push(generate_generic_field_handler(&pascal_name));
            }
        }
    }

    // Generate dynamic relation match arms
    let mut relation_match_arms = Vec::new();
    let mut field_mappings = Vec::new();

    for relation in relations {
        let relation_name = &relation.name;
        let relation_name_str = relation_name.to_snake_case();
        let target = &relation.target;

        // Get the foreign key column identifier
        let foreign_key_column_ident = if let Some(fk_col) = &relation.foreign_key_column {
            format_ident!("{}", fk_col)
        } else {
            format_ident!("Id") // fallback
        };

        // Get the foreign key column name as string
        let foreign_key_column_str = if let Some(fk_col) = &relation.foreign_key_column {
            // Convert PascalCase to snake_case for database column name
            fk_col.to_string().to_snake_case()
        } else {
            "id".to_string() // fallback
        };

        // Get table names from relation metadata
        let target_table_name_str = relation
            .target_table_name
            .as_ref()
            .unwrap_or(&relation_name_str)
            .to_string();
        let current_table_name_str = relation
            .current_table_name
            .as_ref()
            .unwrap_or(&"unknown".to_string())
            .to_string();

        // Generate completely agnostic field mappings that work with any entity
        let target_field_mappings = generate_target_field_mappings(target, &target_table_name_str);
        field_mappings.extend(target_field_mappings);

        // Generate match arm for this relation
        let relation_match_arm = quote! {
            #relation_name_str => {
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

    quote! {
        /// Convert a Filter to a SeaORM condition for the target entity
        fn convert_filter_to_condition<T: EntityTrait>(filter: &caustics::Filter, table_name: &str) -> sea_query::Condition {
            use sea_orm::{EntityTrait, ColumnTrait};
            use sea_query::Condition;

            // Type-safe field handling - direct FieldOp matching
            // Fallback to no-op Condition for unsupported operations
            match &filter.operation {
                caustics::FieldOp::Equals(value) => {
                    Condition::all().add(sea_query::Expr::cust_with_values(
                        &format!("\"{}\".{} = ?", table_name, filter.field),
                        [sea_orm::Value::from(value)]
                    ))
                },
                caustics::FieldOp::NotEquals(value) => {
                    Condition::all().add(sea_query::Expr::cust_with_values(
                        &format!("\"{}\".{} != ?", table_name, filter.field),
                        [sea_orm::Value::from(value)]
                    ))
                },
                caustics::FieldOp::Gt(value) => {
                    Condition::all().add(sea_query::Expr::cust_with_values(
                        &format!("\"{}\".{} > ?", table_name, filter.field),
                        [sea_orm::Value::from(value)]
                    ))
                },
                caustics::FieldOp::Lt(value) => {
                    Condition::all().add(sea_query::Expr::cust_with_values(
                        &format!("\"{}\".{} < ?", table_name, filter.field),
                        [sea_orm::Value::from(value)]
                    ))
                },
                caustics::FieldOp::Gte(value) => {
                    Condition::all().add(sea_query::Expr::cust_with_values(
                        &format!("\"{}\".{} >= ?", table_name, filter.field),
                        [sea_orm::Value::from(value)]
                    ))
                },
                caustics::FieldOp::Lte(value) => {
                    Condition::all().add(sea_query::Expr::cust_with_values(
                        &format!("\"{}\".{} <= ?", table_name, filter.field),
                        [sea_orm::Value::from(value)]
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
                        values.iter().map(|v| sea_orm::Value::from(v)).collect::<Vec<_>>()
                    ))
                },
                caustics::FieldOp::NotInVec(values) => {
                    Condition::all().add(sea_query::Expr::cust_with_values(
                        &format!("\"{}\".{} NOT IN ({})", table_name, filter.field,
                            values.iter().map(|_| "?").collect::<Vec<_>>().join(",")),
                        values.iter().map(|v| sea_orm::Value::from(v)).collect::<Vec<_>>()
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
                // Relation operations (should not be used in field mappings) -> no-op
                caustics::FieldOp::Some(_) | caustics::FieldOp::Every(_) | caustics::FieldOp::None(_) => Condition::all(),
            }
        }

        /// Convert a vector of WhereParams to a SeaORM Condition, properly handling QueryMode
        pub fn where_params_to_condition(params: Vec<WhereParam>, database_backend: sea_orm::DatabaseBackend) -> sea_query::Condition {
            use std::collections::HashMap;
            use sea_orm::{EntityTrait, ColumnTrait, QuerySelect, QueryTrait};
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
                    WhereParam::RelationCondition(relation_condition) => {
                        match relation_condition.relation_name.as_ref() {
                            // ... dynamically generated arms ...
                            #(
                                #relation_match_arms
                            )*
                            _ => panic!("Unknown relation: {}", relation_condition.relation_name),
                        }
                    },
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
) -> proc_macro2::TokenStream {
    let field_name_str = pascal_name.to_string().to_lowercase();

    if is_nullable {
        quote! {
            WhereParam::#pascal_name(op) => {
                let query_mode = query_modes.get(#field_name_str).copied().unwrap_or(caustics::QueryMode::Default);
                match op {
                    caustics::FieldOp::Equals(v) => {
                        match v {
                            Some(val) => {
                                if query_mode == caustics::QueryMode::Insensitive {
                                    // Database-agnostic case insensitive equality
                                    match database_backend {
                                        sea_orm::DatabaseBackend::Postgres => {
                                            Condition::all().add(
                                                sea_query::Expr::cust_with_values(
                                                    &format!("UPPER({}) = UPPER(?)", <Entity as EntityTrait>::Column::#pascal_name.to_string()),
                                                    [val]
                                                )
                                            )
                                        },
                                        _ => {
                                            // MySQL, MariaDB, SQLite - use UPPER() for consistency
                                            Condition::all().add(
                                                sea_query::Expr::cust_with_values(
                                                    &format!("UPPER({}) = UPPER(?)", <Entity as EntityTrait>::Column::#pascal_name.to_string()),
                                                    [val]
                                                )
                                            )
                                        }
                                    }
                                } else {
                                    Condition::all().add(<Entity as EntityTrait>::Column::#pascal_name.eq(val))
                                }
                            },
                            None => Condition::all().add(<Entity as EntityTrait>::Column::#pascal_name.is_null()),
                        }
                    },
                    caustics::FieldOp::NotEquals(v) => {
                        match v {
                            Some(val) => {
                                if query_mode == caustics::QueryMode::Insensitive {
                                    match database_backend {
                                        sea_orm::DatabaseBackend::Postgres => {
                                        Condition::all().add(
                                            sea_query::Expr::cust_with_values(
                                                &format!("UPPER({}) != UPPER(?)", <Entity as EntityTrait>::Column::#pascal_name.to_string()),
                                                [val]
                                            )
                                        )
                                        },
                                        _ => {
                                            Condition::all().add(
                                                sea_query::Expr::cust_with_values(
                                                    &format!("UPPER({}) != UPPER(?)", <Entity as EntityTrait>::Column::#pascal_name.to_string()),
                                                    [val]
                                                )
                                            )
                                        }
                                    }
                                } else {
                                    Condition::all().add(<Entity as EntityTrait>::Column::#pascal_name.ne(val))
                                }
                            },
                            None => Condition::all().add(<Entity as EntityTrait>::Column::#pascal_name.is_not_null()),
                        }
                    },
                    FieldOp::Contains(s) => {
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
                    FieldOp::StartsWith(s) => {
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
                        match v {
                            Some(val) => Condition::all().add(<Entity as EntityTrait>::Column::#pascal_name.gt(val)),
                            None => Condition::all().add(<Entity as EntityTrait>::Column::#pascal_name.is_not_null()),
                        }
                    },
                    caustics::FieldOp::Lt(v) => {
                        match v {
                            Some(val) => Condition::all().add(<Entity as EntityTrait>::Column::#pascal_name.lt(val)),
                            None => Condition::all().add(<Entity as EntityTrait>::Column::#pascal_name.is_not_null()),
                        }
                    },
                    caustics::FieldOp::Gte(v) => {
                        match v {
                            Some(val) => Condition::all().add(<Entity as EntityTrait>::Column::#pascal_name.gte(val)),
                            None => Condition::all().add(<Entity as EntityTrait>::Column::#pascal_name.is_not_null()),
                        }
                    },
                    caustics::FieldOp::Lte(v) => {
                        match v {
                            Some(val) => Condition::all().add(<Entity as EntityTrait>::Column::#pascal_name.lte(val)),
                            None => Condition::all().add(<Entity as EntityTrait>::Column::#pascal_name.is_not_null()),
                        }
                    },
                    caustics::FieldOp::InVec(v) => Condition::all().add(<Entity as EntityTrait>::Column::#pascal_name.is_in(v)),
                    caustics::FieldOp::NotInVec(v) => Condition::all().add(<Entity as EntityTrait>::Column::#pascal_name.is_not_in(v)),
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
                    FieldOp::Equals(val) => {
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
                    FieldOp::NotEquals(val) => {
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
                    FieldOp::Contains(s) => {
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
                    FieldOp::StartsWith(s) => {
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
                    FieldOp::EndsWith(s) => {
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
                    FieldOp::Gt(val) => Condition::all().add(<Entity as EntityTrait>::Column::#pascal_name.gt(val)),
                    FieldOp::Lt(val) => Condition::all().add(<Entity as EntityTrait>::Column::#pascal_name.lt(val)),
                    FieldOp::Gte(val) => Condition::all().add(<Entity as EntityTrait>::Column::#pascal_name.gte(val)),
                    FieldOp::Lte(val) => Condition::all().add(<Entity as EntityTrait>::Column::#pascal_name.lte(val)),
                    FieldOp::InVec(vs) => Condition::all().add(<Entity as EntityTrait>::Column::#pascal_name.is_in(vs)),
                    FieldOp::NotInVec(vs) => Condition::all().add(<Entity as EntityTrait>::Column::#pascal_name.is_not_in(vs)),
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
) -> proc_macro2::TokenStream {
    if is_nullable {
        quote! {
            WhereParam::#pascal_name(op) => match op {
                FieldOp::Equals(v) => {
                    match v {
                        Some(val) => Condition::all().add(<Entity as EntityTrait>::Column::#pascal_name.eq(val)),
                        None => Condition::all().add(<Entity as EntityTrait>::Column::#pascal_name.is_null()),
                    }
                },
                FieldOp::NotEquals(v) => {
                    match v {
                        Some(val) => Condition::all().add(<Entity as EntityTrait>::Column::#pascal_name.ne(val)),
                        None => Condition::all().add(<Entity as EntityTrait>::Column::#pascal_name.is_not_null()),
                    }
                },
                FieldOp::Gt(v) => {
                    match v {
                        Some(val) => Condition::all().add(<Entity as EntityTrait>::Column::#pascal_name.gt(val)),
                        None => Condition::all().add(<Entity as EntityTrait>::Column::#pascal_name.is_not_null()),
                    }
                },
                FieldOp::Lt(v) => {
                    match v {
                        Some(val) => Condition::all().add(<Entity as EntityTrait>::Column::#pascal_name.lt(val)),
                        None => Condition::all().add(<Entity as EntityTrait>::Column::#pascal_name.is_null()),
                    }
                },
                FieldOp::Gte(v) => {
                    match v {
                        Some(val) => Condition::all().add(<Entity as EntityTrait>::Column::#pascal_name.gte(val)),
                        None => Condition::all().add(<Entity as EntityTrait>::Column::#pascal_name.is_not_null()),
                    }
                },
                FieldOp::Lte(v) => {
                    match v {
                        Some(val) => Condition::all().add(<Entity as EntityTrait>::Column::#pascal_name.lte(val)),
                        None => Condition::all().add(<Entity as EntityTrait>::Column::#pascal_name.is_null()),
                    }
                },
                FieldOp::InVec(vs) => Condition::all().add(<Entity as EntityTrait>::Column::#pascal_name.is_in(vs)),
                FieldOp::NotInVec(vs) => Condition::all().add(<Entity as EntityTrait>::Column::#pascal_name.is_not_in(vs)),
                FieldOp::IsNull => Condition::all().add(<Entity as EntityTrait>::Column::#pascal_name.is_null()),
                FieldOp::IsNotNull => Condition::all().add(<Entity as EntityTrait>::Column::#pascal_name.is_not_null()),
                // Catch-all for unsupported operations
                _ => panic!("Unsupported FieldOp operation for this field type"),
            }
        }
    } else {
        quote! {
            WhereParam::#pascal_name(op) => match op {
                FieldOp::Equals(v) => Condition::all().add(<Entity as EntityTrait>::Column::#pascal_name.eq(v)),
                FieldOp::NotEquals(v) => Condition::all().add(<Entity as EntityTrait>::Column::#pascal_name.ne(v)),
                FieldOp::Gt(v) => Condition::all().add(<Entity as EntityTrait>::Column::#pascal_name.gt(v)),
                FieldOp::Lt(v) => Condition::all().add(<Entity as EntityTrait>::Column::#pascal_name.lt(v)),
                FieldOp::Gte(v) => Condition::all().add(<Entity as EntityTrait>::Column::#pascal_name.gte(v)),
                FieldOp::Lte(v) => Condition::all().add(<Entity as EntityTrait>::Column::#pascal_name.lte(v)),
                FieldOp::InVec(vs) => Condition::all().add(<Entity as EntityTrait>::Column::#pascal_name.is_in(vs)),
                FieldOp::NotInVec(vs) => Condition::all().add(<Entity as EntityTrait>::Column::#pascal_name.is_not_in(vs)),
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
) -> proc_macro2::TokenStream {
    if is_nullable {
        quote! {
            WhereParam::#pascal_name(op) => match op {
                FieldOp::Equals(v) => {
                    match v {
                        Some(val) => Condition::all().add(<Entity as EntityTrait>::Column::#pascal_name.eq(val)),
                        None => Condition::all().add(<Entity as EntityTrait>::Column::#pascal_name.is_null()),
                    }
                },
                FieldOp::NotEquals(v) => {
                    match v {
                        Some(val) => Condition::all().add(<Entity as EntityTrait>::Column::#pascal_name.ne(val)),
                        None => Condition::all().add(<Entity as EntityTrait>::Column::#pascal_name.is_not_null()),
                    }
                },
                FieldOp::InVec(vs) => Condition::all().add(<Entity as EntityTrait>::Column::#pascal_name.is_in(vs)),
                FieldOp::NotInVec(vs) => Condition::all().add(<Entity as EntityTrait>::Column::#pascal_name.is_not_in(vs)),
                FieldOp::IsNull => Condition::all().add(<Entity as EntityTrait>::Column::#pascal_name.is_null()),
                FieldOp::IsNotNull => Condition::all().add(<Entity as EntityTrait>::Column::#pascal_name.is_not_null()),
                // Catch-all for unsupported operations
                _ => panic!("Unsupported FieldOp operation for this field type"),
            }
        }
    } else {
        quote! {
            WhereParam::#pascal_name(op) => match op {
                FieldOp::Equals(v) => Condition::all().add(<Entity as EntityTrait>::Column::#pascal_name.eq(v)),
                FieldOp::NotEquals(v) => Condition::all().add(<Entity as EntityTrait>::Column::#pascal_name.ne(v)),
                FieldOp::InVec(vs) => Condition::all().add(<Entity as EntityTrait>::Column::#pascal_name.is_in(vs)),
                FieldOp::NotInVec(vs) => Condition::all().add(<Entity as EntityTrait>::Column::#pascal_name.is_not_in(vs)),
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
) -> proc_macro2::TokenStream {
    if is_nullable {
        quote! {
            WhereParam::#pascal_name(op) => match op {
                FieldOp::Equals(v) => {
                    match v {
                        Some(val) => Condition::all().add(<Entity as EntityTrait>::Column::#pascal_name.eq(val)),
                        None => Condition::all().add(<Entity as EntityTrait>::Column::#pascal_name.is_null()),
                    }
                },
                FieldOp::NotEquals(v) => {
                    match v {
                        Some(val) => Condition::all().add(<Entity as EntityTrait>::Column::#pascal_name.ne(val)),
                        None => Condition::all().add(<Entity as EntityTrait>::Column::#pascal_name.is_not_null()),
                    }
                },
                FieldOp::Gt(v) => {
                    match v {
                        Some(val) => Condition::all().add(<Entity as EntityTrait>::Column::#pascal_name.gt(val)),
                        None => Condition::all().add(<Entity as EntityTrait>::Column::#pascal_name.is_not_null()),
                    }
                },
                FieldOp::Lt(v) => {
                    match v {
                        Some(val) => Condition::all().add(<Entity as EntityTrait>::Column::#pascal_name.lt(val)),
                        None => Condition::all().add(<Entity as EntityTrait>::Column::#pascal_name.is_null()),
                    }
                },
                FieldOp::Gte(v) => {
                    match v {
                        Some(val) => Condition::all().add(<Entity as EntityTrait>::Column::#pascal_name.gte(val)),
                        None => Condition::all().add(<Entity as EntityTrait>::Column::#pascal_name.is_not_null()),
                    }
                },
                FieldOp::Lte(v) => {
                    match v {
                        Some(val) => Condition::all().add(<Entity as EntityTrait>::Column::#pascal_name.lte(val)),
                        None => Condition::all().add(<Entity as EntityTrait>::Column::#pascal_name.is_null()),
                    }
                },
                FieldOp::InVec(vs) => Condition::all().add(<Entity as EntityTrait>::Column::#pascal_name.is_in(vs)),
                FieldOp::NotInVec(vs) => Condition::all().add(<Entity as EntityTrait>::Column::#pascal_name.is_not_in(vs)),
                FieldOp::IsNull => Condition::all().add(<Entity as EntityTrait>::Column::#pascal_name.is_null()),
                FieldOp::IsNotNull => Condition::all().add(<Entity as EntityTrait>::Column::#pascal_name.is_not_null()),
                // Catch-all for unsupported operations
                _ => panic!("Unsupported FieldOp operation for this field type"),
            }
        }
    } else {
        quote! {
            WhereParam::#pascal_name(op) => match op {
                FieldOp::Equals(v) => Condition::all().add(<Entity as EntityTrait>::Column::#pascal_name.eq(v)),
                FieldOp::NotEquals(v) => Condition::all().add(<Entity as EntityTrait>::Column::#pascal_name.ne(v)),
                FieldOp::Gt(v) => Condition::all().add(<Entity as EntityTrait>::Column::#pascal_name.gt(v)),
                FieldOp::Lt(v) => Condition::all().add(<Entity as EntityTrait>::Column::#pascal_name.lt(v)),
                FieldOp::Gte(v) => Condition::all().add(<Entity as EntityTrait>::Column::#pascal_name.gte(v)),
                FieldOp::Lte(v) => Condition::all().add(<Entity as EntityTrait>::Column::#pascal_name.lte(v)),
                FieldOp::InVec(vs) => Condition::all().add(<Entity as EntityTrait>::Column::#pascal_name.is_in(vs)),
                FieldOp::NotInVec(vs) => Condition::all().add(<Entity as EntityTrait>::Column::#pascal_name.is_not_in(vs)),
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
) -> proc_macro2::TokenStream {
    if is_nullable {
        quote! {
            WhereParam::#pascal_name(op) => match op {
                FieldOp::Equals(v) => {
                    match v {
                        Some(val) => Condition::all().add(<Entity as EntityTrait>::Column::#pascal_name.eq(val)),
                        None => Condition::all().add(<Entity as EntityTrait>::Column::#pascal_name.is_null()),
                    }
                },
                FieldOp::NotEquals(v) => {
                    match v {
                        Some(val) => Condition::all().add(<Entity as EntityTrait>::Column::#pascal_name.ne(val)),
                        None => Condition::all().add(<Entity as EntityTrait>::Column::#pascal_name.is_not_null()),
                    }
                },
                FieldOp::InVec(vs) => Condition::all().add(<Entity as EntityTrait>::Column::#pascal_name.is_in(vs)),
                FieldOp::NotInVec(vs) => Condition::all().add(<Entity as EntityTrait>::Column::#pascal_name.is_not_in(vs)),
                FieldOp::IsNull => Condition::all().add(<Entity as EntityTrait>::Column::#pascal_name.is_null()),
                FieldOp::IsNotNull => Condition::all().add(<Entity as EntityTrait>::Column::#pascal_name.is_not_null()),
                // Catch-all for unsupported operations
                _ => panic!("Unsupported FieldOp operation for this field type"),
            }
        }
    } else {
        quote! {
            WhereParam::#pascal_name(op) => match op {
                FieldOp::Equals(v) => Condition::all().add(<Entity as EntityTrait>::Column::#pascal_name.eq(v)),
                FieldOp::NotEquals(v) => Condition::all().add(<Entity as EntityTrait>::Column::#pascal_name.ne(v)),
                FieldOp::InVec(vs) => Condition::all().add(<Entity as EntityTrait>::Column::#pascal_name.is_in(vs)),
                FieldOp::NotInVec(vs) => Condition::all().add(<Entity as EntityTrait>::Column::#pascal_name.is_not_in(vs)),
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
) -> proc_macro2::TokenStream {
    if is_nullable {
        quote! {
            WhereParam::#pascal_name(op) => match op {
                FieldOp::Equals(v) => {
                    match v {
                        Some(val) => Condition::all().add(<Entity as EntityTrait>::Column::#pascal_name.eq(val)),
                        None => Condition::all().add(<Entity as EntityTrait>::Column::#pascal_name.is_null()),
                    }
                },
                FieldOp::NotEquals(v) => {
                    match v {
                        Some(val) => Condition::all().add(<Entity as EntityTrait>::Column::#pascal_name.ne(val)),
                        None => Condition::all().add(<Entity as EntityTrait>::Column::#pascal_name.is_not_null()),
                    }
                },
                FieldOp::Gt(val) => Condition::all().add(<Entity as EntityTrait>::Column::#pascal_name.gt(val)),
                FieldOp::Lt(val) => Condition::all().add(<Entity as EntityTrait>::Column::#pascal_name.lt(val)),
                FieldOp::Gte(val) => Condition::all().add(<Entity as EntityTrait>::Column::#pascal_name.gte(val)),
                FieldOp::Lte(val) => Condition::all().add(<Entity as EntityTrait>::Column::#pascal_name.lte(val)),
                FieldOp::InVec(vals) => Condition::all().add(<Entity as EntityTrait>::Column::#pascal_name.is_in(vals)),
                FieldOp::NotInVec(vals) => Condition::all().add(<Entity as EntityTrait>::Column::#pascal_name.is_not_in(vals)),
                FieldOp::IsNull => Condition::all().add(<Entity as EntityTrait>::Column::#pascal_name.is_null()),
                FieldOp::IsNotNull => Condition::all().add(<Entity as EntityTrait>::Column::#pascal_name.is_not_null()),
                // JSON-specific operations - use database-agnostic SQL
                FieldOp::JsonPath(path) => {
                    let json_path = path.join(".");
                        Condition::all().add(
                            sea_query::Expr::cust_with_values(
                                &format!("json_extract({}, ?) IS NOT NULL", <Entity as EntityTrait>::Column::#pascal_name.to_string()),
                            [format!("$.{}", json_path)]
                            )
                        )
                },
                FieldOp::JsonStringContains(s) => {
                    Condition::all().add(sea_query::Expr::cust_with_values(
                                &format!("json_extract({}, '$') LIKE ?", <Entity as EntityTrait>::Column::#pascal_name.to_string()),
                                [format!("%{}%", s)]
                    ))
                },
                FieldOp::JsonStringStartsWith(s) => {
                    Condition::all().add(sea_query::Expr::cust_with_values(
                                &format!("json_extract({}, '$') LIKE ?", <Entity as EntityTrait>::Column::#pascal_name.to_string()),
                                [format!("{}%", s)]
                    ))
                },
                FieldOp::JsonStringEndsWith(s) => {
                    Condition::all().add(sea_query::Expr::cust_with_values(
                                &format!("json_extract({}, '$') LIKE ?", <Entity as EntityTrait>::Column::#pascal_name.to_string()),
                                [format!("%{}", s)]
                    ))
                },
                FieldOp::JsonArrayContains(val) => {
                    Condition::all().add(sea_query::Expr::cust_with_values(
                                &format!("EXISTS (SELECT 1 FROM json_each({}) WHERE value = ?)", <Entity as EntityTrait>::Column::#pascal_name.to_string()),
                                [val.to_string()]
                    ))
                },
                FieldOp::JsonArrayStartsWith(val) => {
                    Condition::all().add(sea_query::Expr::cust_with_values(
                                &format!("json_extract({}, '$[0]') = ?", <Entity as EntityTrait>::Column::#pascal_name.to_string()),
                                [val.to_string()]
                    ))
                },
                FieldOp::JsonArrayEndsWith(val) => {
                    Condition::all().add(sea_query::Expr::cust_with_values(
                                &format!("json_extract({}, '$[#-1]') = ?", <Entity as EntityTrait>::Column::#pascal_name.to_string()),
                                [val.to_string()]
                    ))
                },
                FieldOp::JsonObjectContains(key) => {
                    Condition::all().add(sea_query::Expr::cust_with_values(
                                &format!("json_extract({}, ?) IS NOT NULL", <Entity as EntityTrait>::Column::#pascal_name.to_string()),
                        [format!("$.{}", key)]
                    ))
                },
                // Catch-all for unsupported operations
                _ => panic!("Unsupported FieldOp operation for this field type"),
            }
        }
    } else {
        quote! {
            WhereParam::#pascal_name(op) => match op {
                FieldOp::Equals(val) => Condition::all().add(<Entity as EntityTrait>::Column::#pascal_name.eq(val)),
                FieldOp::NotEquals(val) => Condition::all().add(<Entity as EntityTrait>::Column::#pascal_name.ne(val)),
                FieldOp::Gt(val) => Condition::all().add(<Entity as EntityTrait>::Column::#pascal_name.gt(val)),
                FieldOp::Lt(val) => Condition::all().add(<Entity as EntityTrait>::Column::#pascal_name.lt(val)),
                FieldOp::Gte(val) => Condition::all().add(<Entity as EntityTrait>::Column::#pascal_name.gte(val)),
                FieldOp::Lte(val) => Condition::all().add(<Entity as EntityTrait>::Column::#pascal_name.lte(val)),
                FieldOp::InVec(vals) => Condition::all().add(<Entity as EntityTrait>::Column::#pascal_name.is_in(vals)),
                FieldOp::NotInVec(vals) => Condition::all().add(<Entity as EntityTrait>::Column::#pascal_name.is_not_in(vals)),
                // JSON-specific operations - use database-agnostic SQL (same as nullable version)
                FieldOp::JsonPath(path) => {
                    let json_path = path.join(".");
                        Condition::all().add(
                            sea_query::Expr::cust_with_values(
                                &format!("json_extract({}, ?) IS NOT NULL", <Entity as EntityTrait>::Column::#pascal_name.to_string()),
                            [format!("$.{}", json_path)]
                            )
                        )
                },
                FieldOp::JsonStringContains(s) => {
                    Condition::all().add(sea_query::Expr::cust_with_values(
                                &format!("json_extract({}, '$') LIKE ?", <Entity as EntityTrait>::Column::#pascal_name.to_string()),
                                [format!("%{}%", s)]
                    ))
                },
                FieldOp::JsonStringStartsWith(s) => {
                    Condition::all().add(sea_query::Expr::cust_with_values(
                                &format!("json_extract({}, '$') LIKE ?", <Entity as EntityTrait>::Column::#pascal_name.to_string()),
                                [format!("{}%", s)]
                    ))
                },
                FieldOp::JsonStringEndsWith(s) => {
                    Condition::all().add(sea_query::Expr::cust_with_values(
                                &format!("json_extract({}, '$') LIKE ?", <Entity as EntityTrait>::Column::#pascal_name.to_string()),
                                [format!("%{}", s)]
                    ))
                },
                FieldOp::JsonArrayContains(val) => {
                    Condition::all().add(sea_query::Expr::cust_with_values(
                                &format!("EXISTS (SELECT 1 FROM json_each({}) WHERE value = ?)", <Entity as EntityTrait>::Column::#pascal_name.to_string()),
                                [val.to_string()]
                    ))
                },
                FieldOp::JsonArrayStartsWith(val) => {
                    Condition::all().add(sea_query::Expr::cust_with_values(
                                &format!("json_extract({}, '$[0]') = ?", <Entity as EntityTrait>::Column::#pascal_name.to_string()),
                                [val.to_string()]
                    ))
                },
                FieldOp::JsonArrayEndsWith(val) => {
                    Condition::all().add(sea_query::Expr::cust_with_values(
                                &format!("json_extract({}, '$[#-1]') = ?", <Entity as EntityTrait>::Column::#pascal_name.to_string()),
                                [val.to_string()]
                    ))
                },
                FieldOp::JsonObjectContains(key) => {
                    Condition::all().add(sea_query::Expr::cust_with_values(
                                &format!("json_extract({}, ?) IS NOT NULL", <Entity as EntityTrait>::Column::#pascal_name.to_string()),
                        [format!("$.{}", key)]
                    ))
                },
                // Catch-all for unsupported operations
                _ => panic!("Unsupported FieldOp operation for this field type"),
            }
        }
    }
}

/// Generate generic field handler for unknown types
fn generate_generic_field_handler(pascal_name: &proc_macro2::Ident) -> proc_macro2::TokenStream {
    quote! {
        WhereParam::#pascal_name(op) => match op {
            FieldOp::Equals(v) => Condition::all().add(<Entity as EntityTrait>::Column::#pascal_name.eq(v)),
            FieldOp::NotEquals(v) => Condition::all().add(<Entity as EntityTrait>::Column::#pascal_name.ne(v)),
            FieldOp::Gt(v) => Condition::all().add(<Entity as EntityTrait>::Column::#pascal_name.gt(v)),
            FieldOp::Lt(v) => Condition::all().add(<Entity as EntityTrait>::Column::#pascal_name.lt(v)),
            FieldOp::Gte(v) => Condition::all().add(<Entity as EntityTrait>::Column::#pascal_name.gte(v)),
            FieldOp::Lte(v) => Condition::all().add(<Entity as EntityTrait>::Column::#pascal_name.lte(v)),
            FieldOp::InVec(vs) => Condition::all().add(<Entity as EntityTrait>::Column::#pascal_name.is_in(vs)),
            FieldOp::IsNull => Condition::all().add(<Entity as EntityTrait>::Column::#pascal_name.is_null()),
            FieldOp::IsNotNull => Condition::all().add(<Entity as EntityTrait>::Column::#pascal_name.is_not_null()),
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
    // For all field types, generate an equals function using the actual field type
    quote! {
        pub fn equals<T: Into<#ty>>(value: T) -> WhereParam {
            WhereParam::#pascal_name(FieldOp::Equals(value.into()))
        }
    }
}

/// Generate dynamic field mappings for a target entity
/// This makes the code agnostic by generating field-to-column mapping for any entity
fn generate_target_field_mappings(
    target: &syn::Path,
    table_name: &str,
) -> Vec<proc_macro2::TokenStream> {
    // Type-safe approach: generate direct FieldOp matching for any field
    vec![quote! {
                    field_name => {
            // Type-safe field handling - direct FieldOp matching
            // This approach eliminates string parsing and is fully type-safe
            match &filter.operation {
                caustics::FieldOp::Equals(value) => {
                                        Condition::all().add(sea_query::Expr::cust_with_values(
                        &format!("\"{}\".{} = ?", #table_name, field_name),
                        [sea_orm::Value::from(value)]
                                        ))
                                    },
                caustics::FieldOp::NotEquals(value) => {
                                        Condition::all().add(sea_query::Expr::cust_with_values(
                        &format!("\"{}\".{} != ?", #table_name, field_name),
                        [sea_orm::Value::from(value)]
                                        ))
                                    },
                caustics::FieldOp::Gt(value) => {
                                        Condition::all().add(sea_query::Expr::cust_with_values(
                        &format!("\"{}\".{} > ?", #table_name, field_name),
                        [sea_orm::Value::from(value)]
                    ))
                },
                caustics::FieldOp::Lt(value) => {
                                Condition::all().add(sea_query::Expr::cust_with_values(
                        &format!("\"{}\".{} < ?", #table_name, field_name),
                        [sea_orm::Value::from(value)]
                    ))
                },
                caustics::FieldOp::Gte(value) => {
                    Condition::all().add(sea_query::Expr::cust_with_values(
                        &format!("\"{}\".{} >= ?", #table_name, field_name),
                        [sea_orm::Value::from(value)]
                    ))
                },
                caustics::FieldOp::Lte(value) => {
                    Condition::all().add(sea_query::Expr::cust_with_values(
                        &format!("\"{}\".{} <= ?", #table_name, field_name),
                        [sea_orm::Value::from(value)]
                    ))
                },
                caustics::FieldOp::Contains(value) => {
                                Condition::all().add(sea_query::Expr::cust_with_values(
                                    &format!("\"{}\".{} LIKE ?", #table_name, field_name),
                        [sea_orm::Value::from(format!("%{}%", value))]
                    ))
                },
                caustics::FieldOp::StartsWith(value) => {
                    Condition::all().add(sea_query::Expr::cust_with_values(
                        &format!("\"{}\".{} LIKE ?", #table_name, field_name),
                        [sea_orm::Value::from(format!("{}%", value))]
                    ))
                },
                caustics::FieldOp::EndsWith(value) => {
                    Condition::all().add(sea_query::Expr::cust_with_values(
                        &format!("\"{}\".{} LIKE ?", #table_name, field_name),
                        [sea_orm::Value::from(format!("%{}", value))]
                    ))
                },
                caustics::FieldOp::InVec(values) => {
                    Condition::all().add(sea_query::Expr::cust_with_values(
                        &format!("\"{}\".{} IN ({})", #table_name, field_name,
                            values.iter().map(|_| "?").collect::<Vec<_>>().join(",")),
                        values.iter().map(|v| sea_orm::Value::from(v)).collect::<Vec<_>>()
                    ))
                },
                caustics::FieldOp::NotInVec(values) => {
                    Condition::all().add(sea_query::Expr::cust_with_values(
                        &format!("\"{}\".{} NOT IN ({})", #table_name, field_name,
                            values.iter().map(|_| "?").collect::<Vec<_>>().join(",")),
                        values.iter().map(|v| sea_orm::Value::from(v)).collect::<Vec<_>>()
                    ))
                },
                caustics::FieldOp::IsNull => {
                                Condition::all().add(sea_query::Expr::cust_with_values(
                                    &format!("\"{}\".{} IS NULL", #table_name, field_name),
                                    Vec::<sea_orm::Value>::new()
                                ))
                },
                caustics::FieldOp::IsNotNull => {
                                Condition::all().add(sea_query::Expr::cust_with_values(
                                    &format!("\"{}\".{} IS NOT NULL", #table_name, field_name),
                                    Vec::<sea_orm::Value>::new()
                                ))
                },
                // JSON operations
                caustics::FieldOp::JsonPath(path) => {
                    let json_path = path.join(".");
                                Condition::all().add(sea_query::Expr::cust_with_values(
                        &format!("\"{}\".{}->'{}' IS NOT NULL", #table_name, field_name, json_path),
                        Vec::<sea_orm::Value>::new()
                                ))
                },
                caustics::FieldOp::JsonStringContains(value) => {
                                Condition::all().add(sea_query::Expr::cust_with_values(
                        &format!("\"{}\".{}::text LIKE ?", #table_name, field_name),
                        [sea_orm::Value::from(format!("%{}%", value))]
                                ))
                },
                caustics::FieldOp::JsonStringStartsWith(value) => {
                                Condition::all().add(sea_query::Expr::cust_with_values(
                        &format!("\"{}\".{}::text LIKE ?", #table_name, field_name),
                        [sea_orm::Value::from(format!("{}%", value))]
                                ))
                },
                caustics::FieldOp::JsonStringEndsWith(value) => {
                                Condition::all().add(sea_query::Expr::cust_with_values(
                        &format!("\"{}\".{}::text LIKE ?", #table_name, field_name),
                        [sea_orm::Value::from(format!("%{}", value))]
                    ))
                },
                caustics::FieldOp::JsonArrayContains(value) => {
                    Condition::all().add(sea_query::Expr::cust_with_values(
                        &format!("\"{}\".{} @> ?", #table_name, field_name),
                        [sea_orm::Value::from(serde_json::to_string(&value).unwrap())]
                    ))
                },
                caustics::FieldOp::JsonArrayStartsWith(value) => {
                    Condition::all().add(sea_query::Expr::cust_with_values(
                        &format!("\"{}\".{} @> ?", #table_name, field_name),
                        [sea_orm::Value::from(serde_json::to_string(&value).unwrap())]
                    ))
                },
                caustics::FieldOp::JsonArrayEndsWith(value) => {
                    Condition::all().add(sea_query::Expr::cust_with_values(
                        &format!("\"{}\".{} @> ?", #table_name, field_name),
                        [sea_orm::Value::from(serde_json::to_string(&value).unwrap())]
                    ))
                },
                caustics::FieldOp::JsonObjectContains(key) => {
                    Condition::all().add(sea_query::Expr::cust_with_values(
                        &format!("\"{}\".{} ? ?", #table_name, field_name),
                        [sea_orm::Value::from(key)]
                    ))
                },
                // Relation operations (these should not be used in field mappings)
                caustics::FieldOp::Some(_) | caustics::FieldOp::Every(_) | caustics::FieldOp::None(_) => {
                    panic!("Relation operations should not be used in field mappings")
                }
            }
        },
    }]
}
