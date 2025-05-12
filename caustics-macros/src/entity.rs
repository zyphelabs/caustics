use proc_macro2::TokenStream;
use quote::{quote, format_ident};
use syn::{DeriveInput, Data, Fields};
use heck::ToPascalCase;
use crate::common::is_option;

pub fn generate_entity(ast: DeriveInput) -> TokenStream {
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

    // Generate struct fields for required fields (with pub)
    let required_struct_fields = required_fields
        .iter()
        .map(|field| {
            let ty = &field.ty;
            let name = field.ident.as_ref().unwrap();
            quote! { pub #name: #ty }
        })
        .collect::<Vec<_>>();

    // Generate function arguments for required fields (no pub)
    let required_fn_args = required_fields
        .iter()
        .map(|field| {
            let ty = &field.ty;
            let name = field.ident.as_ref().unwrap();
            quote! { #name: #ty }
        })
        .collect::<Vec<_>>();

    // Generate initializers for required fields (no pub)
    let required_inits = required_fields
        .iter()
        .map(|field| {
            let name = field.ident.as_ref().unwrap();
            quote! { #name }
        })
        .collect::<Vec<_>>();

    // Generate assignments for required fields (self.#name)
    let required_assigns = required_fields
        .iter()
        .map(|field| {
            let name = field.ident.as_ref().unwrap();
            quote! { model.#name = sea_orm::ActiveValue::Set(self.#name); }
        })
        .collect::<Vec<_>>();

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
        })
        .collect::<Vec<_>>();

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
        })
        .collect::<Vec<_>>();

    // Generate field operator modules (including primary keys for query operations)
    let field_ops = fields.iter().map(|field| {
        let field_name = &field.ident;
        let field_type = &field.ty;
        let pascal_name = format_ident!("{}", field_name.as_ref().unwrap().to_string().to_pascal_case());
        let is_primary_key = primary_key_fields.iter().any(|pk_field| {
            pk_field.ident.as_ref().unwrap() == field_name.as_ref().unwrap()
        });
        
        let set_fn = if !is_primary_key {
            quote! {
                pub fn set<T: Into<#field_type>>(value: T) -> SetValue {
                    SetValue::#pascal_name(sea_orm::ActiveValue::Set(value.into()))
                }
            }
        } else {
            quote! {}
        };
        let order_fn = quote! {
            pub fn order(order: super::super::SortOrder) -> (<Entity as EntityTrait>::Column, super::super::SortOrder) {
                (<Entity as EntityTrait>::Column::#pascal_name, order)
            }
        };
        quote! {
            pub mod #field_name {
                use super::{Entity, Model, ActiveModel, SetValue};
                use sea_orm::{Condition, ColumnTrait, EntityTrait, ActiveValue};
                use chrono::{NaiveDate, NaiveDateTime, DateTime, FixedOffset};
                use uuid::Uuid;
                use std::vec::Vec;
                use super::super::SortOrder;

                #set_fn
                #order_fn

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

    let expanded = {
        let required_struct_fields = required_struct_fields.clone();
        let required_fn_args = required_fn_args.clone();
        let required_inits = required_inits.clone();
        let required_assigns = required_assigns.clone();
        quote! {
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
                IntoActiveModel,
                ConnectionTrait,
            };
            use std::marker::PhantomData;
            use super::SortOrder;

            pub struct EntityClient<'a, C: ConnectionTrait> {
                conn: &'a C
            }

            pub enum SetValue {
                #(#field_variants,)*
            }

            impl SetValue {
                fn merge_into(&self, model: &mut ActiveModel) {
                    match self {
                        #(#match_arms,)*
                    }
                }
            }

            pub struct Create {
                #(#required_struct_fields,)*
                pub _params: Vec<SetValue>,
            }

            impl Create {
                pub fn new(#(#required_fn_args,)* _params: Vec<SetValue>) -> Self {
                    Self {
                        #(#required_inits,)*
                        _params,
                    }
                }

                fn into_active_model(self) -> ActiveModel {
                    let mut model = ActiveModel::new();
                    #(#required_assigns)*
                    for opt in self._params {
                        opt.merge_into(&mut model);
                    }
                    model
                }
            }

            pub struct UniqueQueryBuilder<'a, C: ConnectionTrait> {
                query: sea_orm::Select<Entity>,
                conn: &'a C,
            }

            impl<'a, C: ConnectionTrait> UniqueQueryBuilder<'a, C> {
                pub async fn exec(self) -> Result<Option<Model>, sea_orm::DbErr> {
                    self.query.one(self.conn).await
                }
            }

            pub struct FirstQueryBuilder<'a, C: ConnectionTrait> {
                query: sea_orm::Select<Entity>,
                db: &'a C,
            }

            impl<'a, C: ConnectionTrait> FirstQueryBuilder<'a, C> {
                pub async fn exec(self) -> Result<Option<Model>, sea_orm::DbErr> {
                    self.query.one(self.db).await
                }
            }

            pub struct ManyQueryBuilder<'a, C: ConnectionTrait> {
                query: sea_orm::Select<Entity>,
                db: &'a C,
            }

            impl<'a, C: ConnectionTrait> ManyQueryBuilder<'a, C> {
                pub fn take(mut self, limit: u64) -> Self {
                    self.query = self.query.limit(limit);
                    self
                }
                pub fn skip(mut self, offset: u64) -> Self {
                    self.query = self.query.offset(offset);
                    self
                }
                pub fn order_by<Col>(mut self, col_and_order: (Col, SortOrder)) -> Self 
                where 
                    Col: sea_orm::ColumnTrait + sea_orm::IntoSimpleExpr
                {
                    let (col, sort_order) = col_and_order;
                    let order = match sort_order {
                        SortOrder::Asc => sea_orm::Order::Asc,
                        SortOrder::Desc => sea_orm::Order::Desc,
                    };
                    self.query = self.query.order_by(col, order);
                    self
                }
                pub async fn exec(self) -> Result<Vec<Model>, sea_orm::DbErr> {
                    self.query.all(self.db).await
                }
            }

            pub struct CreateQueryBuilder<'a, C: ConnectionTrait> {
                model: ActiveModel,
                db: &'a C,
            }

            impl<'a, C: ConnectionTrait> CreateQueryBuilder<'a, C> {
                pub async fn exec(self) -> Result<Model, sea_orm::DbErr> {
                    self.model.insert(self.db).await
                }
            }

            pub struct DeleteQueryBuilder<'a, C: ConnectionTrait> {
                condition: Condition,
                db: &'a C,
            }

            impl<'a, C: ConnectionTrait> DeleteQueryBuilder<'a, C> {
                pub async fn exec(self) -> Result<(), sea_orm::DbErr> {
                    Entity::delete_many()
                        .filter(self.condition)
                        .exec(self.db)
                        .await?;
                    Ok(())
                }
            }

            pub struct UpsertQueryBuilder<'a, C: ConnectionTrait> {
                condition: Condition,
                create: Create,
                update: Vec<SetValue>,
                db: &'a C,
            }

            impl<'a, C: ConnectionTrait> UpsertQueryBuilder<'a, C> {
                pub async fn exec(self) -> Result<Model, sea_orm::DbErr> {
                    let existing = Entity::find()
                        .filter(self.condition.clone())
                        .one(self.db)
                        .await?;

                    match existing {
                        Some(model) => {
                            let mut active_model = model.into_active_model();
                            for change in self.update {
                                change.merge_into(&mut active_model);
                            }
                            active_model.update(self.db).await
                        }
                        None => {
                            let mut active_model = self.create.into_active_model();
                            for change in self.update {
                                change.merge_into(&mut active_model);
                            }
                            active_model.insert(self.db).await
                        }
                    }
                }
            }

            pub struct UpdateQueryBuilder<'a, C: ConnectionTrait> {
                condition: Condition,
                changes: Vec<SetValue>,
                db: &'a C,
            }

            impl<'a, C: ConnectionTrait> UpdateQueryBuilder<'a, C> {
                pub async fn exec(self) -> Result<Model, sea_orm::DbErr> {
                    let mut entity = <Entity as EntityTrait>::find().filter(self.condition).one(self.db).await?;
                    if let Some(mut model) = entity.map(|m| m.into_active_model()) {
                        for change in self.changes {
                            change.merge_into(&mut model);
                        }
                        model.update(self.db).await
                    } else {
                        Err(sea_orm::DbErr::RecordNotFound("No record found to update".to_string()))
                    }
                }
            }

            impl<'a, C: ConnectionTrait> EntityClient<'a, C> {
                pub fn new(conn: &'a C) -> Self {
                    Self { conn }
                }

                pub fn find_unique(&self, condition: Condition) -> UniqueQueryBuilder<'a, C> {
                    UniqueQueryBuilder {
                        query: <Entity as EntityTrait>::find().filter(condition),
                        conn: self.conn,
                    }
                }

                pub fn find_first(&self, conditions: Vec<Condition>) -> FirstQueryBuilder<'a, C> {
                    let mut query = <Entity as EntityTrait>::find();
                    for cond in conditions {
                        query = query.filter(cond);
                    }
                    FirstQueryBuilder {
                        query,
                        db: self.conn,
                    }
                }

                pub fn find_many(&self, conditions: Vec<Condition>) -> ManyQueryBuilder<'a, C> {
                    let mut query = <Entity as EntityTrait>::find();
                    for cond in conditions {
                        query = query.filter(cond);
                    }
                    ManyQueryBuilder {
                        query,
                        db: self.conn,
                    }
                }

                pub fn create(&self, #(#required_fn_args,)* _params: Vec<SetValue>) -> CreateQueryBuilder<'a, C> {
                    let create = Create::new(#(#required_inits,)* _params);
                    CreateQueryBuilder {
                        model: create.into_active_model(),
                        db: self.conn,
                    }
                }

                pub fn update(&self, condition: Condition, changes: Vec<SetValue>) -> UpdateQueryBuilder<'a, C> {
                    UpdateQueryBuilder {
                        condition,
                        changes,
                        db: self.conn,
                    }
                }

                pub fn delete(&self, condition: Condition) -> DeleteQueryBuilder<'a, C> {
                    DeleteQueryBuilder {
                        condition,
                        db: self.conn,
                    }
                }

                pub fn upsert(&self, condition: Condition, create: Create, update: Vec<SetValue>) -> UpsertQueryBuilder<'a, C> {
                    UpsertQueryBuilder {
                        condition,
                        create,
                        update,
                        db: self.conn,
                    }
                }
            }

            #(#field_ops)*
        }
    };
    TokenStream::from(expanded)
} 