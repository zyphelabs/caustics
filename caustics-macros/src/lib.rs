#![crate_type = "proc-macro"]
#![allow(dead_code)]
#![allow(unused_variables)]
// Enable decl_macro feature when select feature is enabled
#![cfg_attr(feature = "select", feature(decl_macro))]

use proc_macro::TokenStream;
use quote::quote;
use syn::{DeriveInput, Error, File};

mod common;
mod entity;
mod errors;
mod name_resolution;
mod primary_key;
mod select_struct;
mod validation;
mod where_param;

#[proc_macro_attribute]
pub fn caustics(_args: TokenStream, input: TokenStream) -> TokenStream {
    // No longer using namespaces - simplified
    let namespace = "default".to_string();

    let mut ast = match syn::parse::<syn::ItemMod>(input.clone()) {
        Ok(ast) => ast,
        Err(e) => {
            return Error::new(
                e.span(),
                "The #[caustics] attribute must be placed on a module declaration",
            )
            .to_compile_error()
            .into();
        }
    };

    let mut model_ast = None;
    let mut relation_ast = None;

    // Get module content or return error
    let content = match &ast.content {
        Some(content) => content,
        None => {
            return Error::new(ast.ident.span(), "Module must have a body")
                .to_compile_error()
                .into();
        }
    };

    // Find struct and enum with #[derive(Caustics)]
    for item in &content.1 {
        match item {
            syn::Item::Struct(struct_item) => {
                if struct_item.attrs.iter().any(|attr| {
                    if attr.path().is_ident("derive") {
                        if let syn::Meta::List(meta) = &attr.meta {
                            ::std::string::ToString::to_string(&meta.tokens).contains("Caustics")
                        } else {
                            false
                        }
                    } else {
                        false
                    }
                }) {
                    // Filter out #[derive(Caustics)] attributes
                    let filtered_attrs: Vec<_> = struct_item
                        .attrs
                        .iter()
                        .filter(|attr| {
                            !(attr.path().is_ident("derive")
                                && if let syn::Meta::List(meta) = &attr.meta {
                                    ::std::string::ToString::to_string(&meta.tokens)
                                        .contains("Caustics")
                                } else {
                                    false
                                })
                        })
                        .cloned()
                        .collect();

                    model_ast = Some(DeriveInput {
                        attrs: filtered_attrs,
                        vis: struct_item.vis.clone(),
                        ident: struct_item.ident.clone(),
                        generics: struct_item.generics.clone(),
                        data: syn::Data::Struct(syn::DataStruct {
                            struct_token: struct_item.struct_token,
                            fields: struct_item.fields.clone(),
                            semi_token: struct_item.semi_token,
                        }),
                    });
                }
            }
            syn::Item::Enum(enum_item) => {
                // Check if this enum has #[derive(Caustics)]
                let has_caustics_derive = enum_item.attrs.iter().any(|attr| {
                    if attr.path().is_ident("derive") {
                        if let syn::Meta::List(meta) = &attr.meta {
                            ::std::string::ToString::to_string(&meta.tokens).contains("Caustics")
                        } else {
                            false
                        }
                    } else {
                        false
                    }
                });

                if has_caustics_derive {
                    // Filter out #[derive(Caustics)] attributes
                    let filtered_attrs: Vec<_> = enum_item
                        .attrs
                        .iter()
                        .filter(|attr| {
                            !(attr.path().is_ident("derive")
                                && if let syn::Meta::List(meta) = &attr.meta {
                                    ::std::string::ToString::to_string(&meta.tokens)
                                        .contains("Caustics")
                                } else {
                                    false
                                })
                        })
                        .cloned()
                        .collect();

                    relation_ast = Some(DeriveInput {
                        attrs: filtered_attrs,
                        vis: enum_item.vis.clone(),
                        ident: enum_item.ident.clone(),
                        generics: enum_item.generics.clone(),
                        data: syn::Data::Enum(syn::DataEnum {
                            enum_token: enum_item.enum_token,
                            brace_token: enum_item.brace_token,
                            variants: enum_item.variants.clone(),
                        }),
                    });
                }
            }
            _ => {}
        }
    }

    // If we found both struct and enum with #[derive(Caustics)], generate the entity code
    match (&model_ast, &relation_ast) {
        (Some(model_ast), Some(relation_ast)) => {
            // Use the module name for the path: crate::<mod_ident>
            let mod_ident = &ast.ident;
            let full_mod_path: syn::Path =
                syn::parse_str(&format!("crate::{}", mod_ident)).unwrap();
            let generated =
                match entity::generate_entity(model_ast.clone(), relation_ast.clone(), namespace, &full_mod_path) {
                    Ok(tokens) => tokens,
                    Err(error_tokens) => return error_tokens.into(),
                };

            // Debug: Print the generated code (commented for production, useful for AI debugging)
            //eprintln!("DEBUG: Generated code for {}: {}", mod_ident, generated);

            // Parse the generated items into a File
            let generated_file = match syn::parse2::<File>(generated) {
                Ok(file) => file,
                Err(e) => {
                    return Error::new(e.span(), format!("Failed to parse generated items: {}", e))
                        .to_compile_error()
                        .into();
                }
            };

            // Modify the module's content to include the generated items
            if let Some((_, items)) = &mut ast.content {
                items.extend(generated_file.items);
            }

            // Return the modified module
            quote! {
                #[allow(clippy::cmp_owned)]
                #[allow(clippy::type_complexity)]
                #[allow(clippy::too_many_arguments)]
                #[allow(clippy::possible_missing_else)]
                #ast
            }
            .into()
        }
        _ => {
            // Check if we have a Model but no Relation with Caustics derive
            if model_ast.is_some() && relation_ast.is_none() {
                return crate::errors::CausticsError::missing_caustics_on_relation(
                    &ast.ident.to_string(),
                    ast.ident.span(),
                )
                .into();
            }
            
            // If we didn't find both struct and enum with #[derive(Caustics)], return the original module
            quote! {
                #[allow(clippy::cmp_owned)]
                #[allow(clippy::type_complexity)]
                #[allow(clippy::too_many_arguments)]
                #[allow(clippy::possible_missing_else)]
                #ast
            }
            .into()
        }
    }
}

#[proc_macro_derive(Caustics)]
pub fn caustics_derive(_input: TokenStream) -> TokenStream {
    // Return empty token stream since this is just a marker
    TokenStream::new()
}

#[proc_macro]
pub fn select_struct(input: TokenStream) -> TokenStream {
    select_struct::select_struct(input)
}
