#![crate_type = "proc-macro"]
#![allow(dead_code)]
#![allow(unused_variables)]

use proc_macro::TokenStream;
use syn::{DeriveInput, Error, File};
use quote::quote;


mod common;
mod entity;

#[proc_macro_attribute]
pub fn caustics(_args: TokenStream, input: TokenStream) -> TokenStream {
    let mut ast = match syn::parse::<syn::ItemMod>(input.clone()) {
        Ok(ast) => ast,
        Err(e) => return Error::new(e.span(), "The #[caustics] attribute must be placed on a module declaration").to_compile_error().into(),
    };

    let mut model_ast = None;
    let mut relation_ast = None;

    // Get module content or return error
    let content = match &ast.content {
        Some(content) => content,
        None => return Error::new(ast.ident.span(), "Module must have a body").to_compile_error().into(),
    };

    // Find struct and enum with #[derive(Caustics)]
    for item in &content.1 {
        match item {
            syn::Item::Struct(struct_item) => {
                if struct_item.attrs.iter().any(|attr| {
                    if attr.path().is_ident("derive") {
                        if let syn::Meta::List(meta) = &attr.meta {
                            meta.tokens.to_string().contains("Caustics")
                        } else {
                            false
                        }
                    } else {
                        false
                    }
                }) {
                    // Filter out #[derive(Caustics)] attributes
                    let filtered_attrs: Vec<_> = struct_item.attrs.iter()
                        .filter(|attr| {
                            !(attr.path().is_ident("derive") && 
                              if let syn::Meta::List(meta) = &attr.meta {
                                  meta.tokens.to_string().contains("Caustics")
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
                if enum_item.attrs.iter().any(|attr| {
                    if attr.path().is_ident("derive") {
                        if let syn::Meta::List(meta) = &attr.meta {
                            meta.tokens.to_string().contains("Caustics")
                        } else {
                            false
                        }
                    } else {
                        false
                    }
                }) {
                    // Filter out #[derive(Caustics)] attributes
                    let filtered_attrs: Vec<_> = enum_item.attrs.iter()
                        .filter(|attr| {
                            !(attr.path().is_ident("derive") && 
                              if let syn::Meta::List(meta) = &attr.meta {
                                  meta.tokens.to_string().contains("Caustics")
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
    match (model_ast, relation_ast) {
        (Some(model_ast), Some(relation_ast)) => {
            let generated = entity::generate_entity(model_ast, relation_ast);
            
            // Parse the generated items into a File
            let generated_file = match syn::parse2::<File>(generated) {
                Ok(file) => file,
                Err(e) => return Error::new(e.span(), format!("Failed to parse generated items: {}", e)).to_compile_error().into(),
            };

            // Modify the module's content to include the generated items
            if let Some((_, items)) = &mut ast.content {
                items.extend(generated_file.items);
            }

            // Return the modified module
            quote! {
                #ast
            }.into()
        }
        _ => {
            // If we didn't find both struct and enum with #[derive(Caustics)], return the original module
            quote! {
                #ast
            }.into()
        }
    }
}

#[proc_macro_derive(Caustics)]
pub fn caustics_derive(_input: TokenStream) -> TokenStream {
    // Return empty token stream since this is just a marker
    TokenStream::new()
}
