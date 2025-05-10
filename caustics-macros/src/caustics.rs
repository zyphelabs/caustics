use proc_macro2::TokenStream;
use quote::{quote, format_ident};
use lazy_static::lazy_static;
use parking_lot::Mutex;
use heck::ToPascalCase;
use std::collections::HashSet;
use proc_macro::TokenStream as ProcMacroTokenStream;
use syn::{parse_macro_input, DeriveInput, Data, Fields, Type, Ident, Path, PathSegment, GenericArgument, AngleBracketedGenericArguments};
use sea_orm::{Condition, QueryBuilder};
use sea_orm::Error;

// Store entity names for client generation
lazy_static! {
    static ref ENTITIES: Mutex<HashSet<String>> = Mutex::new(HashSet::new());
}

#[proc_macro_derive(Caustics)]
pub fn caustics_derive(input: TokenStream) -> TokenStream {
    let ast = parse_macro_input!(input as DeriveInput);
    let name = &ast.ident;
    let name_str = name.to_string();
    
    // Register the entity name
    ENTITIES.lock().insert(name_str.clone());

    let expanded = quote! {
        impl #name {
            pub fn id(&self) -> i32 {
                self.id
            }
        }
    };

    TokenStream::from(expanded)
}

fn generate_entity_client(entity_name: &str) -> proc_macro2::TokenStream {
    let entity_ident = format_ident!("{}", entity_name);
    let entity_client = format_ident!("{}Client", entity_name);
    
    quote! {
        pub struct #entity_client {
            db: DatabaseConnection,
        }

        impl #entity_client {
            pub fn new(db: DatabaseConnection) -> Self {
                Self { db }
            }

            pub async fn find_unique(&self, condition: Condition) -> Result<Option<#entity_ident::Model>, Error> {
                let query = QueryBuilder::new()
                    .select_from(#entity_ident::Entity)
                    .where_condition(condition)
                    .limit(1)
                    .build();
                
                let result = self.db.query_one(&query).await?;
                Ok(result.map(|row| #entity_ident::Model::from_row(row)))
            }

            pub async fn find_first(&self, conditions: Vec<Condition>) -> Result<Option<#entity_ident::Model>, Error> {
                let query = QueryBuilder::new()
                    .select_from(#entity_ident::Entity)
                    .where_conditions(conditions)
                    .limit(1)
                    .build();
                
                let result = self.db.query_one(&query).await?;
                Ok(result.map(|row| #entity_ident::Model::from_row(row)))
            }

            pub async fn find_many(&self, conditions: Vec<Condition>) -> Result<Vec<#entity_ident::Model>, Error> {
                let query = QueryBuilder::new()
                    .select_from(#entity_ident::Entity)
                    .where_conditions(conditions)
                    .build();
                
                let results = self.db.query_all(&query).await?;
                Ok(results.into_iter().map(|row| #entity_ident::Model::from_row(row)).collect())
            }
        }
    }
}

fn generate_client_impl() -> proc_macro2::TokenStream {
    let entity_methods: Vec<_> = ENTITIES.lock().iter().map(|entity_name| {
        let method_name = format_ident!("{}", entity_name.to_lowercase());
        let entity_client = format_ident!("{}Client", entity_name);
        
        quote! {
            pub fn #method_name(&self) -> #entity_client {
                #entity_client::new(self.db.clone())
            }
        }
    }).collect();

    quote! {
        pub struct CausticsClient {
            db: DatabaseConnection,
        }

        impl CausticsClient {
            pub fn new(db: DatabaseConnection) -> Self {
                Self { db }
            }

            pub fn db(&self) -> &DatabaseConnection {
                &self.db
            }

            #(#entity_methods)*
        }
    }
}

#[proc_macro]
pub fn generate_client(_input: TokenStream) -> TokenStream {
    let expanded = generate_client_impl();
    TokenStream::from(expanded)
}

pub fn generate_caustics_impl(input: TokenStream) -> TokenStream {
    let ast: syn::DeriveInput = syn::parse(input.into()).unwrap();
    let name = &ast.ident;
    let (_impl_generics, _ty_generics, _where_clause) = ast.generics.split_for_impl();

    // Register the entity name
    ENTITIES.lock().insert(name.to_string());

    let fields = match &ast.data {
        syn::Data::Struct(syn::DataStruct { fields: syn::Fields::Named(fields), .. }) => fields.named.iter().collect::<Vec<_>>(),
        _ => panic!("Expected a struct with named fields"),
    };

    let field_ops = fields.iter().map(|field| {
        let field_name = &field.ident;
        let field_type = &field.ty;
        let column_variant = syn::Ident::new(&field_name.as_ref().unwrap().to_string().to_pascal_case(), field_name.as_ref().unwrap().span());
        quote! {
            pub mod #field_name {
                use super::{Entity, Model, ActiveModel};
                use sea_orm::{Condition, ColumnTrait, Iterable, EntityTrait};
                use chrono::{DateTime, FixedOffset};
                pub fn equals<T: Into<#field_type>>(value: T) -> Condition {
                    Condition::all().add(<Entity as sea_orm::EntityTrait>::Column::#column_variant.eq(value.into()))
                }
                pub fn not_equals<T: Into<#field_type>>(value: T) -> Condition {
                    Condition::all().add(<Entity as sea_orm::EntityTrait>::Column::#column_variant.ne(value.into()))
                }
                pub fn gt<T: Into<#field_type>>(value: T) -> Condition {
                    Condition::all().add(<Entity as sea_orm::EntityTrait>::Column::#column_variant.gt(value.into()))
                }
                pub fn lt<T: Into<#field_type>>(value: T) -> Condition {
                    Condition::all().add(<Entity as sea_orm::EntityTrait>::Column::#column_variant.lt(value.into()))
                }
                pub fn gte<T: Into<#field_type>>(value: T) -> Condition {
                    Condition::all().add(<Entity as sea_orm::EntityTrait>::Column::#column_variant.gte(value.into()))
                }
                pub fn lte<T: Into<#field_type>>(value: T) -> Condition {
                    Condition::all().add(<Entity as sea_orm::EntityTrait>::Column::#column_variant.lte(value.into()))
                }
                pub fn in_vec<T: Into<#field_type>>(values: Vec<T>) -> Condition {
                    Condition::all().add(<Entity as sea_orm::EntityTrait>::Column::#column_variant.is_in(values.into_iter().map(|v| v.into()).collect::<Vec<_>>()))
                }
                pub fn not_in_vec<T: Into<#field_type>>(values: Vec<T>) -> Condition {
                    Condition::all().add(<Entity as sea_orm::EntityTrait>::Column::#column_variant.is_not_in(values.into_iter().map(|v| v.into()).collect::<Vec<_>>()))
                }
            }
        }
    });

    let expanded = quote! {
        pub struct EntityClient {
            db: sea_orm::DatabaseConnection,
        }

        impl EntityClient {
            pub fn new(db: sea_orm::DatabaseConnection) -> Self {
                Self { db }
            }
            pub fn db(&self) -> &sea_orm::DatabaseConnection {
                &self.db
            }
            pub async fn find_unique(&self, condition: sea_orm::Condition) -> Result<Option<Model>, sea_orm::DbErr> {
                <Entity as sea_orm::EntityTrait>::find().filter(condition).one(&self.db).await
            }
            pub async fn find_first(&self, conditions: Vec<sea_orm::Condition>) -> Result<Option<Model>, sea_orm::DbErr> {
                let mut query = <Entity as sea_orm::EntityTrait>::find();
                for cond in conditions {
                    query = query.filter(cond);
                }
                query.one(&self.db).await
            }
            pub async fn find_many(&self, conditions: Vec<sea_orm::Condition>) -> Result<Vec<Model>, sea_orm::DbErr> {
                let mut query = <Entity as sea_orm::EntityTrait>::find();
                for cond in conditions {
                    query = query.filter(cond);
                }
                query.all(&self.db).await
            }
        }

        #(#field_ops)*
    };

    expanded.into()
}