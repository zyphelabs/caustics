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

    // Generate field operator modules
    let field_ops = fields.iter().map(|field| {
        let field_name = &field.ident;
        let field_type = &field.ty;
        let column_variant = format_ident!("{}", field_name.as_ref().unwrap().to_string().to_pascal_case());
        quote! {
            pub mod #field_name {
                use super::{Entity, Model, ActiveModel};
                use sea_orm::{Condition, ColumnTrait, EntityTrait};
                use chrono::{NaiveDate, NaiveDateTime, DateTime, FixedOffset};
                use uuid::Uuid;
                // For binary types
                use std::vec::Vec;
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
        use sea_orm::{DatabaseConnection, Condition, EntityTrait};

        pub struct EntityClient {
            db: DatabaseConnection,
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
        }
        #(#field_ops)*
    };

    TokenStream::from(expanded)
}

fn generate_entity_client(entity_name: &str) -> proc_macro2::TokenStream {
    let entity_ident = format_ident!("{}", entity_name);
    let entity_client = format_ident!("{}Client", entity_name);
    
    quote! {
        pub struct #entity_client {
            db: sea_orm::DatabaseConnection,
        }

        impl #entity_client {
            pub fn new(db: sea_orm::DatabaseConnection) -> Self {
                Self { db }
            }

            pub async fn find_unique(&self, condition: sea_orm::Condition) -> Result<Option<#entity_ident::Model>, sea_orm::DbErr> {
                let query = sea_orm::QuerySelect::query(
                    &mut sea_orm::Query::select()
                        .from(#entity_ident::Entity)
                        .cond_where(condition)
                        .limit(1)
                );
                
                let result = self.db.query_one(&query).await?;
                Ok(result.map(|row| #entity_ident::Model::from_row(row)))
            }

            pub async fn find_first(&self, conditions: Vec<sea_orm::Condition>) -> Result<Option<#entity_ident::Model>, sea_orm::DbErr> {
                let mut query = sea_orm::Query::select()
                    .from(#entity_ident::Entity)
                    .limit(1);
                
                for condition in conditions {
                    query = query.cond_where(condition);
                }
                
                let result = self.db.query_one(&query).await?;
                Ok(result.map(|row| #entity_ident::Model::from_row(row)))
            }

            pub async fn find_many(&self, conditions: Vec<sea_orm::Condition>) -> Result<Vec<#entity_ident::Model>, sea_orm::DbErr> {
                let mut query = sea_orm::Query::select()
                    .from(#entity_ident::Entity);
                
                for condition in conditions {
                    query = query.cond_where(condition);
                }
                
                let results = self.db.query_all(&query).await?;
                Ok(results.into_iter().map(|row| #entity_ident::Model::from_row(row)).collect())
            }
        }
    }
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

#[proc_macro]
pub fn generate_client(_input: TokenStream) -> TokenStream {
    let expanded = generate_client_impl();
    TokenStream::from(expanded)
}