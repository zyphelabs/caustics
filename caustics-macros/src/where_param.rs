// Logic for generating field variants for WhereParam enum and their filter functions.
// This module will support all types and will be long.

use heck::ToPascalCase;
use quote::{format_ident, quote};

/// Generate field variants, match arms, and field operator modules for WhereParam enum and filters.
pub fn generate_where_param_logic(
    fields: &[&syn::Field],
    unique_fields: &[&syn::Field],
    full_mod_path: &syn::Path,
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
                        super::WhereParam::#pascal_name(FieldOp::Equals(v))
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

        // String ops (only for string types)
        let string_ops = match field_type {
            FieldType::String | FieldType::OptionString => {
                quote! {
                    pub fn contains<T: Into<String>>(value: T) -> WhereParam {
                        WhereParam::#pascal_name(FieldOp::Contains(value.into()))
                    }
                    pub fn starts_with<T: Into<String>>(value: T) -> WhereParam {
                        WhereParam::#pascal_name(FieldOp::StartsWith(value.into()))
                    }
                    pub fn ends_with<T: Into<String>>(value: T) -> WhereParam {
                        WhereParam::#pascal_name(FieldOp::EndsWith(value.into()))
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
                WhereParam::#pascal_name(FieldOp::NotEquals(value.into()))
            }
            pub fn gt<T: Into<#ty>>(value: T) -> WhereParam {
                WhereParam::#pascal_name(FieldOp::Gt(value.into()))
            }
            pub fn lt<T: Into<#ty>>(value: T) -> WhereParam {
                WhereParam::#pascal_name(FieldOp::Lt(value.into()))
            }
            pub fn gte<T: Into<#ty>>(value: T) -> WhereParam {
                WhereParam::#pascal_name(FieldOp::Gte(value.into()))
            }
            pub fn lte<T: Into<#ty>>(value: T) -> WhereParam {
                WhereParam::#pascal_name(FieldOp::Lte(value.into()))
            }
            }
        } else {
            // For boolean, UUID, and JSON fields, only provide equals/not_equals
            quote! {
                pub fn not_equals<T: Into<#ty>>(value: T) -> WhereParam {
                    WhereParam::#pascal_name(FieldOp::NotEquals(value.into()))
                }
            }
        };

        // Collection operations (for all types)
        let collection_ops = quote! {
            pub fn in_vec<T: Into<#ty>>(values: Vec<T>) -> WhereParam {
                WhereParam::#pascal_name(FieldOp::InVec(values.into_iter().map(|v| v.into()).collect()))
            }
            pub fn not_in_vec<T: Into<#ty>>(values: Vec<T>) -> WhereParam {
                WhereParam::#pascal_name(FieldOp::NotInVec(values.into_iter().map(|v| v.into()).collect()))
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
                        WhereParam::#pascal_name(FieldOp::IsNull)
                    }
                    pub fn is_not_null() -> WhereParam {
                        WhereParam::#pascal_name(FieldOp::IsNotNull)
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
                        WhereParam::#pascal_name(FieldOp::JsonPath(path))
                    }
                    pub fn json_string_contains(value: String) -> WhereParam {
                        WhereParam::#pascal_name(FieldOp::JsonStringContains(value))
                    }
                    pub fn json_string_starts_with(value: String) -> WhereParam {
                        WhereParam::#pascal_name(FieldOp::JsonStringStartsWith(value))
                    }
                    pub fn json_string_ends_with(value: String) -> WhereParam {
                        WhereParam::#pascal_name(FieldOp::JsonStringEndsWith(value))
                    }
                    pub fn json_array_contains(value: serde_json::Value) -> WhereParam {
                        WhereParam::#pascal_name(FieldOp::JsonArrayContains(value))
                    }
                    pub fn json_array_starts_with(value: serde_json::Value) -> WhereParam {
                        WhereParam::#pascal_name(FieldOp::JsonArrayStartsWith(value))
                    }
                    pub fn json_array_ends_with(value: serde_json::Value) -> WhereParam {
                        WhereParam::#pascal_name(FieldOp::JsonArrayEndsWith(value))
                    }
                    pub fn json_object_contains(key: String) -> WhereParam {
                        WhereParam::#pascal_name(FieldOp::JsonObjectContains(key))
                    }
                }
            }
            _ => quote! {},
        };

        let mut field_mod_items = vec![
            set_fn,
            unique_where_fn,
            order_fn,
            type_specific_ops,
            string_ops,
            comparison_ops,
            collection_ops,
            null_ops,
            json_ops,
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
            #[allow(dead_code)]
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

    // Generate a function that processes all WhereParams together, properly handling QueryMode
    let where_params_to_condition_fn = generate_where_params_to_condition_function(&fields);

    let where_match_arms: Vec<proc_macro2::TokenStream> = vec![where_params_to_condition_fn];
    (where_field_variants, where_match_arms, field_ops)
}

/// Generate a function that converts Vec<WhereParam> to Condition, properly handling QueryMode
fn generate_where_params_to_condition_function(fields: &[&syn::Field]) -> proc_macro2::TokenStream {
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
                field_handlers.push(generate_string_field_handler(&pascal_name, &name, false));
                mode_handlers.push(generate_mode_handler(&pascal_name, &name));
            }
            FieldType::OptionString => {
                field_handlers.push(generate_string_field_handler(&pascal_name, &name, true));
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

    quote! {
        /// Convert a vector of WhereParams to a SeaORM Condition, properly handling QueryMode
        pub fn where_params_to_condition(params: Vec<WhereParam>) -> sea_query::Condition {
            use std::collections::HashMap;
            use sea_orm::{EntityTrait, ColumnTrait};
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
                            cond = cond.add(where_params_to_condition(vec![p]));
                        }
                        cond
                    },
                    WhereParam::Or(params) => {
                        let mut cond = Condition::any();
                        for p in params {
                            cond = cond.add(where_params_to_condition(vec![p]));
                        }
                        cond
                    },
                    WhereParam::Not(params) => {
                        let mut cond = Condition::all();
                        for p in params {
                            cond = cond.add(where_params_to_condition(vec![p]));
                        }
                        cond.not()
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
enum FieldType {
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
fn detect_field_type(ty: &syn::Type) -> FieldType {
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

/// Generate string field handler with QueryMode support
fn generate_string_field_handler(
    pascal_name: &proc_macro2::Ident,
    name: &syn::Ident,
    is_nullable: bool,
) -> proc_macro2::TokenStream {
    if is_nullable {
        quote! {
            WhereParam::#pascal_name(op) => {
                let query_mode = query_modes.get(stringify!(#name)).copied().unwrap_or(caustics::QueryMode::Default);
                match op {
                    FieldOp::Equals(v) => {
                        match v {
                            Some(val) => {
                                if query_mode == caustics::QueryMode::Insensitive {
                                    Condition::all().add(<Entity as EntityTrait>::Column::#pascal_name.like(val))
                                } else {
                                    Condition::all().add(<Entity as EntityTrait>::Column::#pascal_name.eq(val))
                                }
                            },
                            None => Condition::all().add(<Entity as EntityTrait>::Column::#pascal_name.is_null()),
                        }
                    },
                    FieldOp::NotEquals(v) => {
                        match v {
                            Some(val) => {
                                if query_mode == caustics::QueryMode::Insensitive {
                                    Condition::all().add(<Entity as EntityTrait>::Column::#pascal_name.not_like(val))
                                } else {
                                    Condition::all().add(<Entity as EntityTrait>::Column::#pascal_name.ne(val))
                                }
                            },
                            None => Condition::all().add(<Entity as EntityTrait>::Column::#pascal_name.is_not_null()),
                        }
                    },
                    FieldOp::Contains(s) => {
                        if query_mode == caustics::QueryMode::Insensitive {
                            Condition::all().add(<Entity as EntityTrait>::Column::#pascal_name.like(format!("%{}%", s)))
                        } else {
                            Condition::all().add(<Entity as EntityTrait>::Column::#pascal_name.contains(s))
                        }
                    },
                    FieldOp::StartsWith(s) => {
                        if query_mode == caustics::QueryMode::Insensitive {
                            Condition::all().add(<Entity as EntityTrait>::Column::#pascal_name.like(format!("{}%", s)))
                        } else {
                            Condition::all().add(<Entity as EntityTrait>::Column::#pascal_name.starts_with(s))
                        }
                    },
                    FieldOp::EndsWith(s) => {
                        if query_mode == caustics::QueryMode::Insensitive {
                            Condition::all().add(<Entity as EntityTrait>::Column::#pascal_name.like(format!("%{}", s)))
                        } else {
                            Condition::all().add(<Entity as EntityTrait>::Column::#pascal_name.ends_with(s))
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
                    // JSON-specific operations (not supported for regular string fields)
                    FieldOp::JsonPath(_) => panic!("JsonPath operation only supported for JSON fields"),
                    FieldOp::JsonStringContains(_) => panic!("JsonStringContains operation only supported for JSON fields"),
                    FieldOp::JsonStringStartsWith(_) => panic!("JsonStringStartsWith operation only supported for JSON fields"),
                    FieldOp::JsonStringEndsWith(_) => panic!("JsonStringEndsWith operation only supported for JSON fields"),
                    FieldOp::JsonArrayContains(_) => panic!("JsonArrayContains operation only supported for JSON fields"),
                    FieldOp::JsonArrayStartsWith(_) => panic!("JsonArrayStartsWith operation only supported for JSON fields"),
                    FieldOp::JsonArrayEndsWith(_) => panic!("JsonArrayEndsWith operation only supported for JSON fields"),
                    FieldOp::JsonObjectContains(_) => panic!("JsonObjectContains operation only supported for JSON fields"),
                }
            }
        }
    } else {
        quote! {
            WhereParam::#pascal_name(op) => {
                let query_mode = query_modes.get(stringify!(#name)).copied().unwrap_or(caustics::QueryMode::Default);
                match op {
                    FieldOp::Equals(v) => {
                        if query_mode == caustics::QueryMode::Insensitive {
                            Condition::all().add(<Entity as EntityTrait>::Column::#pascal_name.like(v))
                        } else {
                            Condition::all().add(<Entity as EntityTrait>::Column::#pascal_name.eq(v))
                        }
                    },
                    FieldOp::NotEquals(v) => {
                        if query_mode == caustics::QueryMode::Insensitive {
                            Condition::all().add(<Entity as EntityTrait>::Column::#pascal_name.not_like(v))
                        } else {
                            Condition::all().add(<Entity as EntityTrait>::Column::#pascal_name.ne(v))
                        }
                    },
                    FieldOp::Contains(s) => {
                        if query_mode == caustics::QueryMode::Insensitive {
                            Condition::all().add(<Entity as EntityTrait>::Column::#pascal_name.like(format!("%{}%", s)))
                        } else {
                            Condition::all().add(<Entity as EntityTrait>::Column::#pascal_name.contains(s))
                        }
                    },
                    FieldOp::StartsWith(s) => {
                        if query_mode == caustics::QueryMode::Insensitive {
                            Condition::all().add(<Entity as EntityTrait>::Column::#pascal_name.like(format!("{}%", s)))
                        } else {
                            Condition::all().add(<Entity as EntityTrait>::Column::#pascal_name.starts_with(s))
                        }
                    },
                    FieldOp::EndsWith(s) => {
                        if query_mode == caustics::QueryMode::Insensitive {
                            Condition::all().add(<Entity as EntityTrait>::Column::#pascal_name.like(format!("%{}", s)))
                        } else {
                            Condition::all().add(<Entity as EntityTrait>::Column::#pascal_name.ends_with(s))
                        }
                    },
                    FieldOp::Gt(v) => Condition::all().add(<Entity as EntityTrait>::Column::#pascal_name.gt(v)),
                    FieldOp::Lt(v) => Condition::all().add(<Entity as EntityTrait>::Column::#pascal_name.lt(v)),
                    FieldOp::Gte(v) => Condition::all().add(<Entity as EntityTrait>::Column::#pascal_name.gte(v)),
                    FieldOp::Lte(v) => Condition::all().add(<Entity as EntityTrait>::Column::#pascal_name.lte(v)),
                    FieldOp::InVec(vs) => Condition::all().add(<Entity as EntityTrait>::Column::#pascal_name.is_in(vs)),
                    FieldOp::NotInVec(vs) => Condition::all().add(<Entity as EntityTrait>::Column::#pascal_name.is_not_in(vs)),
                    FieldOp::IsNull => panic!("IsNull operation not supported for non-nullable fields"),
                    FieldOp::IsNotNull => panic!("IsNotNull operation not supported for non-nullable fields"),
                    // JSON-specific operations (not supported for regular string fields)
                    FieldOp::JsonPath(_) => panic!("JsonPath operation only supported for JSON fields"),
                    FieldOp::JsonStringContains(_) => panic!("JsonStringContains operation only supported for JSON fields"),
                    FieldOp::JsonStringStartsWith(_) => panic!("JsonStringStartsWith operation only supported for JSON fields"),
                    FieldOp::JsonStringEndsWith(_) => panic!("JsonStringEndsWith operation only supported for JSON fields"),
                    FieldOp::JsonArrayContains(_) => panic!("JsonArrayContains operation only supported for JSON fields"),
                    FieldOp::JsonArrayStartsWith(_) => panic!("JsonArrayStartsWith operation only supported for JSON fields"),
                    FieldOp::JsonArrayEndsWith(_) => panic!("JsonArrayEndsWith operation only supported for JSON fields"),
                    FieldOp::JsonObjectContains(_) => panic!("JsonObjectContains operation only supported for JSON fields"),
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
                FieldOp::Contains(_) => panic!("Contains operation not supported for numeric fields"),
                FieldOp::StartsWith(_) => panic!("StartsWith operation not supported for numeric fields"),
                FieldOp::EndsWith(_) => panic!("EndsWith operation not supported for numeric fields"),
                FieldOp::IsNull => Condition::all().add(<Entity as EntityTrait>::Column::#pascal_name.is_null()),
                FieldOp::IsNotNull => Condition::all().add(<Entity as EntityTrait>::Column::#pascal_name.is_not_null()),
                // JSON-specific operations (not supported for numeric fields)
                FieldOp::JsonPath(_) => panic!("JsonPath operation only supported for JSON fields"),
                FieldOp::JsonStringContains(_) => panic!("JsonStringContains operation only supported for JSON fields"),
                FieldOp::JsonStringStartsWith(_) => panic!("JsonStringStartsWith operation only supported for JSON fields"),
                FieldOp::JsonStringEndsWith(_) => panic!("JsonStringEndsWith operation only supported for JSON fields"),
                FieldOp::JsonArrayContains(_) => panic!("JsonArrayContains operation only supported for JSON fields"),
                FieldOp::JsonArrayStartsWith(_) => panic!("JsonArrayStartsWith operation only supported for JSON fields"),
                FieldOp::JsonArrayEndsWith(_) => panic!("JsonArrayEndsWith operation only supported for JSON fields"),
                FieldOp::JsonObjectContains(_) => panic!("JsonObjectContains operation only supported for JSON fields"),
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
                FieldOp::Contains(_) => panic!("Contains operation not supported for numeric fields"),
                FieldOp::StartsWith(_) => panic!("StartsWith operation not supported for numeric fields"),
                FieldOp::EndsWith(_) => panic!("EndsWith operation not supported for numeric fields"),
                FieldOp::IsNull => panic!("IsNull operation not supported for non-nullable fields"),
                FieldOp::IsNotNull => panic!("IsNotNull operation not supported for non-nullable fields"),
                // JSON-specific operations (not supported for numeric fields)
                FieldOp::JsonPath(_) => panic!("JsonPath operation only supported for JSON fields"),
                FieldOp::JsonStringContains(_) => panic!("JsonStringContains operation only supported for JSON fields"),
                FieldOp::JsonStringStartsWith(_) => panic!("JsonStringStartsWith operation only supported for JSON fields"),
                FieldOp::JsonStringEndsWith(_) => panic!("JsonStringEndsWith operation only supported for JSON fields"),
                FieldOp::JsonArrayContains(_) => panic!("JsonArrayContains operation only supported for JSON fields"),
                FieldOp::JsonArrayStartsWith(_) => panic!("JsonArrayStartsWith operation only supported for JSON fields"),
                FieldOp::JsonArrayEndsWith(_) => panic!("JsonArrayEndsWith operation only supported for JSON fields"),
                FieldOp::JsonObjectContains(_) => panic!("JsonObjectContains operation only supported for JSON fields"),
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
                FieldOp::Gt(_) => panic!("Gt operation not supported for boolean fields"),
                FieldOp::Lt(_) => panic!("Lt operation not supported for boolean fields"),
                FieldOp::Gte(_) => panic!("Gte operation not supported for boolean fields"),
                FieldOp::Lte(_) => panic!("Lte operation not supported for boolean fields"),
                FieldOp::Contains(_) => panic!("Contains operation not supported for boolean fields"),
                FieldOp::StartsWith(_) => panic!("StartsWith operation not supported for boolean fields"),
                FieldOp::EndsWith(_) => panic!("EndsWith operation not supported for boolean fields"),
                FieldOp::IsNull => Condition::all().add(<Entity as EntityTrait>::Column::#pascal_name.is_null()),
                FieldOp::IsNotNull => Condition::all().add(<Entity as EntityTrait>::Column::#pascal_name.is_not_null()),
                // JSON-specific operations (not supported for boolean fields)
                FieldOp::JsonPath(_) => panic!("JsonPath operation only supported for JSON fields"),
                FieldOp::JsonStringContains(_) => panic!("JsonStringContains operation only supported for JSON fields"),
                FieldOp::JsonStringStartsWith(_) => panic!("JsonStringStartsWith operation only supported for JSON fields"),
                FieldOp::JsonStringEndsWith(_) => panic!("JsonStringEndsWith operation only supported for JSON fields"),
                FieldOp::JsonArrayContains(_) => panic!("JsonArrayContains operation only supported for JSON fields"),
                FieldOp::JsonArrayStartsWith(_) => panic!("JsonArrayStartsWith operation only supported for JSON fields"),
                FieldOp::JsonArrayEndsWith(_) => panic!("JsonArrayEndsWith operation only supported for JSON fields"),
                FieldOp::JsonObjectContains(_) => panic!("JsonObjectContains operation only supported for JSON fields"),
            }
        }
    } else {
        quote! {
            WhereParam::#pascal_name(op) => match op {
                FieldOp::Equals(v) => Condition::all().add(<Entity as EntityTrait>::Column::#pascal_name.eq(v)),
                FieldOp::NotEquals(v) => Condition::all().add(<Entity as EntityTrait>::Column::#pascal_name.ne(v)),
                FieldOp::InVec(vs) => Condition::all().add(<Entity as EntityTrait>::Column::#pascal_name.is_in(vs)),
                FieldOp::NotInVec(vs) => Condition::all().add(<Entity as EntityTrait>::Column::#pascal_name.is_not_in(vs)),
                FieldOp::Gt(_) => panic!("Gt operation not supported for boolean fields"),
                FieldOp::Lt(_) => panic!("Lt operation not supported for boolean fields"),
                FieldOp::Gte(_) => panic!("Gte operation not supported for boolean fields"),
                FieldOp::Lte(_) => panic!("Lte operation not supported for boolean fields"),
                FieldOp::Contains(_) => panic!("Contains operation not supported for boolean fields"),
                FieldOp::StartsWith(_) => panic!("StartsWith operation not supported for boolean fields"),
                FieldOp::EndsWith(_) => panic!("EndsWith operation not supported for boolean fields"),
                FieldOp::IsNull => panic!("IsNull operation not supported for non-nullable fields"),
                FieldOp::IsNotNull => panic!("IsNotNull operation not supported for non-nullable fields"),
                // JSON-specific operations (not supported for boolean fields)
                FieldOp::JsonPath(_) => panic!("JsonPath operation only supported for JSON fields"),
                FieldOp::JsonStringContains(_) => panic!("JsonStringContains operation only supported for JSON fields"),
                FieldOp::JsonStringStartsWith(_) => panic!("JsonStringStartsWith operation only supported for JSON fields"),
                FieldOp::JsonStringEndsWith(_) => panic!("JsonStringEndsWith operation only supported for JSON fields"),
                FieldOp::JsonArrayContains(_) => panic!("JsonArrayContains operation only supported for JSON fields"),
                FieldOp::JsonArrayStartsWith(_) => panic!("JsonArrayStartsWith operation only supported for JSON fields"),
                FieldOp::JsonArrayEndsWith(_) => panic!("JsonArrayEndsWith operation only supported for JSON fields"),
                FieldOp::JsonObjectContains(_) => panic!("JsonObjectContains operation only supported for JSON fields"),
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
                FieldOp::Contains(_) => panic!("Contains operation not supported for DateTime fields"),
                FieldOp::StartsWith(_) => panic!("StartsWith operation not supported for DateTime fields"),
                FieldOp::EndsWith(_) => panic!("EndsWith operation not supported for DateTime fields"),
                FieldOp::IsNull => Condition::all().add(<Entity as EntityTrait>::Column::#pascal_name.is_null()),
                FieldOp::IsNotNull => Condition::all().add(<Entity as EntityTrait>::Column::#pascal_name.is_not_null()),
                // JSON-specific operations (not supported for DateTime fields)
                FieldOp::JsonPath(_) => panic!("JsonPath operation only supported for JSON fields"),
                FieldOp::JsonStringContains(_) => panic!("JsonStringContains operation only supported for JSON fields"),
                FieldOp::JsonStringStartsWith(_) => panic!("JsonStringStartsWith operation only supported for JSON fields"),
                FieldOp::JsonStringEndsWith(_) => panic!("JsonStringEndsWith operation only supported for JSON fields"),
                FieldOp::JsonArrayContains(_) => panic!("JsonArrayContains operation only supported for JSON fields"),
                FieldOp::JsonArrayStartsWith(_) => panic!("JsonArrayStartsWith operation only supported for JSON fields"),
                FieldOp::JsonArrayEndsWith(_) => panic!("JsonArrayEndsWith operation only supported for JSON fields"),
                FieldOp::JsonObjectContains(_) => panic!("JsonObjectContains operation only supported for JSON fields"),
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
                FieldOp::Contains(_) => panic!("Contains operation not supported for DateTime fields"),
                FieldOp::StartsWith(_) => panic!("StartsWith operation not supported for DateTime fields"),
                FieldOp::EndsWith(_) => panic!("EndsWith operation not supported for DateTime fields"),
                FieldOp::IsNull => panic!("IsNull operation not supported for non-nullable fields"),
                FieldOp::IsNotNull => panic!("IsNotNull operation not supported for non-nullable fields"),
                // JSON-specific operations (not supported for DateTime fields)
                FieldOp::JsonPath(_) => panic!("JsonPath operation only supported for JSON fields"),
                FieldOp::JsonStringContains(_) => panic!("JsonStringContains operation only supported for JSON fields"),
                FieldOp::JsonStringStartsWith(_) => panic!("JsonStringStartsWith operation only supported for JSON fields"),
                FieldOp::JsonStringEndsWith(_) => panic!("JsonStringEndsWith operation only supported for JSON fields"),
                FieldOp::JsonArrayContains(_) => panic!("JsonArrayContains operation only supported for JSON fields"),
                FieldOp::JsonArrayStartsWith(_) => panic!("JsonArrayStartsWith operation only supported for JSON fields"),
                FieldOp::JsonArrayEndsWith(_) => panic!("JsonArrayEndsWith operation only supported for JSON fields"),
                FieldOp::JsonObjectContains(_) => panic!("JsonObjectContains operation only supported for JSON fields"),
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
                FieldOp::Gt(_) => panic!("Gt operation not supported for UUID fields"),
                FieldOp::Lt(_) => panic!("Lt operation not supported for UUID fields"),
                FieldOp::Gte(_) => panic!("Gte operation not supported for UUID fields"),
                FieldOp::Lte(_) => panic!("Lte operation not supported for UUID fields"),
                FieldOp::Contains(_) => panic!("Contains operation not supported for UUID fields"),
                FieldOp::StartsWith(_) => panic!("StartsWith operation not supported for UUID fields"),
                FieldOp::EndsWith(_) => panic!("EndsWith operation not supported for UUID fields"),
                FieldOp::IsNull => Condition::all().add(<Entity as EntityTrait>::Column::#pascal_name.is_null()),
                FieldOp::IsNotNull => Condition::all().add(<Entity as EntityTrait>::Column::#pascal_name.is_not_null()),
                // JSON-specific operations (not supported for UUID fields)
                FieldOp::JsonPath(_) => panic!("JsonPath operation only supported for JSON fields"),
                FieldOp::JsonStringContains(_) => panic!("JsonStringContains operation only supported for JSON fields"),
                FieldOp::JsonStringStartsWith(_) => panic!("JsonStringStartsWith operation only supported for JSON fields"),
                FieldOp::JsonStringEndsWith(_) => panic!("JsonStringEndsWith operation only supported for JSON fields"),
                FieldOp::JsonArrayContains(_) => panic!("JsonArrayContains operation only supported for JSON fields"),
                FieldOp::JsonArrayStartsWith(_) => panic!("JsonArrayStartsWith operation only supported for JSON fields"),
                FieldOp::JsonArrayEndsWith(_) => panic!("JsonArrayEndsWith operation only supported for JSON fields"),
                FieldOp::JsonObjectContains(_) => panic!("JsonObjectContains operation only supported for JSON fields"),
            }
        }
    } else {
        quote! {
            WhereParam::#pascal_name(op) => match op {
                FieldOp::Equals(v) => Condition::all().add(<Entity as EntityTrait>::Column::#pascal_name.eq(v)),
                FieldOp::NotEquals(v) => Condition::all().add(<Entity as EntityTrait>::Column::#pascal_name.ne(v)),
                FieldOp::InVec(vs) => Condition::all().add(<Entity as EntityTrait>::Column::#pascal_name.is_in(vs)),
                FieldOp::NotInVec(vs) => Condition::all().add(<Entity as EntityTrait>::Column::#pascal_name.is_not_in(vs)),
                FieldOp::Gt(_) => panic!("Gt operation not supported for UUID fields"),
                FieldOp::Lt(_) => panic!("Lt operation not supported for UUID fields"),
                FieldOp::Gte(_) => panic!("Gte operation not supported for UUID fields"),
                FieldOp::Lte(_) => panic!("Lte operation not supported for UUID fields"),
                FieldOp::Contains(_) => panic!("Contains operation not supported for UUID fields"),
                FieldOp::StartsWith(_) => panic!("StartsWith operation not supported for UUID fields"),
                FieldOp::EndsWith(_) => panic!("EndsWith operation not supported for UUID fields"),
                FieldOp::IsNull => panic!("IsNull operation not supported for non-nullable fields"),
                FieldOp::IsNotNull => panic!("IsNotNull operation not supported for non-nullable fields"),
                // JSON-specific operations (not supported for UUID fields)
                FieldOp::JsonPath(_) => panic!("JsonPath operation only supported for JSON fields"),
                FieldOp::JsonStringContains(_) => panic!("JsonStringContains operation only supported for JSON fields"),
                FieldOp::JsonStringStartsWith(_) => panic!("JsonStringStartsWith operation only supported for JSON fields"),
                FieldOp::JsonStringEndsWith(_) => panic!("JsonStringEndsWith operation only supported for JSON fields"),
                FieldOp::JsonArrayContains(_) => panic!("JsonArrayContains operation only supported for JSON fields"),
                FieldOp::JsonArrayStartsWith(_) => panic!("JsonArrayStartsWith operation only supported for JSON fields"),
                FieldOp::JsonArrayEndsWith(_) => panic!("JsonArrayEndsWith operation only supported for JSON fields"),
                FieldOp::JsonObjectContains(_) => panic!("JsonObjectContains operation only supported for JSON fields"),
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
                FieldOp::InVec(vs) => Condition::all().add(<Entity as EntityTrait>::Column::#pascal_name.is_in(vs)),
                FieldOp::NotInVec(vs) => Condition::all().add(<Entity as EntityTrait>::Column::#pascal_name.is_not_in(vs)),
                FieldOp::Gt(v) => Condition::all().add(<Entity as EntityTrait>::Column::#pascal_name.gt(v)),
                FieldOp::Lt(v) => Condition::all().add(<Entity as EntityTrait>::Column::#pascal_name.lt(v)),
                FieldOp::Gte(v) => Condition::all().add(<Entity as EntityTrait>::Column::#pascal_name.gte(v)),
                FieldOp::Lte(v) => Condition::all().add(<Entity as EntityTrait>::Column::#pascal_name.lte(v)),
                FieldOp::IsNull => Condition::all().add(<Entity as EntityTrait>::Column::#pascal_name.is_null()),
                FieldOp::IsNotNull => Condition::all().add(<Entity as EntityTrait>::Column::#pascal_name.is_not_null()),
                // JSON operations with database detection
                FieldOp::JsonPath(path) => {
                    let column_name = <Entity as EntityTrait>::Column::#pascal_name.to_string();
                    // Runtime database detection
                    if std::env::var("DATABASE_URL").unwrap_or_default().starts_with("postgres") {
                        // PostgreSQL: column #> '{path,to,key}' IS NOT NULL
                        let path_array = format!("{{{}}}", path.join(","));
                        Condition::all().add(
                            sea_query::Expr::cust_with_values(
                                &format!("{} #> ? IS NOT NULL", column_name),
                                [path_array]
                            )
                        )
                    } else {
                        // SQLite: json_extract(column, '$.path.to.key') IS NOT NULL
                        let json_path = format!("$.{}", path.join("."));
                        Condition::all().add(
                            sea_query::Expr::cust_with_values(
                                &format!("json_extract({}, ?) IS NOT NULL", column_name),
                                [json_path]
                            )
                        )
                    }
                },
                FieldOp::JsonStringContains(s) => {
                    let column_name = <Entity as EntityTrait>::Column::#pascal_name.to_string();
                    if std::env::var("DATABASE_URL").unwrap_or_default().starts_with("postgres") {
                        // PostgreSQL: column ->> '$' LIKE '%search%'
                        Condition::all().add(
                            sea_query::Expr::cust_with_values(
                                &format!("({} ->> '$') LIKE ?", column_name),
                                [format!("%{}%", s)]
                            )
                        )
                    } else {
                        // SQLite: json_extract(column, '$') LIKE '%search%'
                        Condition::all().add(
                            sea_query::Expr::cust_with_values(
                                &format!("json_extract({}, '$') LIKE ?", column_name),
                                [format!("%{}%", s)]
                            )
                        )
                    }
                },
                FieldOp::JsonStringStartsWith(s) => {
                    let column_name = <Entity as EntityTrait>::Column::#pascal_name.to_string();
                    if std::env::var("DATABASE_URL").unwrap_or_default().starts_with("postgres") {
                        Condition::all().add(
                            sea_query::Expr::cust_with_values(
                                &format!("({} ->> '$') LIKE ?", column_name),
                                [format!("{}%", s)]
                            )
                        )
                    } else {
                        Condition::all().add(
                            sea_query::Expr::cust_with_values(
                                &format!("json_extract({}, '$') LIKE ?", column_name),
                                [format!("{}%", s)]
                            )
                        )
                    }
                },
                FieldOp::JsonStringEndsWith(s) => {
                    let column_name = <Entity as EntityTrait>::Column::#pascal_name.to_string();
                    if std::env::var("DATABASE_URL").unwrap_or_default().starts_with("postgres") {
                        Condition::all().add(
                            sea_query::Expr::cust_with_values(
                                &format!("({} ->> '$') LIKE ?", column_name),
                                [format!("%{}", s)]
                            )
                        )
                    } else {
                        Condition::all().add(
                            sea_query::Expr::cust_with_values(
                                &format!("json_extract({}, '$') LIKE ?", column_name),
                                [format!("%{}", s)]
                            )
                        )
                    }
                },
                FieldOp::JsonArrayContains(val) => {
                    let column_name = <Entity as EntityTrait>::Column::#pascal_name.to_string();
                    if std::env::var("DATABASE_URL").unwrap_or_default().starts_with("postgres") {
                        // PostgreSQL: column @> '[value]'
                        Condition::all().add(
                            sea_query::Expr::cust_with_values(
                                &format!("{} @> ?", column_name),
                                [format!("[{}]", val.to_string())]
                            )
                        )
                    } else {
                        // SQLite: json_each approach
                        Condition::all().add(
                            sea_query::Expr::cust_with_values(
                                &format!("EXISTS (SELECT 1 FROM json_each({}) WHERE value = ?)", column_name),
                                [val.to_string()]
                            )
                        )
                    }
                },
                FieldOp::JsonArrayStartsWith(val) => {
                    let column_name = <Entity as EntityTrait>::Column::#pascal_name.to_string();
                    if std::env::var("DATABASE_URL").unwrap_or_default().starts_with("postgres") {
                        Condition::all().add(
                            sea_query::Expr::cust_with_values(
                                &format!("{} -> 0 = ?", column_name),
                                [val.to_string()]
                            )
                        )
                    } else {
                        Condition::all().add(
                            sea_query::Expr::cust_with_values(
                                &format!("json_extract({}, '$[0]') = ?", column_name),
                                [val.to_string()]
                            )
                        )
                    }
                },
                FieldOp::JsonArrayEndsWith(val) => {
                    let column_name = <Entity as EntityTrait>::Column::#pascal_name.to_string();
                    if std::env::var("DATABASE_URL").unwrap_or_default().starts_with("postgres") {
                        Condition::all().add(
                            sea_query::Expr::cust_with_values(
                                &format!("{} -> -1 = ?", column_name),
                                [val.to_string()]
                            )
                        )
                    } else {
                        Condition::all().add(
                            sea_query::Expr::cust_with_values(
                                &format!("json_extract({}, '$[#-1]') = ?", column_name),
                                [val.to_string()]
                            )
                        )
                    }
                },
                FieldOp::JsonObjectContains(key) => {
                    let column_name = <Entity as EntityTrait>::Column::#pascal_name.to_string();
                    if std::env::var("DATABASE_URL").unwrap_or_default().starts_with("postgres") {
                        // PostgreSQL: column ? 'key'
                        Condition::all().add(
                            sea_query::Expr::cust_with_values(
                                &format!("{} ? ?", column_name),
                                [key]
                            )
                        )
                    } else {
                        // SQLite: json_extract(column, '$.key') IS NOT NULL
                        Condition::all().add(
                            sea_query::Expr::cust_with_values(
                                &format!("json_extract({}, ?) IS NOT NULL", column_name),
                                [format!("$.{}", key)]
                            )
                        )
                    }
                },
                // String operations (not supported for JSON fields)
                FieldOp::Contains(_) => panic!("Use JsonStringContains for JSON string operations"),
                FieldOp::StartsWith(_) => panic!("Use JsonStringStartsWith for JSON string operations"),
                FieldOp::EndsWith(_) => panic!("Use JsonStringEndsWith for JSON string operations"),
            }
        }
    } else {
        quote! {
            WhereParam::#pascal_name(op) => match op {
                FieldOp::Equals(v) => Condition::all().add(<Entity as EntityTrait>::Column::#pascal_name.eq(v)),
                FieldOp::NotEquals(v) => Condition::all().add(<Entity as EntityTrait>::Column::#pascal_name.ne(v)),
                FieldOp::InVec(vs) => Condition::all().add(<Entity as EntityTrait>::Column::#pascal_name.is_in(vs)),
                FieldOp::NotInVec(vs) => Condition::all().add(<Entity as EntityTrait>::Column::#pascal_name.is_not_in(vs)),
                FieldOp::Gt(v) => Condition::all().add(<Entity as EntityTrait>::Column::#pascal_name.gt(v)),
                FieldOp::Lt(v) => Condition::all().add(<Entity as EntityTrait>::Column::#pascal_name.lt(v)),
                FieldOp::Gte(v) => Condition::all().add(<Entity as EntityTrait>::Column::#pascal_name.gte(v)),
                FieldOp::Lte(v) => Condition::all().add(<Entity as EntityTrait>::Column::#pascal_name.lte(v)),
                // JSON operations with database detection
                FieldOp::JsonPath(path) => {
                    let column_name = <Entity as EntityTrait>::Column::#pascal_name.to_string();
                    if std::env::var("DATABASE_URL").unwrap_or_default().starts_with("postgres") {
                        let path_array = format!("{{{}}}", path.join(","));
                        Condition::all().add(
                            sea_query::Expr::cust_with_values(
                                &format!("{} #> ? IS NOT NULL", column_name),
                                [path_array]
                            )
                        )
                    } else {
                        let json_path = format!("$.{}", path.join("."));
                        Condition::all().add(
                            sea_query::Expr::cust_with_values(
                                &format!("json_extract({}, ?) IS NOT NULL", column_name),
                                [json_path]
                            )
                        )
                    }
                },
                FieldOp::JsonStringContains(s) => {
                    let column_name = <Entity as EntityTrait>::Column::#pascal_name.to_string();
                    if std::env::var("DATABASE_URL").unwrap_or_default().starts_with("postgres") {
                        Condition::all().add(
                            sea_query::Expr::cust_with_values(
                                &format!("({} ->> '$') LIKE ?", column_name),
                                [format!("%{}%", s)]
                            )
                        )
                    } else {
                        Condition::all().add(
                            sea_query::Expr::cust_with_values(
                                &format!("json_extract({}, '$') LIKE ?", column_name),
                                [format!("%{}%", s)]
                            )
                        )
                    }
                },
                FieldOp::JsonStringStartsWith(s) => {
                    let column_name = <Entity as EntityTrait>::Column::#pascal_name.to_string();
                    if std::env::var("DATABASE_URL").unwrap_or_default().starts_with("postgres") {
                        Condition::all().add(
                            sea_query::Expr::cust_with_values(
                                &format!("({} ->> '$') LIKE ?", column_name),
                                [format!("{}%", s)]
                            )
                        )
                    } else {
                        Condition::all().add(
                            sea_query::Expr::cust_with_values(
                                &format!("json_extract({}, '$') LIKE ?", column_name),
                                [format!("{}%", s)]
                            )
                        )
                    }
                },
                FieldOp::JsonStringEndsWith(s) => {
                    let column_name = <Entity as EntityTrait>::Column::#pascal_name.to_string();
                    if std::env::var("DATABASE_URL").unwrap_or_default().starts_with("postgres") {
                        Condition::all().add(
                            sea_query::Expr::cust_with_values(
                                &format!("({} ->> '$') LIKE ?", column_name),
                                [format!("%{}", s)]
                            )
                        )
                    } else {
                        Condition::all().add(
                            sea_query::Expr::cust_with_values(
                                &format!("json_extract({}, '$') LIKE ?", column_name),
                                [format!("%{}", s)]
                            )
                        )
                    }
                },
                FieldOp::JsonArrayContains(val) => {
                    let column_name = <Entity as EntityTrait>::Column::#pascal_name.to_string();
                    if std::env::var("DATABASE_URL").unwrap_or_default().starts_with("postgres") {
                        Condition::all().add(
                            sea_query::Expr::cust_with_values(
                                &format!("{} @> ?", column_name),
                                [format!("[{}]", val.to_string())]
                            )
                        )
                    } else {
                        Condition::all().add(
                            sea_query::Expr::cust_with_values(
                                &format!("EXISTS (SELECT 1 FROM json_each({}) WHERE value = ?)", column_name),
                                [val.to_string()]
                            )
                        )
                    }
                },
                FieldOp::JsonArrayStartsWith(val) => {
                    let column_name = <Entity as EntityTrait>::Column::#pascal_name.to_string();
                    if std::env::var("DATABASE_URL").unwrap_or_default().starts_with("postgres") {
                        Condition::all().add(
                            sea_query::Expr::cust_with_values(
                                &format!("{} -> 0 = ?", column_name),
                                [val.to_string()]
                            )
                        )
                    } else {
                        Condition::all().add(
                            sea_query::Expr::cust_with_values(
                                &format!("json_extract({}, '$[0]') = ?", column_name),
                                [val.to_string()]
                            )
                        )
                    }
                },
                FieldOp::JsonArrayEndsWith(val) => {
                    let column_name = <Entity as EntityTrait>::Column::#pascal_name.to_string();
                    if std::env::var("DATABASE_URL").unwrap_or_default().starts_with("postgres") {
                        Condition::all().add(
                            sea_query::Expr::cust_with_values(
                                &format!("{} -> -1 = ?", column_name),
                                [val.to_string()]
                            )
                        )
                    } else {
                        Condition::all().add(
                            sea_query::Expr::cust_with_values(
                                &format!("json_extract({}, '$[#-1]') = ?", column_name),
                                [val.to_string()]
                            )
                        )
                    }
                },
                FieldOp::JsonObjectContains(key) => {
                    let column_name = <Entity as EntityTrait>::Column::#pascal_name.to_string();
                    if std::env::var("DATABASE_URL").unwrap_or_default().starts_with("postgres") {
                        Condition::all().add(
                            sea_query::Expr::cust_with_values(
                                &format!("{} ? ?", column_name),
                                [key]
                            )
                        )
                    } else {
                        Condition::all().add(
                            sea_query::Expr::cust_with_values(
                                &format!("json_extract({}, ?) IS NOT NULL", column_name),
                                [format!("$.{}", key)]
                            )
                        )
                    }
                },
                // String operations (not supported for JSON fields)
                FieldOp::Contains(_) => panic!("Use JsonStringContains for JSON string operations"),
                FieldOp::StartsWith(_) => panic!("Use JsonStringStartsWith for JSON string operations"),
                FieldOp::EndsWith(_) => panic!("Use JsonStringEndsWith for JSON string operations"),
                FieldOp::IsNull => panic!("IsNull operation not supported for non-nullable fields"),
                FieldOp::IsNotNull => panic!("IsNotNull operation not supported for non-nullable fields"),
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
            FieldOp::NotInVec(vs) => Condition::all().add(<Entity as EntityTrait>::Column::#pascal_name.is_not_in(vs)),
            FieldOp::Contains(_) => panic!("Contains operation not supported for this field type"),
            FieldOp::StartsWith(_) => panic!("StartsWith operation not supported for this field type"),
            FieldOp::EndsWith(_) => panic!("EndsWith operation not supported for this field type"),
            FieldOp::IsNull => Condition::all().add(<Entity as EntityTrait>::Column::#pascal_name.is_null()),
            FieldOp::IsNotNull => Condition::all().add(<Entity as EntityTrait>::Column::#pascal_name.is_not_null()),
            // JSON-specific operations (not supported for generic fields)
            FieldOp::JsonPath(_) => panic!("JsonPath operation only supported for JSON fields"),
            FieldOp::JsonStringContains(_) => panic!("JsonStringContains operation only supported for JSON fields"),
            FieldOp::JsonStringStartsWith(_) => panic!("JsonStringStartsWith operation only supported for JSON fields"),
            FieldOp::JsonStringEndsWith(_) => panic!("JsonStringEndsWith operation only supported for JSON fields"),
            FieldOp::JsonArrayContains(_) => panic!("JsonArrayContains operation only supported for JSON fields"),
            FieldOp::JsonArrayStartsWith(_) => panic!("JsonArrayStartsWith operation only supported for JSON fields"),
            FieldOp::JsonArrayEndsWith(_) => panic!("JsonArrayEndsWith operation only supported for JSON fields"),
            FieldOp::JsonObjectContains(_) => panic!("JsonObjectContains operation only supported for JSON fields"),
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
