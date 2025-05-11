#![crate_type = "proc-macro"]

use proc_macro::TokenStream;
use quote::{quote, format_ident};
use syn::{parse_macro_input, DeriveInput, Data, Fields, Type};
use std::sync::Mutex;
use std::collections::HashSet;
use heck::ToPascalCase;

lazy_static::lazy_static! {
    static ref ENTITIES: Mutex<HashSet<String>> = Mutex::new(HashSet::new());
}

fn is_option(ty: &Type) -> bool {
    if let syn::Type::Path(type_path) = ty {
        if let Some(segment) = type_path.path.segments.first() {
            return segment.ident == "Option";
        }
    }
    false
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

    // Only non-nullable, non-primary-key fields are required
    let required_fields: Vec<_> = fields
        .iter()
        .filter(|field| {
            !primary_key_fields.contains(field) && !is_option(&field.ty)
        })
        .collect();

    let required_args = required_fields
        .iter()
        .enumerate()
        .map(|(i, field)| {
            let ty = &field.ty;
            let arg_name = format_ident!("arg{}", i);
            quote! { #arg_name: RequiredSetValue<#ty> }
        });

    let required_names = required_fields
        .iter()
        .enumerate()
        .map(|(i, field)| {
            let name = field.ident.as_ref().unwrap();
            let pascal_name = format_ident!("{}", name.to_string().to_pascal_case());
            let arg_name = format_ident!("arg{}", i);
            let required_fields_str = required_fields
                .iter()
                .map(|f| f.ident.as_ref().unwrap().to_string())
                .collect::<Vec<_>>()
                .join(", ");
            quote! { 
                #name: match #arg_name.0 {
                    SetValue::#pascal_name(value) => value,
                    other => panic!("Expected SetValue::{} but got {:?}. Make sure required fields are passed in the correct order: {}", 
                        stringify!(#pascal_name), 
                        other,
                        #required_fields_str
                    )
                }
            }
        });

    let match_arms = fields
        .iter()
        .filter(|field| !primary_key_fields.contains(field))
        .map(|field| {
            let name = field.ident.as_ref().unwrap();
            let pascal_name = format_ident!("{}", name.to_string().to_pascal_case());
            quote! {
                SetValue::#pascal_name(value) => {
                    model.#name = value.clone();
                }
            }
        });

    // Generate field variants for SetValue enum (excluding primary keys)
    let field_variants = fields
        .iter()
        .filter(|field| !primary_key_fields.contains(field))
        .map(|field| {
            let name = field.ident.as_ref().unwrap();
            let pascal_name = format_ident!("{}", name.to_string().to_pascal_case());
            let ty = &field.ty;
            quote! {
                #pascal_name(sea_orm::ActiveValue<#ty>)
            }
        });

    // Generate field operator modules (including primary keys for query operations)
    let field_ops = fields.iter().map(|field| {
        let field_name = &field.ident;
        let field_type = &field.ty;
        let pascal_name = format_ident!("{}", field_name.as_ref().unwrap().to_string().to_pascal_case());
        let is_primary_key = primary_key_fields.iter().any(|pk_field| {
            pk_field.ident.as_ref().unwrap() == field_name.as_ref().unwrap()
        });
        let is_required = !is_primary_key && !is_option(field_type);
        
        let set_fn = if !is_primary_key {
            if is_required {
                quote! {
                    pub fn set<T: Into<#field_type>>(value: T) -> RequiredSetValue<#field_type> {
                        RequiredSetValue(SetValue::#pascal_name(sea_orm::ActiveValue::Set(value.into())), std::marker::PhantomData)
                    }
                }
            } else {
                quote! {
                    pub fn set<T: Into<#field_type>>(value: T) -> SetValue {
                        SetValue::#pascal_name(sea_orm::ActiveValue::Set(value.into()))
                    }
                }
            }
        } else {
            quote! {}
        };
        
        quote! {
            pub mod #field_name {
                use super::{Entity, Model, ActiveModel, SetValue, RequiredSetValue};
                use sea_orm::{Condition, ColumnTrait, EntityTrait, ActiveValue};
                use chrono::{NaiveDate, NaiveDateTime, DateTime, FixedOffset};
                use uuid::Uuid;
                use std::vec::Vec;

                #set_fn

                pub fn equals<T: Into<#field_type>>(value: T) -> Condition {
                    Condition::all().add(<Entity as EntityTrait>::Column::#pascal_name.eq(value.into()))
                }
                pub fn not_equals<T: Into<#field_type>>(value: T) -> Condition {
                    Condition::all().add(<Entity as EntityTrait>::Column::#pascal_name.ne(value.into()))
                }
                pub fn gt<T: Into<#field_type>>(value: T) -> Condition {
                    Condition::all().add(<Entity as EntityTrait>::Column::#pascal_name.gt(value.into()))
                }
                pub fn lt<T: Into<#field_type>>(value: T) -> Condition {
                    Condition::all().add(<Entity as EntityTrait>::Column::#pascal_name.lt(value.into()))
                }
                pub fn gte<T: Into<#field_type>>(value: T) -> Condition {
                    Condition::all().add(<Entity as EntityTrait>::Column::#pascal_name.gte(value.into()))
                }
                pub fn lte<T: Into<#field_type>>(value: T) -> Condition {
                    Condition::all().add(<Entity as EntityTrait>::Column::#pascal_name.lte(value.into()))
                }
                pub fn in_vec<T: Into<#field_type>>(values: Vec<T>) -> Condition {
                    Condition::all().add(<Entity as EntityTrait>::Column::#pascal_name.is_in(values.into_iter().map(|v| v.into()).collect::<Vec<_>>()))
                }
                pub fn not_in_vec<T: Into<#field_type>>(values: Vec<T>) -> Condition {
                    Condition::all().add(<Entity as EntityTrait>::Column::#pascal_name.is_not_in(values.into_iter().map(|v| v.into()).collect::<Vec<_>>()))
                }
            }
        }
    });

    let expanded = quote! {
        use sea_orm::{
            DatabaseConnection, 
            Condition, 
            EntityTrait, 
            ActiveValue, 
            QuerySelect, 
            QueryOrder,
            QueryTrait,
            Select,
            ColumnTrait,
            IntoSimpleExpr,
        };
        use std::marker::PhantomData;

        pub struct EntityClient {
            db: DatabaseConnection,
        }

        // Type-safe wrapper for required field values
        pub struct RequiredSetValue<T>(SetValue, std::marker::PhantomData<T>);

        // Query builder for non-final operations
        pub struct QueryBuilder {
            query: Select<Entity>,
            db: DatabaseConnection,
            query_type: QueryType,
        }

        #[derive(Clone, Copy)]
        pub enum QueryType {
            Unique,
            First,
            Many,
        }

        impl QueryBuilder {
            pub fn take(mut self, limit: u64) -> Self {
                self.query = self.query.limit(limit);
                self
            }

            pub fn skip(mut self, offset: u64) -> Self {
                self.query = self.query.offset(offset);
                self
            }

            pub async fn exec(self) -> Result<QueryResult, sea_orm::DbErr> {
                match self.query_type {
                    QueryType::Unique | QueryType::First => {
                        let result = self.query.one(&self.db).await?;
                        Ok(QueryResult::Single(result))
                    }
                    QueryType::Many => {
                        let result = self.query.all(&self.db).await?;
                        Ok(QueryResult::Many(result))
                    }
                }
            }
        }

        pub enum QueryResult {
            Single(Option<Model>),
            Many(Vec<Model>),
        }

        // Enum to handle different Set types
        #[derive(Debug)]
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

        pub struct UniqueQueryBuilder {
            query: sea_orm::Select<Entity>,
            db: DatabaseConnection,
        }
        pub struct FirstQueryBuilder {
            query: sea_orm::Select<Entity>,
            db: DatabaseConnection,
        }
        pub struct ManyQueryBuilder {
            query: sea_orm::Select<Entity>,
            db: DatabaseConnection,
        }

        impl UniqueQueryBuilder {
            pub async fn exec(self) -> Result<Option<Model>, sea_orm::DbErr> {
                self.query.one(&self.db).await
            }
        }
        impl FirstQueryBuilder {
            pub async fn exec(self) -> Result<Option<Model>, sea_orm::DbErr> {
                self.query.one(&self.db).await
            }
        }
        impl ManyQueryBuilder {
            pub fn take(mut self, limit: u64) -> Self {
                self.query = self.query.limit(limit);
                self
            }
            pub fn skip(mut self, offset: u64) -> Self {
                self.query = self.query.offset(offset);
                self
            }
            pub async fn exec(self) -> Result<Vec<Model>, sea_orm::DbErr> {
                self.query.all(&self.db).await
            }
        }

        pub struct CreateQueryBuilder {
            model: ActiveModel,
            db: DatabaseConnection,
        }

        impl CreateQueryBuilder {
            pub async fn exec(self) -> Result<Model, sea_orm::DbErr> {
                self.model.insert(&self.db).await
            }
        }

        impl EntityClient {
            pub fn new(db: DatabaseConnection) -> Self {
                Self { db }
            }
            pub fn db(&self) -> &DatabaseConnection {
                &self.db
            }
            pub fn find_unique(&self, condition: Condition) -> UniqueQueryBuilder {
                UniqueQueryBuilder {
                    query: <Entity as EntityTrait>::find().filter(condition),
                    db: self.db.clone(),
                }
            }
            pub fn find_first(&self, conditions: Vec<Condition>) -> FirstQueryBuilder {
                let mut query = <Entity as EntityTrait>::find();
                for cond in conditions {
                    query = query.filter(cond);
                }
                FirstQueryBuilder {
                    query,
                    db: self.db.clone(),
                }
            }
            pub fn find_many(&self, conditions: Vec<Condition>) -> ManyQueryBuilder {
                let mut query = <Entity as EntityTrait>::find();
                for cond in conditions {
                    query = query.filter(cond);
                }
                ManyQueryBuilder {
                    query,
                    db: self.db.clone(),
                }
            }
            pub fn create(&self, #(#required_args,)* optional: Vec<SetValue>) -> CreateQueryBuilder {
                let mut model = ActiveModel {
                    #(#required_names,)*
                    ..Default::default()
                };
                for opt in optional {
                    opt.merge_into(&mut model);
                }
                CreateQueryBuilder {
                    model,
                    db: self.db.clone(),
                }
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