use proc_macro2::TokenStream;
use quote::quote;
use lazy_static::lazy_static;
use parking_lot::Mutex;
use heck::ToPascalCase;
use std::collections::HashSet;

// Store entity names for client generation
lazy_static! {
    static ref ENTITIES: Mutex<HashSet<String>> = Mutex::new(HashSet::new());
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