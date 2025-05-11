#![crate_type = "proc-macro"]

use proc_macro::TokenStream;
use quote::{quote, format_ident};
use syn::{parse_macro_input, DeriveInput, Data, Fields};
use std::sync::Mutex;
use std::collections::HashSet;
use heck::ToPascalCase;
use chrono::{DateTime, FixedOffset};
use uuid::Uuid;

lazy_static::lazy_static! {
    static ref ENTITIES: Mutex<HashSet<String>> = Mutex::new(HashSet::new());
}

#[proc_macro_derive(Caustics)]
pub fn caustics_derive(input: TokenStream) -> TokenStream {
    let ast = parse_macro_input!(input as DeriveInput);
    let name = &ast.ident;
    let name_str = name.to_string();
    ENTITIES.lock().unwrap().insert(name_str.clone());

    // Extract fields
    let fields = match &ast.data {
        Data::Struct(data_struct) => match &data_struct.fields {
            Fields::Named(fields_named) => fields_named.named.iter().collect::<Vec<_>>(),
            _ => panic!("Expected named fields"),
        },
        _ => panic!("Expected a struct"),
    };

    // Filter out primary key fields for set operations
    let primary_key_fields: Vec<_> = fields
        .iter()
        .filter(|field| {
            field.attrs.iter().any(|attr| {
                if let syn::Meta::List(meta) = &attr.meta {
                    (meta.path.is_ident("sea_orm") && meta.tokens.to_string().contains("primary_key")) ||
                    meta.path.is_ident("primary_key")
                } else {
                    false
                }
            })
        })
        .collect();

    // Generate function arguments for required fields (excluding primary keys)
    let required_args = fields
        .iter()
        .filter(|field| !primary_key_fields.contains(field))
        .enumerate()
        .map(|(i, field)| {
            let _name = field.ident.as_ref().unwrap();
            let arg_name = format_ident!("arg{}", i);
            let ty = &field.ty;
            quote! { #arg_name: SetValue }
        });

    // Generate field names for required fields (excluding primary keys)
    let required_names = fields
        .iter()
        .filter(|field| !primary_key_fields.contains(field))
        .enumerate()
        .map(|(i, field)| {
            let name = field.ident.as_ref().unwrap();
            let arg_name = format_ident!("arg{}", i);
            quote! { 
                #name: match #arg_name {
                    SetValue::#name(value) => value,
                    _ => panic!("Invalid SetValue variant for field {}", stringify!(#name))
                }
            }
        });

    // Generate field variants for SetValue enum (excluding primary keys)
    let field_variants = fields
        .iter()
        .filter(|field| !primary_key_fields.contains(field))
        .map(|field| {
            let name = field.ident.as_ref().unwrap();
            let ty = &field.ty;
            quote! {
                #name(sea_orm::ActiveValue<#ty>)
            }
        });

    let match_arms = fields
        .iter()
        .filter(|field| !primary_key_fields.contains(field))
        .map(|field| {
            let name = field.ident.as_ref().unwrap();
            quote! {
                SetValue::#name(value) => {
                    model.#name = value.clone();
                }
            }
        });

    // Generate field operator modules (including primary keys for query operations)
    let field_ops = fields.iter().map(|field| {
        let field_name = &field.ident;
        let field_type = &field.ty;
        let column_variant = format_ident!("{}", field_name.as_ref().unwrap().to_string().to_pascal_case());
        let is_primary_key = primary_key_fields.iter().any(|pk_field| {
            pk_field.ident.as_ref().unwrap() == field_name.as_ref().unwrap()
        });
        
        let set_fn = if !is_primary_key {
            quote! {
                pub fn set<T: Into<#field_type>>(value: T) -> SetValue {
                    SetValue::#field_name(sea_orm::ActiveValue::Set(value.into()))
                }
            }
        } else {
            quote! {}
        };
        
        quote! {
            pub mod #field_name {
                use super::{Entity, Model, ActiveModel, SetValue};
                use sea_orm::{Condition, ColumnTrait, EntityTrait, ActiveValue};
                use chrono::{NaiveDate, NaiveDateTime, DateTime, FixedOffset};
                use uuid::Uuid;
                use std::vec::Vec;

                #set_fn

                pub fn equals<T: Into<#field_type>>(value: T) -> Condition {
                    Condition::all().add(<Entity as EntityTrait>::Column::#column_variant.eq(value.into()))
                }
                pub fn not_equals<T: Into<#field_type>>(value: T) -> Condition {
                    Condition::all().add(<Entity as EntityTrait>::Column::#column_variant.ne(value.into()))
                }
                pub fn gt<T: Into<#field_type>>(value: T) -> Condition {
                    Condition::all().add(<Entity as EntityTrait>::Column::#column_variant.gt(value.into()))
                }
                pub fn lt<T: Into<#field_type>>(value: T) -> Condition {
                    Condition::all().add(<Entity as EntityTrait>::Column::#column_variant.lt(value.into()))
                }
                pub fn gte<T: Into<#field_type>>(value: T) -> Condition {
                    Condition::all().add(<Entity as EntityTrait>::Column::#column_variant.gte(value.into()))
                }
                pub fn lte<T: Into<#field_type>>(value: T) -> Condition {
                    Condition::all().add(<Entity as EntityTrait>::Column::#column_variant.lte(value.into()))
                }
                pub fn in_vec<T: Into<#field_type>>(values: Vec<T>) -> Condition {
                    Condition::all().add(<Entity as EntityTrait>::Column::#column_variant.is_in(values.into_iter().map(|v| v.into()).collect::<Vec<_>>()))
                }
                pub fn not_in_vec<T: Into<#field_type>>(values: Vec<T>) -> Condition {
                    Condition::all().add(<Entity as EntityTrait>::Column::#column_variant.is_not_in(values.into_iter().map(|v| v.into()).collect::<Vec<_>>()))
                }
            }
        }
    });

    let expanded = quote! {
        use sea_orm::{DatabaseConnection, Condition, EntityTrait, ActiveValue};

        pub struct EntityClient {
            db: DatabaseConnection,
        }

        // Enum to handle different Set types
        pub enum SetValue {
            #(#field_variants),*
        }

        impl SetValue {
            fn merge_into(&self, model: &mut ActiveModel) {
                match self {
                    #(#match_arms),*
                }
            }
        }

        impl EntityClient {
            pub fn new(db: DatabaseConnection) -> Self {
                Self { db }
            }
            pub fn db(&self) -> &DatabaseConnection {
                &self.db
            }
            pub async fn find_unique(&self, condition: Condition) -> Result<Option<Model>, sea_orm::DbErr> {
                <Entity as EntityTrait>::find().filter(condition).one(&self.db).await
            }
            pub async fn find_first(&self, conditions: Vec<Condition>) -> Result<Option<Model>, sea_orm::DbErr> {
                let mut query = <Entity as EntityTrait>::find();
                for cond in conditions {
                    query = query.filter(cond);
                }
                query.one(&self.db).await
            }
            pub async fn find_many(&self, conditions: Vec<Condition>) -> Result<Vec<Model>, sea_orm::DbErr> {
                let mut query = <Entity as EntityTrait>::find();
                for cond in conditions {
                    query = query.filter(cond);
                }
                query.all(&self.db).await
            }
            pub async fn create(&self, #(#required_args,)* optional: Vec<SetValue>) -> Result<Model, sea_orm::DbErr> {
                let mut model = ActiveModel {
                    #(#required_names,)*
                    ..Default::default()
                };
                for opt in optional {
                    opt.merge_into(&mut model);
                }
                model.insert(&self.db).await
            }
        }

        // Field operator modules
        #(#field_ops)*
    };

    TokenStream::from(expanded)
}

#[proc_macro]
pub fn generate_client(_input: TokenStream) -> TokenStream {
    let expanded = generate_client_impl();
    TokenStream::from(expanded)
}

fn generate_client_impl() -> proc_macro2::TokenStream {
    let entity_names: Vec<String> = ENTITIES.lock().unwrap().iter().cloned().collect();
    let entity_methods: Vec<_> = entity_names.iter().map(|entity_name| {
        let method_name = format_ident!("{}", entity_name.to_lowercase());
        let entity_client = format_ident!("{}Client", entity_name);
        
        quote! {
            pub fn #method_name(&self) -> #entity_client {
                #entity_client::new(self.db.clone())
            }
        }
    }).collect();

    quote! {
        use sea_orm::DatabaseConnection;
        use std::sync::Arc;

        pub struct CausticsClient {
            db: Arc<DatabaseConnection>,
        }

        impl CausticsClient {
            pub fn new(db: DatabaseConnection) -> Self {
                Self { db: Arc::new(db) }
            }

            pub fn db(&self) -> &DatabaseConnection {
                &self.db
            }

            #(#entity_methods)*
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use quote::quote;

    #[test]
    fn test_expanded_macro() {
        let input = quote! {
            #[derive(Caustics)]
            struct User {
                email: String,
                name: String,
                age: i32,
                active: bool,
            }
        };

        let expanded = caustics_derive(input.into());
        println!("Expanded macro:\n{}", expanded);
    }
}