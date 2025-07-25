// Logic for generating field variants for WhereParam enum and their filter functions.
// This module will support all types and will be long.

use heck::ToPascalCase;
use quote::{quote, format_ident};


/// Generate field variants, match arms, and field operator modules for WhereParam enum and filters.
pub fn generate_where_param_logic(
    fields: &[&syn::Field],
    unique_fields: &[&syn::Field],
    full_mod_path: &syn::Path,
) -> (Vec<proc_macro2::TokenStream>, Vec<proc_macro2::TokenStream>, Vec<proc_macro2::TokenStream>) {
    let mut where_field_variants: Vec<proc_macro2::TokenStream> = Vec::new();
    let mut field_ops: Vec<proc_macro2::TokenStream> = Vec::new();
    for field in fields.iter() {
        let name = field.ident.as_ref().unwrap();
        let pascal_name = format_ident!("{}", name.to_string().to_pascal_case());
        let ty = &field.ty;
        let is_unique = unique_fields.iter().any(|unique_field| unique_field.ident.as_ref().unwrap() == name);

        // WhereParam variant uses FieldOp<T>
        where_field_variants.push(quote! { #pascal_name(FieldOp<#ty>) });

        // Field operator module
        let set_fn = if !is_unique {
            quote! {
                pub fn set<T: Into<#ty>>(value: T) -> super::SetParam {
                    super::SetParam::#pascal_name(sea_orm::ActiveValue::Set(value.into()))
                }
            }
        } else { quote! {} };

        let unique_where_fn = if is_unique {
            let equals_variant = format_ident!("{}Equals", pascal_name);
            quote! {
                pub struct Equals(pub #ty);
                pub fn equals<T: From<Equals>>(value: impl Into<#ty>) -> T {
                    Equals(value.into()).into()
                }
                impl From<Equals> for super::UniqueWhereParam {
                    fn from(Equals(v): Equals) -> Self {
                        super::UniqueWhereParam::#equals_variant(v)
                    }
                }
                impl From<Equals> for WhereParam {
                    fn from(Equals(v): Equals) -> Self {
                        WhereParam::#pascal_name(FieldOp::Equals(v))
                    }
                }
            }
        } else {
            quote! {
                pub fn equals<T: Into<#ty>>(value: T) -> WhereParam {
                    WhereParam::#pascal_name(FieldOp::Equals(value.into()))
                }
            }
        };

        let order_fn = quote! {
            pub fn order(order: caustics::SortOrder) -> super::OrderByParam {
                super::OrderByParam::#pascal_name(order)
            }
        };

        // String ops
        let is_string_field = matches!(ty, syn::Type::Path(p) if p.path.is_ident("String"));
        let is_option_string_field = matches!(ty, syn::Type::Path(p) if {
            if let Some(segment) = p.path.segments.last() {
                segment.ident == "Option" && matches!(&segment.arguments, syn::PathArguments::AngleBracketed(args) if {
                    args.args.len() == 1 && matches!(args.args.first().unwrap(), syn::GenericArgument::Type(syn::Type::Path(inner)) if inner.path.is_ident("String"))
                })
            } else {
                false
            }
        });
        let string_ops = if is_string_field || is_option_string_field {
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
        } else { quote! {} };

        // Comparison ops for all types
        let comparison_ops = quote! {
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
            pub fn in_vec<T: Into<#ty>>(values: Vec<T>) -> WhereParam {
                WhereParam::#pascal_name(FieldOp::InVec(values.into_iter().map(|v| v.into()).collect()))
            }
            pub fn not_in_vec<T: Into<#ty>>(values: Vec<T>) -> WhereParam {
                WhereParam::#pascal_name(FieldOp::NotInVec(values.into_iter().map(|v| v.into()).collect()))
            }
        };

        let mut field_mod_items = vec![set_fn, unique_where_fn, order_fn, string_ops, comparison_ops];
        // If this is a string or Option<String> field, add a Mode variant and mode function
        if is_string_field || is_option_string_field {
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
        
        // Check if this is a string field that supports QueryMode
        let is_string_field = matches!(ty, syn::Type::Path(p) if p.path.is_ident("String"));
        let is_option_string_field = matches!(ty, syn::Type::Path(p) if {
            if let Some(segment) = p.path.segments.last() {
                segment.ident == "Option" && matches!(&segment.arguments, syn::PathArguments::AngleBracketed(args) if {
                    args.args.len() == 1 && matches!(args.args.first().unwrap(), syn::GenericArgument::Type(syn::Type::Path(inner)) if inner.path.is_ident("String"))
                })
            } else {
                false
            }
        });
        
        // Generate field operation handler
        if is_string_field || is_option_string_field {
            // String fields with QueryMode support
            if is_string_field {
                // Non-nullable String fields
                field_handlers.push(quote! {
                    WhereParam::#pascal_name(op) => {
                        let query_mode = query_modes.get(stringify!(#name)).copied().unwrap_or(caustics::QueryMode::Default);
                        match op {
                            FieldOp::Equals(v) => {
                                if query_mode == caustics::QueryMode::Insensitive {
                                    // For case-insensitive equals, use LIKE
                                    Condition::all().add(<Entity as EntityTrait>::Column::#pascal_name.like(v))
                                } else {
                                    Condition::all().add(<Entity as EntityTrait>::Column::#pascal_name.eq(v))
                                }
                            },
                            FieldOp::NotEquals(v) => {
                                if query_mode == caustics::QueryMode::Insensitive {
                                    // For case-insensitive not equals, use NOT LIKE
                                    Condition::all().add(<Entity as EntityTrait>::Column::#pascal_name.not_like(v))
                                } else {
                                    Condition::all().add(<Entity as EntityTrait>::Column::#pascal_name.ne(v))
                                }
                            },
                            FieldOp::Contains(s) => {
                                if query_mode == caustics::QueryMode::Insensitive {
                                    // LIKE is case-insensitive in SQLite by default
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
                        }
                    }
                });
            } else {
                // Nullable Option<String> fields
                field_handlers.push(quote! {
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
                            FieldOp::Gt(v) => Condition::all().add(<Entity as EntityTrait>::Column::#pascal_name.gt(v)),
                            FieldOp::Lt(v) => Condition::all().add(<Entity as EntityTrait>::Column::#pascal_name.lt(v)),
                            FieldOp::Gte(v) => Condition::all().add(<Entity as EntityTrait>::Column::#pascal_name.gte(v)),
                            FieldOp::Lte(v) => Condition::all().add(<Entity as EntityTrait>::Column::#pascal_name.lte(v)),
                            FieldOp::InVec(vs) => Condition::all().add(<Entity as EntityTrait>::Column::#pascal_name.is_in(vs)),
                            FieldOp::NotInVec(vs) => Condition::all().add(<Entity as EntityTrait>::Column::#pascal_name.is_not_in(vs)),
                        }
                    }
                });
            }
            
            // Generate mode handler for string fields
            let mode_variant = format_ident!("{}Mode", pascal_name);
            mode_handlers.push(quote! {
                WhereParam::#mode_variant(mode) => {
                    query_modes.insert(stringify!(#name).to_string(), mode);
                    continue; // Skip adding condition, this just sets the mode
                }
            });
        } else {
            // Non-string fields (no QueryMode support)
            field_handlers.push(quote! {
                WhereParam::#pascal_name(op) => match op {
                    FieldOp::Equals(v) => Condition::all().add(<Entity as EntityTrait>::Column::#pascal_name.eq(v)),
                    FieldOp::NotEquals(v) => Condition::all().add(<Entity as EntityTrait>::Column::#pascal_name.ne(v)),
                    FieldOp::Gt(v) => Condition::all().add(<Entity as EntityTrait>::Column::#pascal_name.gt(v)),
                    FieldOp::Lt(v) => Condition::all().add(<Entity as EntityTrait>::Column::#pascal_name.lt(v)),
                    FieldOp::Gte(v) => Condition::all().add(<Entity as EntityTrait>::Column::#pascal_name.gte(v)),
                    FieldOp::Lte(v) => Condition::all().add(<Entity as EntityTrait>::Column::#pascal_name.lte(v)),
                    FieldOp::InVec(vs) => Condition::all().add(<Entity as EntityTrait>::Column::#pascal_name.is_in(vs)),
                    FieldOp::NotInVec(vs) => Condition::all().add(<Entity as EntityTrait>::Column::#pascal_name.is_not_in(vs)),
                    FieldOp::Contains(_) => panic!("Contains operation not supported for non-string fields"),
                    FieldOp::StartsWith(_) => panic!("StartsWith operation not supported for non-string fields"),
                    FieldOp::EndsWith(_) => panic!("EndsWith operation not supported for non-string fields"),
                }
            });
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
