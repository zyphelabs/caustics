//! Strict validation functions that fail-fast instead of using fallbacks
//! This ensures configuration errors are caught at compile time

use crate::errors::CausticsError;
use crate::primary_key::PrimaryKeyInfo;
use proc_macro2::Span;
use proc_macro2::TokenStream;
use quote::quote;
use syn::{DeriveInput, Field};

/// Strictly validate that an entity has a primary key
/// Fails with clear error if no primary key is found
pub fn validate_primary_key(entity_ast: &DeriveInput) -> Result<PrimaryKeyInfo, TokenStream> {
    let entity_name = &entity_ast.ident;
    let fields = match &entity_ast.data {
        syn::Data::Struct(data_struct) => match &data_struct.fields {
            syn::Fields::Named(fields_named) => &fields_named.named,
            _ => {
                return Err(CausticsError::no_primary_key(
                    &entity_name.to_string(),
                    entity_ast.ident.span(),
                ));
            }
        },
        _ => {
            return Err(CausticsError::no_primary_key(
                &entity_name.to_string(),
                entity_ast.ident.span(),
            ));
        }
    };

    // Look for explicit primary key attribute
    let explicit_pk = fields.iter().find(|field| {
        field.attrs.iter().any(|attr| {
            if let syn::Meta::List(meta) = &attr.meta {
                meta.path.is_ident("sea_orm") && meta.tokens.to_string().contains("primary_key")
            } else {
                false
            }
        })
    });

    if let Some(field) = explicit_pk {
        return Ok(PrimaryKeyInfo::from_field(field));
    }

    // No primary key found - fail with clear error
    Err(CausticsError::no_primary_key(
        &entity_name.to_string(),
        entity_ast.ident.span(),
    ))
}

/// Strictly validate that a relation has a foreign key column
/// Fails with clear error if no foreign key column is specified
pub fn validate_foreign_key_column(
    relation_name: &str,
    foreign_key_column: &Option<String>,
    span: Span,
) -> Result<String, TokenStream> {
    match foreign_key_column {
        Some(column) => Ok(column.clone()),
        None => Err(CausticsError::no_foreign_key_column(relation_name, span)),
    }
}

/// Strictly validate that a relation has a primary key field specified
/// Fails with clear error if no primary key field is specified
pub fn validate_relation_primary_key(
    relation_name: &str,
    primary_key_field: &Option<String>,
    span: Span,
) -> Result<String, TokenStream> {
    match primary_key_field {
        Some(field) => Ok(field.clone()),
        None => Err(CausticsError::no_relation_primary_key(relation_name, span)),
    }
}

/// Strictly validate that an entity has a table name
/// Fails with clear error if no table name is specified
pub fn validate_table_name(entity_ast: &DeriveInput) -> Result<String, TokenStream> {
    let entity_name = &entity_ast.ident;

    for attr in &entity_ast.attrs {
        if let syn::Meta::List(meta) = &attr.meta {
            if meta.path.is_ident("sea_orm") {
                if let Ok(nested) = meta.parse_args_with(
                    syn::punctuated::Punctuated::<syn::Meta, syn::Token![,]>::parse_terminated,
                ) {
                    for meta in nested {
                        if let syn::Meta::NameValue(nv) = &meta {
                            if nv.path.is_ident("table_name") {
                                if let syn::Expr::Lit(syn::ExprLit {
                                    lit: syn::Lit::Str(lit),
                                    ..
                                }) = &nv.value
                                {
                                    return Ok(lit.value());
                                }
                            }
                        }
                    }
                }
            }
        }
    }

    // No table name found - fail with clear error
    Err(CausticsError::no_table_name(
        &entity_name.to_string(),
        entity_ast.ident.span(),
    ))
}

/// Validate that a field type is supported
pub fn validate_field_type(field: &Field) -> Result<(), TokenStream> {
    let field_name = field.ident.as_ref().unwrap().to_string();
    let type_name = quote::quote! { #field.ty }.to_string();

    // Check if it's a supported type
    let is_supported = match &field.ty {
        syn::Type::Path(path) => {
            if let Some(segment) = path.path.segments.last() {
                matches!(
                    segment.ident.to_string().as_str(),
                    "String"
                        | "bool"
                        | "i8"
                        | "i16"
                        | "i32"
                        | "i64"
                        | "u8"
                        | "u16"
                        | "u32"
                        | "u64"
                        | "f32"
                        | "f64"
                        | "Uuid"
                        | "DateTime"
                        | "NaiveDateTime"
                        | "NaiveDate"
                        | "Value"
                        | "Option"
                )
            } else {
                false
            }
        }
        _ => false,
    };

    if !is_supported {
        return Err(CausticsError::UnsupportedFieldType {
            field_name,
            type_name,
        }
        .to_compile_error(field.ident.as_ref().unwrap().span()));
    }

    Ok(())
}

/// Validate that there are no duplicate primary keys in an entity
pub fn validate_no_duplicate_primary_keys(fields: &[&Field]) -> Result<(), TokenStream> {
    let primary_key_fields: Vec<_> = fields
        .iter()
        .filter(|field| {
            field.attrs.iter().any(|attr| {
                if let syn::Meta::List(meta) = &attr.meta {
                    meta.path.is_ident("sea_orm") && meta.tokens.to_string().contains("primary_key")
                } else {
                    false
                }
            })
        })
        .collect();

    if primary_key_fields.len() > 1 {
        let field_names: Vec<String> = primary_key_fields
            .iter()
            .map(|field| field.ident.as_ref().unwrap().to_string())
            .collect();

        let field_names_str = field_names.join(", ");
        return Err(quote! {
            compile_error!(concat!(
                "Multiple primary key fields found: ",
                #field_names_str,
                ". Please specify exactly one primary key field."
            ))
        });
    }

    Ok(())
}

/// Validate that relations don't create circular dependencies
pub fn validate_no_circular_relations(
    relations: &[crate::entity::Relation],
    entity_name: &str,
) -> Result<(), TokenStream> {
    // Check for self-referencing relations
    for relation in relations {
        let target_entity_name = relation
            .target
            .segments
            .last()
            .map(|segment| segment.ident.to_string())
            .unwrap_or_default();

        if target_entity_name.to_lowercase() == entity_name.to_lowercase() {
            let relation_name = &relation.name;
            return Err(quote! {
                compile_error!(concat!(
                    "Circular relation detected: '",
                    #entity_name,
                    "' references itself through relation '",
                    #relation_name,
                    "'. Please remove the circular dependency."
                ))
            });
        }
    }

    Ok(())
}

/// Validate that relation targets are valid entity paths
pub fn validate_relation_targets(relations: &[crate::entity::Relation]) -> Result<(), TokenStream> {
    for relation in relations {
        // Check if the target path is valid (basic validation)
        if relation.target.segments.is_empty() {
            let relation_name = &relation.name;
            return Err(quote! {
                compile_error!(concat!(
                    "Invalid relation target for '",
                    #relation_name,
                    "': empty path. Please provide a valid entity path."
                ))
            });
        }
    }

    Ok(())
}
