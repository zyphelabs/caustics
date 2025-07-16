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
    let mut where_field_variants = Vec::new();
    let mut where_match_arms = Vec::new();
    let mut field_ops = Vec::new();
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
        let is_string_field = if let syn::Type::Path(type_path) = ty {
            let last = &type_path.path.segments.last().unwrap().ident;
            last == "String"
        } else { false };
        let is_option_string_field = if crate::common::is_option(ty) {
                if let syn::Type::Path(option_path) = ty {
                    if let Some(segment) = option_path.path.segments.last() {
                        if let syn::PathArguments::AngleBracketed(args) = &segment.arguments {
                            if let Some(syn::GenericArgument::Type(inner_ty)) = args.args.first() {
                                if let syn::Type::Path(inner_path) = inner_ty {
                                    if let Some(inner_segment) = inner_path.path.segments.last() {
                                    inner_segment.ident == "String"
                                } else { false }
                            } else { false }
                        } else { false }
                    } else { false }
                } else { false }
            } else { false }
        } else { false };
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
        pub fn or(params: Vec<WhereParam>) -> WhereParam {
            WhereParam::Or(params)
        }
        pub fn not(params: Vec<WhereParam>) -> WhereParam {
            WhereParam::Not(params)
        }
    });
    // Use unqualified name for logical operator variants
    where_field_variants.push(quote! { And(Vec<WhereParam>) });
    where_field_variants.push(quote! { Or(Vec<WhereParam>) });
    where_field_variants.push(quote! { Not(Vec<WhereParam>) });


    // Generate match arms for all WhereParam variants in From<WhereParam> for Condition.
    for field in fields.iter() {
        let name = field.ident.as_ref().unwrap();
        let pascal_name = format_ident!("{}", name.to_string().to_pascal_case());
        let ty = &field.ty;
        // Generate match arm for WhereParam -> Condition
        where_match_arms.push(quote! {
            WhereParam::#pascal_name(op) => match op {
                FieldOp::Equals(v) => Condition::all().add(<Entity as EntityTrait>::Column::#pascal_name.eq(v)),
                FieldOp::NotEquals(v) => Condition::all().add(<Entity as EntityTrait>::Column::#pascal_name.ne(v)),
                FieldOp::Gt(v) => Condition::all().add(<Entity as EntityTrait>::Column::#pascal_name.gt(v)),
                FieldOp::Lt(v) => Condition::all().add(<Entity as EntityTrait>::Column::#pascal_name.lt(v)),
                FieldOp::Gte(v) => Condition::all().add(<Entity as EntityTrait>::Column::#pascal_name.gte(v)),
                FieldOp::Lte(v) => Condition::all().add(<Entity as EntityTrait>::Column::#pascal_name.lte(v)),
                FieldOp::InVec(vs) => Condition::all().add(<Entity as EntityTrait>::Column::#pascal_name.is_in(vs)),
                FieldOp::NotInVec(vs) => Condition::all().add(<Entity as EntityTrait>::Column::#pascal_name.is_not_in(vs)),
                FieldOp::Contains(s) => Condition::all().add(<Entity as EntityTrait>::Column::#pascal_name.contains(s)),
                FieldOp::StartsWith(s) => Condition::all().add(<Entity as EntityTrait>::Column::#pascal_name.starts_with(s)),
                FieldOp::EndsWith(s) => Condition::all().add(<Entity as EntityTrait>::Column::#pascal_name.ends_with(s)),
            }
        });
    }
    // Logical operator match arms
    where_match_arms.push(quote! {
        WhereParam::And(params) => {
            let mut cond = Condition::all();
            for p in params {
                cond = cond.add(Condition::from(p));
            }
            cond
        }
    });
    where_match_arms.push(quote! {
        WhereParam::Or(params) => {
            let mut cond = Condition::any();
            for p in params {
                cond = cond.add(Condition::from(p));
            }
            cond
        }
    });
    where_match_arms.push(quote! {
        WhereParam::Not(params) => {
            let mut cond = Condition::all();
            for p in params {
                cond = cond.add(Condition::from(p).not());
            }
            cond
        }
    });
    (where_field_variants, where_match_arms, field_ops)
} 