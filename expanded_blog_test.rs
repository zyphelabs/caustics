#![feature(prelude_import)]
#[prelude_import]
use std::prelude::rust_2021::*;
#[macro_use]
extern crate std;
use sea_orm::{DatabaseConnection, DatabaseTransaction, TransactionTrait};
use caustics::{EntityRegistry, EntityFetcher};
#[allow(dead_code)]
pub struct CausticsClient {
    db: std::sync::Arc<DatabaseConnection>,
}
#[allow(dead_code)]
pub struct TransactionCausticsClient {
    tx: std::sync::Arc<DatabaseTransaction>,
}
pub struct TransactionBuilder {
    db: std::sync::Arc<DatabaseConnection>,
}
pub struct CompositeEntityRegistry;
impl<C: sea_orm::ConnectionTrait> EntityRegistry<C> for CompositeEntityRegistry {
    fn get_fetcher(&self, entity_name: &str) -> Option<&dyn EntityFetcher<C>> {
        match entity_name {
            "user" => Some(&user::EntityFetcherImpl),
            "post" => Some(&post::EntityFetcherImpl),
            _ => None,
        }
    }
}
impl<C: sea_orm::ConnectionTrait> EntityRegistry<C>
for &'static CompositeEntityRegistry {
    fn get_fetcher(&self, entity_name: &str) -> Option<&dyn EntityFetcher<C>> {
        (**self).get_fetcher(entity_name)
    }
}
static REGISTRY: CompositeEntityRegistry = CompositeEntityRegistry;
pub fn get_registry() -> &'static CompositeEntityRegistry {
    &REGISTRY
}
#[allow(dead_code)]
impl CausticsClient {
    pub fn new(db: DatabaseConnection) -> Self {
        Self {
            db: std::sync::Arc::new(db),
        }
    }
    pub fn db(&self) -> std::sync::Arc<DatabaseConnection> {
        self.db.clone()
    }
    pub fn _transaction(&self) -> TransactionBuilder {
        TransactionBuilder {
            db: self.db.clone(),
        }
    }
    pub async fn _batch<'a, Entity, ActiveModel, ModelWithRelations, T, Container>(
        &self,
        queries: Container,
    ) -> Result<Container::ReturnType, sea_orm::DbErr>
    where
        Entity: sea_orm::EntityTrait,
        ActiveModel: sea_orm::ActiveModelTrait<Entity = Entity>
            + sea_orm::ActiveModelBehavior + Send + 'static,
        ModelWithRelations: caustics::FromModel<<Entity as sea_orm::EntityTrait>::Model>,
        T: caustics::MergeInto<ActiveModel>,
        <Entity as sea_orm::EntityTrait>::Model: sea_orm::IntoActiveModel<ActiveModel>,
        Container: caustics::BatchContainer<
            'a,
            sea_orm::DatabaseConnection,
            Entity,
            ActiveModel,
            ModelWithRelations,
            T,
        >,
    {
        let txn = self.db.begin().await?;
        let batch_queries = Container::into_queries(queries);
        let mut results = Vec::with_capacity(batch_queries.len());
        for query in batch_queries {
            let res = match query {
                caustics::BatchQuery::Insert(q) => {
                    let result = q.exec_in_txn(&txn).await?;
                    caustics::BatchResult::Insert(result)
                }
                caustics::BatchQuery::Update(q) => {
                    let result = q.exec_in_txn(&txn).await?;
                    caustics::BatchResult::Update(result)
                }
                caustics::BatchQuery::Delete(q) => {
                    q.exec_in_txn(&txn).await?;
                    caustics::BatchResult::Delete(())
                }
                caustics::BatchQuery::Upsert(q) => {
                    let result = q.exec_in_txn(&txn).await?;
                    caustics::BatchResult::Upsert(result)
                }
            };
            results.push(res);
        }
        txn.commit().await?;
        Ok(Container::from_results(results))
    }
    pub fn user(&self) -> user::EntityClient<'_, DatabaseConnection> {
        user::EntityClient::new(&*self.db)
    }
    pub fn post(&self) -> post::EntityClient<'_, DatabaseConnection> {
        post::EntityClient::new(&*self.db)
    }
}
#[allow(dead_code)]
impl TransactionCausticsClient {
    pub fn new(tx: std::sync::Arc<DatabaseTransaction>) -> Self {
        Self { tx }
    }
    pub fn user(&self) -> user::EntityClient<'_, DatabaseTransaction> {
        user::EntityClient::new(&*self.tx)
    }
    pub fn post(&self) -> post::EntityClient<'_, DatabaseTransaction> {
        post::EntityClient::new(&*self.tx)
    }
}
impl TransactionBuilder {
    pub async fn run<F, Fut, T>(&self, f: F) -> Result<T, sea_orm::DbErr>
    where
        F: FnOnce(TransactionCausticsClient) -> Fut,
        Fut: std::future::Future<Output = Result<T, sea_orm::DbErr>>,
    {
        let tx = self.db.begin().await?;
        let tx_arc = std::sync::Arc::new(tx);
        let tx_client = TransactionCausticsClient::new(tx_arc.clone());
        let result = f(tx_client).await;
        let tx = std::sync::Arc::try_unwrap(tx_arc)
            .expect("Transaction Arc should be unique");
        match result {
            Ok(val) => {
                tx.commit().await?;
                Ok(val)
            }
            Err(e) => {
                tx.rollback().await?;
                Err(e)
            }
        }
    }
}
use caustics_macros::caustics;
pub mod user {
    use caustics_macros::Caustics;
    use sea_orm::entity::prelude::*;
    #[sea_orm(table_name = "users")]
    pub struct Model {
        #[sea_orm(primary_key)]
        pub id: i32,
        #[sea_orm(unique)]
        pub email: String,
        pub name: String,
        #[sea_orm(nullable)]
        pub age: Option<i32>,
        pub created_at: DateTime<FixedOffset>,
        pub updated_at: DateTime<FixedOffset>,
        #[sea_orm(nullable)]
        pub deleted_at: Option<DateTime<FixedOffset>>,
    }
    #[automatically_derived]
    impl ::core::clone::Clone for Model {
        #[inline]
        fn clone(&self) -> Model {
            Model {
                id: ::core::clone::Clone::clone(&self.id),
                email: ::core::clone::Clone::clone(&self.email),
                name: ::core::clone::Clone::clone(&self.name),
                age: ::core::clone::Clone::clone(&self.age),
                created_at: ::core::clone::Clone::clone(&self.created_at),
                updated_at: ::core::clone::Clone::clone(&self.updated_at),
                deleted_at: ::core::clone::Clone::clone(&self.deleted_at),
            }
        }
    }
    #[automatically_derived]
    impl ::core::fmt::Debug for Model {
        #[inline]
        fn fmt(&self, f: &mut ::core::fmt::Formatter) -> ::core::fmt::Result {
            let names: &'static _ = &[
                "id",
                "email",
                "name",
                "age",
                "created_at",
                "updated_at",
                "deleted_at",
            ];
            let values: &[&dyn ::core::fmt::Debug] = &[
                &self.id,
                &self.email,
                &self.name,
                &self.age,
                &self.created_at,
                &self.updated_at,
                &&self.deleted_at,
            ];
            ::core::fmt::Formatter::debug_struct_fields_finish(f, "Model", names, values)
        }
    }
    #[automatically_derived]
    impl ::core::marker::StructuralPartialEq for Model {}
    #[automatically_derived]
    impl ::core::cmp::PartialEq for Model {
        #[inline]
        fn eq(&self, other: &Model) -> bool {
            self.id == other.id && self.email == other.email && self.name == other.name
                && self.age == other.age && self.created_at == other.created_at
                && self.updated_at == other.updated_at
                && self.deleted_at == other.deleted_at
        }
    }
    /// Generated by sea-orm-macros
    pub enum Column {
        /// Generated by sea-orm-macros
        Id,
        /// Generated by sea-orm-macros
        Email,
        /// Generated by sea-orm-macros
        Name,
        /// Generated by sea-orm-macros
        Age,
        /// Generated by sea-orm-macros
        CreatedAt,
        /// Generated by sea-orm-macros
        UpdatedAt,
        /// Generated by sea-orm-macros
        DeletedAt,
    }
    #[automatically_derived]
    impl ::core::marker::Copy for Column {}
    #[automatically_derived]
    impl ::core::clone::Clone for Column {
        #[inline]
        fn clone(&self) -> Column {
            *self
        }
    }
    #[automatically_derived]
    impl ::core::fmt::Debug for Column {
        #[inline]
        fn fmt(&self, f: &mut ::core::fmt::Formatter) -> ::core::fmt::Result {
            ::core::fmt::Formatter::write_str(
                f,
                match self {
                    Column::Id => "Id",
                    Column::Email => "Email",
                    Column::Name => "Name",
                    Column::Age => "Age",
                    Column::CreatedAt => "CreatedAt",
                    Column::UpdatedAt => "UpdatedAt",
                    Column::DeletedAt => "DeletedAt",
                },
            )
        }
    }
    ///An iterator over the variants of [Column]
    #[allow(missing_copy_implementations)]
    pub struct ColumnIter {
        idx: usize,
        back_idx: usize,
        marker: ::core::marker::PhantomData<()>,
    }
    impl core::fmt::Debug for ColumnIter {
        fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
            f.debug_struct("ColumnIter").field("len", &self.len()).finish()
        }
    }
    impl ColumnIter {
        fn get(&self, idx: usize) -> Option<Column> {
            match idx {
                0usize => ::core::option::Option::Some(Column::Id),
                1usize => ::core::option::Option::Some(Column::Email),
                2usize => ::core::option::Option::Some(Column::Name),
                3usize => ::core::option::Option::Some(Column::Age),
                4usize => ::core::option::Option::Some(Column::CreatedAt),
                5usize => ::core::option::Option::Some(Column::UpdatedAt),
                6usize => ::core::option::Option::Some(Column::DeletedAt),
                _ => ::core::option::Option::None,
            }
        }
    }
    impl sea_orm::strum::IntoEnumIterator for Column {
        type Iterator = ColumnIter;
        fn iter() -> ColumnIter {
            ColumnIter {
                idx: 0,
                back_idx: 0,
                marker: ::core::marker::PhantomData,
            }
        }
    }
    impl Iterator for ColumnIter {
        type Item = Column;
        fn next(&mut self) -> Option<<Self as Iterator>::Item> {
            self.nth(0)
        }
        fn size_hint(&self) -> (usize, Option<usize>) {
            let t = if self.idx + self.back_idx >= 7usize {
                0
            } else {
                7usize - self.idx - self.back_idx
            };
            (t, Some(t))
        }
        fn nth(&mut self, n: usize) -> Option<<Self as Iterator>::Item> {
            let idx = self.idx + n + 1;
            if idx + self.back_idx > 7usize {
                self.idx = 7usize;
                ::core::option::Option::None
            } else {
                self.idx = idx;
                self.get(idx - 1)
            }
        }
    }
    impl ExactSizeIterator for ColumnIter {
        fn len(&self) -> usize {
            self.size_hint().0
        }
    }
    impl DoubleEndedIterator for ColumnIter {
        fn next_back(&mut self) -> Option<<Self as Iterator>::Item> {
            let back_idx = self.back_idx + 1;
            if self.idx + back_idx > 7usize {
                self.back_idx = 7usize;
                ::core::option::Option::None
            } else {
                self.back_idx = back_idx;
                self.get(7usize - self.back_idx)
            }
        }
    }
    impl Clone for ColumnIter {
        fn clone(&self) -> ColumnIter {
            ColumnIter {
                idx: self.idx,
                back_idx: self.back_idx,
                marker: self.marker.clone(),
            }
        }
    }
    #[automatically_derived]
    impl Column {
        fn default_as_str(&self) -> &str {
            match self {
                Self::Id => "id",
                Self::Email => "email",
                Self::Name => "name",
                Self::Age => "age",
                Self::CreatedAt => "created_at",
                Self::UpdatedAt => "updated_at",
                Self::DeletedAt => "deleted_at",
            }
        }
    }
    #[automatically_derived]
    impl std::str::FromStr for Column {
        type Err = sea_orm::ColumnFromStrErr;
        fn from_str(s: &str) -> std::result::Result<Self, Self::Err> {
            match s {
                "id" | "id" => Ok(Column::Id),
                "email" | "email" => Ok(Column::Email),
                "name" | "name" => Ok(Column::Name),
                "age" | "age" => Ok(Column::Age),
                "created_at" | "createdAt" => Ok(Column::CreatedAt),
                "updated_at" | "updatedAt" => Ok(Column::UpdatedAt),
                "deleted_at" | "deletedAt" => Ok(Column::DeletedAt),
                _ => Err(sea_orm::ColumnFromStrErr(s.to_owned())),
            }
        }
    }
    #[automatically_derived]
    impl sea_orm::Iden for Column {
        fn unquoted(&self, s: &mut dyn std::fmt::Write) {
            s.write_fmt(format_args!("{0}", sea_orm::IdenStatic::as_str(self))).unwrap();
        }
    }
    #[automatically_derived]
    impl sea_orm::IdenStatic for Column {
        fn as_str(&self) -> &str {
            self.default_as_str()
        }
    }
    #[automatically_derived]
    impl sea_orm::prelude::ColumnTrait for Column {
        type EntityName = Entity;
        fn def(&self) -> sea_orm::prelude::ColumnDef {
            match self {
                Self::Id => {
                    sea_orm::prelude::ColumnTypeTrait::def(
                        sea_orm::prelude::ColumnType::Integer,
                    )
                }
                Self::Email => {
                    sea_orm::prelude::ColumnTypeTrait::def(
                            sea_orm::prelude::ColumnType::String(None),
                        )
                        .unique()
                }
                Self::Name => {
                    sea_orm::prelude::ColumnTypeTrait::def(
                        sea_orm::prelude::ColumnType::String(None),
                    )
                }
                Self::Age => {
                    sea_orm::prelude::ColumnTypeTrait::def(
                            sea_orm::prelude::ColumnType::Integer,
                        )
                        .nullable()
                }
                Self::CreatedAt => {
                    sea_orm::prelude::ColumnTypeTrait::def(
                        std::convert::Into::<
                            sea_orm::sea_query::ColumnType,
                        >::into(
                            <DateTime<
                                FixedOffset,
                            > as sea_orm::sea_query::ValueType>::column_type(),
                        ),
                    )
                }
                Self::UpdatedAt => {
                    sea_orm::prelude::ColumnTypeTrait::def(
                        std::convert::Into::<
                            sea_orm::sea_query::ColumnType,
                        >::into(
                            <DateTime<
                                FixedOffset,
                            > as sea_orm::sea_query::ValueType>::column_type(),
                        ),
                    )
                }
                Self::DeletedAt => {
                    sea_orm::prelude::ColumnTypeTrait::def(
                            std::convert::Into::<
                                sea_orm::sea_query::ColumnType,
                            >::into(
                                <DateTime<
                                    FixedOffset,
                                > as sea_orm::sea_query::ValueType>::column_type(),
                            ),
                        )
                        .nullable()
                }
            }
        }
        fn select_as(
            &self,
            expr: sea_orm::sea_query::Expr,
        ) -> sea_orm::sea_query::SimpleExpr {
            match self {
                _ => sea_orm::prelude::ColumnTrait::select_enum_as(self, expr),
            }
        }
        fn save_as(
            &self,
            val: sea_orm::sea_query::Expr,
        ) -> sea_orm::sea_query::SimpleExpr {
            match self {
                _ => sea_orm::prelude::ColumnTrait::save_enum_as(self, val),
            }
        }
    }
    /// Generated by sea-orm-macros
    pub struct Entity;
    #[automatically_derived]
    impl ::core::marker::Copy for Entity {}
    #[automatically_derived]
    impl ::core::clone::Clone for Entity {
        #[inline]
        fn clone(&self) -> Entity {
            *self
        }
    }
    #[automatically_derived]
    impl ::core::default::Default for Entity {
        #[inline]
        fn default() -> Entity {
            Entity {}
        }
    }
    #[automatically_derived]
    impl ::core::fmt::Debug for Entity {
        #[inline]
        fn fmt(&self, f: &mut ::core::fmt::Formatter) -> ::core::fmt::Result {
            ::core::fmt::Formatter::write_str(f, "Entity")
        }
    }
    #[automatically_derived]
    impl sea_orm::entity::EntityTrait for Entity {
        type Model = Model;
        type Column = Column;
        type PrimaryKey = PrimaryKey;
        type Relation = Relation;
    }
    #[automatically_derived]
    impl sea_orm::Iden for Entity {
        fn unquoted(&self, s: &mut dyn std::fmt::Write) {
            s.write_fmt(format_args!("{0}", sea_orm::IdenStatic::as_str(self))).unwrap();
        }
    }
    #[automatically_derived]
    impl sea_orm::IdenStatic for Entity {
        fn as_str(&self) -> &str {
            <Self as sea_orm::EntityName>::table_name(self)
        }
    }
    #[automatically_derived]
    impl sea_orm::prelude::EntityName for Entity {
        fn schema_name(&self) -> Option<&str> {
            None
        }
        fn table_name(&self) -> &str {
            "users"
        }
        fn comment(&self) -> Option<&str> {
            None
        }
    }
    /// Generated by sea-orm-macros
    pub enum PrimaryKey {
        /// Generated by sea-orm-macros
        Id,
    }
    #[automatically_derived]
    impl ::core::marker::Copy for PrimaryKey {}
    #[automatically_derived]
    impl ::core::clone::Clone for PrimaryKey {
        #[inline]
        fn clone(&self) -> PrimaryKey {
            *self
        }
    }
    #[automatically_derived]
    impl ::core::fmt::Debug for PrimaryKey {
        #[inline]
        fn fmt(&self, f: &mut ::core::fmt::Formatter) -> ::core::fmt::Result {
            ::core::fmt::Formatter::write_str(f, "Id")
        }
    }
    ///An iterator over the variants of [PrimaryKey]
    #[allow(missing_copy_implementations)]
    pub struct PrimaryKeyIter {
        idx: usize,
        back_idx: usize,
        marker: ::core::marker::PhantomData<()>,
    }
    impl core::fmt::Debug for PrimaryKeyIter {
        fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
            f.debug_struct("PrimaryKeyIter").field("len", &self.len()).finish()
        }
    }
    impl PrimaryKeyIter {
        fn get(&self, idx: usize) -> Option<PrimaryKey> {
            match idx {
                0usize => ::core::option::Option::Some(PrimaryKey::Id),
                _ => ::core::option::Option::None,
            }
        }
    }
    impl sea_orm::strum::IntoEnumIterator for PrimaryKey {
        type Iterator = PrimaryKeyIter;
        fn iter() -> PrimaryKeyIter {
            PrimaryKeyIter {
                idx: 0,
                back_idx: 0,
                marker: ::core::marker::PhantomData,
            }
        }
    }
    impl Iterator for PrimaryKeyIter {
        type Item = PrimaryKey;
        fn next(&mut self) -> Option<<Self as Iterator>::Item> {
            self.nth(0)
        }
        fn size_hint(&self) -> (usize, Option<usize>) {
            let t = if self.idx + self.back_idx >= 1usize {
                0
            } else {
                1usize - self.idx - self.back_idx
            };
            (t, Some(t))
        }
        fn nth(&mut self, n: usize) -> Option<<Self as Iterator>::Item> {
            let idx = self.idx + n + 1;
            if idx + self.back_idx > 1usize {
                self.idx = 1usize;
                ::core::option::Option::None
            } else {
                self.idx = idx;
                self.get(idx - 1)
            }
        }
    }
    impl ExactSizeIterator for PrimaryKeyIter {
        fn len(&self) -> usize {
            self.size_hint().0
        }
    }
    impl DoubleEndedIterator for PrimaryKeyIter {
        fn next_back(&mut self) -> Option<<Self as Iterator>::Item> {
            let back_idx = self.back_idx + 1;
            if self.idx + back_idx > 1usize {
                self.back_idx = 1usize;
                ::core::option::Option::None
            } else {
                self.back_idx = back_idx;
                self.get(1usize - self.back_idx)
            }
        }
    }
    impl Clone for PrimaryKeyIter {
        fn clone(&self) -> PrimaryKeyIter {
            PrimaryKeyIter {
                idx: self.idx,
                back_idx: self.back_idx,
                marker: self.marker.clone(),
            }
        }
    }
    #[automatically_derived]
    impl sea_orm::Iden for PrimaryKey {
        fn unquoted(&self, s: &mut dyn std::fmt::Write) {
            s.write_fmt(format_args!("{0}", sea_orm::IdenStatic::as_str(self))).unwrap();
        }
    }
    #[automatically_derived]
    impl sea_orm::IdenStatic for PrimaryKey {
        fn as_str(&self) -> &str {
            match self {
                Self::Id => "id",
            }
        }
    }
    #[automatically_derived]
    impl sea_orm::PrimaryKeyToColumn for PrimaryKey {
        type Column = Column;
        fn into_column(self) -> Self::Column {
            match self {
                Self::Id => Self::Column::Id,
            }
        }
        fn from_column(col: Self::Column) -> Option<Self> {
            match col {
                Self::Column::Id => Some(Self::Id),
                _ => None,
            }
        }
    }
    #[automatically_derived]
    impl PrimaryKeyTrait for PrimaryKey {
        type ValueType = i32;
        fn auto_increment() -> bool {
            true
        }
    }
    #[automatically_derived]
    impl sea_orm::FromQueryResult for Model {
        fn from_query_result(
            row: &sea_orm::QueryResult,
            pre: &str,
        ) -> std::result::Result<Self, sea_orm::DbErr> {
            Ok(Self {
                id: row
                    .try_get(
                        pre,
                        sea_orm::IdenStatic::as_str(
                                &<<Self as sea_orm::ModelTrait>::Entity as sea_orm::entity::EntityTrait>::Column::Id,
                            )
                            .into(),
                    )?,
                email: row
                    .try_get(
                        pre,
                        sea_orm::IdenStatic::as_str(
                                &<<Self as sea_orm::ModelTrait>::Entity as sea_orm::entity::EntityTrait>::Column::Email,
                            )
                            .into(),
                    )?,
                name: row
                    .try_get(
                        pre,
                        sea_orm::IdenStatic::as_str(
                                &<<Self as sea_orm::ModelTrait>::Entity as sea_orm::entity::EntityTrait>::Column::Name,
                            )
                            .into(),
                    )?,
                age: row
                    .try_get(
                        pre,
                        sea_orm::IdenStatic::as_str(
                                &<<Self as sea_orm::ModelTrait>::Entity as sea_orm::entity::EntityTrait>::Column::Age,
                            )
                            .into(),
                    )?,
                created_at: row
                    .try_get(
                        pre,
                        sea_orm::IdenStatic::as_str(
                                &<<Self as sea_orm::ModelTrait>::Entity as sea_orm::entity::EntityTrait>::Column::CreatedAt,
                            )
                            .into(),
                    )?,
                updated_at: row
                    .try_get(
                        pre,
                        sea_orm::IdenStatic::as_str(
                                &<<Self as sea_orm::ModelTrait>::Entity as sea_orm::entity::EntityTrait>::Column::UpdatedAt,
                            )
                            .into(),
                    )?,
                deleted_at: row
                    .try_get(
                        pre,
                        sea_orm::IdenStatic::as_str(
                                &<<Self as sea_orm::ModelTrait>::Entity as sea_orm::entity::EntityTrait>::Column::DeletedAt,
                            )
                            .into(),
                    )?,
            })
        }
    }
    #[automatically_derived]
    impl sea_orm::ModelTrait for Model {
        type Entity = Entity;
        fn get(
            &self,
            c: <Self::Entity as sea_orm::entity::EntityTrait>::Column,
        ) -> sea_orm::Value {
            match c {
                <Self::Entity as sea_orm::entity::EntityTrait>::Column::Id => {
                    self.id.clone().into()
                }
                <Self::Entity as sea_orm::entity::EntityTrait>::Column::Email => {
                    self.email.clone().into()
                }
                <Self::Entity as sea_orm::entity::EntityTrait>::Column::Name => {
                    self.name.clone().into()
                }
                <Self::Entity as sea_orm::entity::EntityTrait>::Column::Age => {
                    self.age.clone().into()
                }
                <Self::Entity as sea_orm::entity::EntityTrait>::Column::CreatedAt => {
                    self.created_at.clone().into()
                }
                <Self::Entity as sea_orm::entity::EntityTrait>::Column::UpdatedAt => {
                    self.updated_at.clone().into()
                }
                <Self::Entity as sea_orm::entity::EntityTrait>::Column::DeletedAt => {
                    self.deleted_at.clone().into()
                }
                _ => {
                    ::core::panicking::panic_fmt(
                        format_args!("field does not exist on Model"),
                    );
                }
            }
        }
        fn set(
            &mut self,
            c: <Self::Entity as sea_orm::entity::EntityTrait>::Column,
            v: sea_orm::Value,
        ) {
            match c {
                <Self::Entity as sea_orm::entity::EntityTrait>::Column::Id => {
                    self.id = v.unwrap();
                }
                <Self::Entity as sea_orm::entity::EntityTrait>::Column::Email => {
                    self.email = v.unwrap();
                }
                <Self::Entity as sea_orm::entity::EntityTrait>::Column::Name => {
                    self.name = v.unwrap();
                }
                <Self::Entity as sea_orm::entity::EntityTrait>::Column::Age => {
                    self.age = v.unwrap();
                }
                <Self::Entity as sea_orm::entity::EntityTrait>::Column::CreatedAt => {
                    self.created_at = v.unwrap();
                }
                <Self::Entity as sea_orm::entity::EntityTrait>::Column::UpdatedAt => {
                    self.updated_at = v.unwrap();
                }
                <Self::Entity as sea_orm::entity::EntityTrait>::Column::DeletedAt => {
                    self.deleted_at = v.unwrap();
                }
                _ => {
                    ::core::panicking::panic_fmt(
                        format_args!("field does not exist on Model"),
                    );
                }
            }
        }
    }
    /// Generated by sea-orm-macros
    pub struct ActiveModel {
        /// Generated by sea-orm-macros
        pub id: sea_orm::ActiveValue<i32>,
        /// Generated by sea-orm-macros
        pub email: sea_orm::ActiveValue<String>,
        /// Generated by sea-orm-macros
        pub name: sea_orm::ActiveValue<String>,
        /// Generated by sea-orm-macros
        pub age: sea_orm::ActiveValue<Option<i32>>,
        /// Generated by sea-orm-macros
        pub created_at: sea_orm::ActiveValue<DateTime<FixedOffset>>,
        /// Generated by sea-orm-macros
        pub updated_at: sea_orm::ActiveValue<DateTime<FixedOffset>>,
        /// Generated by sea-orm-macros
        pub deleted_at: sea_orm::ActiveValue<Option<DateTime<FixedOffset>>>,
    }
    #[automatically_derived]
    impl ::core::clone::Clone for ActiveModel {
        #[inline]
        fn clone(&self) -> ActiveModel {
            ActiveModel {
                id: ::core::clone::Clone::clone(&self.id),
                email: ::core::clone::Clone::clone(&self.email),
                name: ::core::clone::Clone::clone(&self.name),
                age: ::core::clone::Clone::clone(&self.age),
                created_at: ::core::clone::Clone::clone(&self.created_at),
                updated_at: ::core::clone::Clone::clone(&self.updated_at),
                deleted_at: ::core::clone::Clone::clone(&self.deleted_at),
            }
        }
    }
    #[automatically_derived]
    impl ::core::fmt::Debug for ActiveModel {
        #[inline]
        fn fmt(&self, f: &mut ::core::fmt::Formatter) -> ::core::fmt::Result {
            let names: &'static _ = &[
                "id",
                "email",
                "name",
                "age",
                "created_at",
                "updated_at",
                "deleted_at",
            ];
            let values: &[&dyn ::core::fmt::Debug] = &[
                &self.id,
                &self.email,
                &self.name,
                &self.age,
                &self.created_at,
                &self.updated_at,
                &&self.deleted_at,
            ];
            ::core::fmt::Formatter::debug_struct_fields_finish(
                f,
                "ActiveModel",
                names,
                values,
            )
        }
    }
    #[automatically_derived]
    impl ::core::marker::StructuralPartialEq for ActiveModel {}
    #[automatically_derived]
    impl ::core::cmp::PartialEq for ActiveModel {
        #[inline]
        fn eq(&self, other: &ActiveModel) -> bool {
            self.id == other.id && self.email == other.email && self.name == other.name
                && self.age == other.age && self.created_at == other.created_at
                && self.updated_at == other.updated_at
                && self.deleted_at == other.deleted_at
        }
    }
    #[automatically_derived]
    impl std::default::Default for ActiveModel {
        fn default() -> Self {
            <Self as sea_orm::ActiveModelBehavior>::new()
        }
    }
    #[automatically_derived]
    impl std::convert::From<<Entity as EntityTrait>::Model> for ActiveModel {
        fn from(m: <Entity as EntityTrait>::Model) -> Self {
            Self {
                id: sea_orm::ActiveValue::unchanged(m.id),
                email: sea_orm::ActiveValue::unchanged(m.email),
                name: sea_orm::ActiveValue::unchanged(m.name),
                age: sea_orm::ActiveValue::unchanged(m.age),
                created_at: sea_orm::ActiveValue::unchanged(m.created_at),
                updated_at: sea_orm::ActiveValue::unchanged(m.updated_at),
                deleted_at: sea_orm::ActiveValue::unchanged(m.deleted_at),
            }
        }
    }
    #[automatically_derived]
    impl sea_orm::IntoActiveModel<ActiveModel> for <Entity as EntityTrait>::Model {
        fn into_active_model(self) -> ActiveModel {
            self.into()
        }
    }
    #[automatically_derived]
    impl sea_orm::ActiveModelTrait for ActiveModel {
        type Entity = Entity;
        fn take(
            &mut self,
            c: <Self::Entity as EntityTrait>::Column,
        ) -> sea_orm::ActiveValue<sea_orm::Value> {
            match c {
                <Self::Entity as EntityTrait>::Column::Id => {
                    let mut value = sea_orm::ActiveValue::not_set();
                    std::mem::swap(&mut value, &mut self.id);
                    value.into_wrapped_value()
                }
                <Self::Entity as EntityTrait>::Column::Email => {
                    let mut value = sea_orm::ActiveValue::not_set();
                    std::mem::swap(&mut value, &mut self.email);
                    value.into_wrapped_value()
                }
                <Self::Entity as EntityTrait>::Column::Name => {
                    let mut value = sea_orm::ActiveValue::not_set();
                    std::mem::swap(&mut value, &mut self.name);
                    value.into_wrapped_value()
                }
                <Self::Entity as EntityTrait>::Column::Age => {
                    let mut value = sea_orm::ActiveValue::not_set();
                    std::mem::swap(&mut value, &mut self.age);
                    value.into_wrapped_value()
                }
                <Self::Entity as EntityTrait>::Column::CreatedAt => {
                    let mut value = sea_orm::ActiveValue::not_set();
                    std::mem::swap(&mut value, &mut self.created_at);
                    value.into_wrapped_value()
                }
                <Self::Entity as EntityTrait>::Column::UpdatedAt => {
                    let mut value = sea_orm::ActiveValue::not_set();
                    std::mem::swap(&mut value, &mut self.updated_at);
                    value.into_wrapped_value()
                }
                <Self::Entity as EntityTrait>::Column::DeletedAt => {
                    let mut value = sea_orm::ActiveValue::not_set();
                    std::mem::swap(&mut value, &mut self.deleted_at);
                    value.into_wrapped_value()
                }
                _ => sea_orm::ActiveValue::not_set(),
            }
        }
        fn get(
            &self,
            c: <Self::Entity as EntityTrait>::Column,
        ) -> sea_orm::ActiveValue<sea_orm::Value> {
            match c {
                <Self::Entity as EntityTrait>::Column::Id => {
                    self.id.clone().into_wrapped_value()
                }
                <Self::Entity as EntityTrait>::Column::Email => {
                    self.email.clone().into_wrapped_value()
                }
                <Self::Entity as EntityTrait>::Column::Name => {
                    self.name.clone().into_wrapped_value()
                }
                <Self::Entity as EntityTrait>::Column::Age => {
                    self.age.clone().into_wrapped_value()
                }
                <Self::Entity as EntityTrait>::Column::CreatedAt => {
                    self.created_at.clone().into_wrapped_value()
                }
                <Self::Entity as EntityTrait>::Column::UpdatedAt => {
                    self.updated_at.clone().into_wrapped_value()
                }
                <Self::Entity as EntityTrait>::Column::DeletedAt => {
                    self.deleted_at.clone().into_wrapped_value()
                }
                _ => sea_orm::ActiveValue::not_set(),
            }
        }
        fn set(&mut self, c: <Self::Entity as EntityTrait>::Column, v: sea_orm::Value) {
            match c {
                <Self::Entity as EntityTrait>::Column::Id => {
                    self.id = sea_orm::ActiveValue::set(v.unwrap());
                }
                <Self::Entity as EntityTrait>::Column::Email => {
                    self.email = sea_orm::ActiveValue::set(v.unwrap());
                }
                <Self::Entity as EntityTrait>::Column::Name => {
                    self.name = sea_orm::ActiveValue::set(v.unwrap());
                }
                <Self::Entity as EntityTrait>::Column::Age => {
                    self.age = sea_orm::ActiveValue::set(v.unwrap());
                }
                <Self::Entity as EntityTrait>::Column::CreatedAt => {
                    self.created_at = sea_orm::ActiveValue::set(v.unwrap());
                }
                <Self::Entity as EntityTrait>::Column::UpdatedAt => {
                    self.updated_at = sea_orm::ActiveValue::set(v.unwrap());
                }
                <Self::Entity as EntityTrait>::Column::DeletedAt => {
                    self.deleted_at = sea_orm::ActiveValue::set(v.unwrap());
                }
                _ => {
                    ::core::panicking::panic_fmt(
                        format_args!("This ActiveModel does not have this field"),
                    );
                }
            }
        }
        fn not_set(&mut self, c: <Self::Entity as EntityTrait>::Column) {
            match c {
                <Self::Entity as EntityTrait>::Column::Id => {
                    self.id = sea_orm::ActiveValue::not_set();
                }
                <Self::Entity as EntityTrait>::Column::Email => {
                    self.email = sea_orm::ActiveValue::not_set();
                }
                <Self::Entity as EntityTrait>::Column::Name => {
                    self.name = sea_orm::ActiveValue::not_set();
                }
                <Self::Entity as EntityTrait>::Column::Age => {
                    self.age = sea_orm::ActiveValue::not_set();
                }
                <Self::Entity as EntityTrait>::Column::CreatedAt => {
                    self.created_at = sea_orm::ActiveValue::not_set();
                }
                <Self::Entity as EntityTrait>::Column::UpdatedAt => {
                    self.updated_at = sea_orm::ActiveValue::not_set();
                }
                <Self::Entity as EntityTrait>::Column::DeletedAt => {
                    self.deleted_at = sea_orm::ActiveValue::not_set();
                }
                _ => {}
            }
        }
        fn is_not_set(&self, c: <Self::Entity as EntityTrait>::Column) -> bool {
            match c {
                <Self::Entity as EntityTrait>::Column::Id => self.id.is_not_set(),
                <Self::Entity as EntityTrait>::Column::Email => self.email.is_not_set(),
                <Self::Entity as EntityTrait>::Column::Name => self.name.is_not_set(),
                <Self::Entity as EntityTrait>::Column::Age => self.age.is_not_set(),
                <Self::Entity as EntityTrait>::Column::CreatedAt => {
                    self.created_at.is_not_set()
                }
                <Self::Entity as EntityTrait>::Column::UpdatedAt => {
                    self.updated_at.is_not_set()
                }
                <Self::Entity as EntityTrait>::Column::DeletedAt => {
                    self.deleted_at.is_not_set()
                }
                _ => {
                    ::core::panicking::panic_fmt(
                        format_args!("This ActiveModel does not have this field"),
                    );
                }
            }
        }
        fn default() -> Self {
            Self {
                id: sea_orm::ActiveValue::not_set(),
                email: sea_orm::ActiveValue::not_set(),
                name: sea_orm::ActiveValue::not_set(),
                age: sea_orm::ActiveValue::not_set(),
                created_at: sea_orm::ActiveValue::not_set(),
                updated_at: sea_orm::ActiveValue::not_set(),
                deleted_at: sea_orm::ActiveValue::not_set(),
            }
        }
        fn reset(&mut self, c: <Self::Entity as EntityTrait>::Column) {
            match c {
                <Self::Entity as EntityTrait>::Column::Id => self.id.reset(),
                <Self::Entity as EntityTrait>::Column::Email => self.email.reset(),
                <Self::Entity as EntityTrait>::Column::Name => self.name.reset(),
                <Self::Entity as EntityTrait>::Column::Age => self.age.reset(),
                <Self::Entity as EntityTrait>::Column::CreatedAt => {
                    self.created_at.reset()
                }
                <Self::Entity as EntityTrait>::Column::UpdatedAt => {
                    self.updated_at.reset()
                }
                <Self::Entity as EntityTrait>::Column::DeletedAt => {
                    self.deleted_at.reset()
                }
                _ => {
                    ::core::panicking::panic_fmt(
                        format_args!("This ActiveModel does not have this field"),
                    );
                }
            }
        }
    }
    #[automatically_derived]
    impl std::convert::TryFrom<ActiveModel> for <Entity as EntityTrait>::Model {
        type Error = sea_orm::DbErr;
        fn try_from(a: ActiveModel) -> Result<Self, sea_orm::DbErr> {
            if match a.id {
                sea_orm::ActiveValue::NotSet => true,
                _ => false,
            } {
                return Err(sea_orm::DbErr::AttrNotSet("id".to_owned()));
            }
            if match a.email {
                sea_orm::ActiveValue::NotSet => true,
                _ => false,
            } {
                return Err(sea_orm::DbErr::AttrNotSet("email".to_owned()));
            }
            if match a.name {
                sea_orm::ActiveValue::NotSet => true,
                _ => false,
            } {
                return Err(sea_orm::DbErr::AttrNotSet("name".to_owned()));
            }
            if match a.age {
                sea_orm::ActiveValue::NotSet => true,
                _ => false,
            } {
                return Err(sea_orm::DbErr::AttrNotSet("age".to_owned()));
            }
            if match a.created_at {
                sea_orm::ActiveValue::NotSet => true,
                _ => false,
            } {
                return Err(sea_orm::DbErr::AttrNotSet("created_at".to_owned()));
            }
            if match a.updated_at {
                sea_orm::ActiveValue::NotSet => true,
                _ => false,
            } {
                return Err(sea_orm::DbErr::AttrNotSet("updated_at".to_owned()));
            }
            if match a.deleted_at {
                sea_orm::ActiveValue::NotSet => true,
                _ => false,
            } {
                return Err(sea_orm::DbErr::AttrNotSet("deleted_at".to_owned()));
            }
            Ok(Self {
                id: a.id.into_value().unwrap().unwrap(),
                email: a.email.into_value().unwrap().unwrap(),
                name: a.name.into_value().unwrap().unwrap(),
                age: a.age.into_value().unwrap().unwrap(),
                created_at: a.created_at.into_value().unwrap().unwrap(),
                updated_at: a.updated_at.into_value().unwrap().unwrap(),
                deleted_at: a.deleted_at.into_value().unwrap().unwrap(),
            })
        }
    }
    #[automatically_derived]
    impl sea_orm::TryIntoModel<<Entity as EntityTrait>::Model> for ActiveModel {
        fn try_into_model(
            self,
        ) -> Result<<Entity as EntityTrait>::Model, sea_orm::DbErr> {
            self.try_into()
        }
    }
    pub enum Relation {
        #[sea_orm(
            has_many = "super::post::Entity",
            from = "Column::Id",
            to = "super::post::Column::UserId"
        )]
        Posts,
    }
    #[automatically_derived]
    impl ::core::marker::Copy for Relation {}
    #[automatically_derived]
    impl ::core::clone::Clone for Relation {
        #[inline]
        fn clone(&self) -> Relation {
            *self
        }
    }
    #[automatically_derived]
    impl ::core::fmt::Debug for Relation {
        #[inline]
        fn fmt(&self, f: &mut ::core::fmt::Formatter) -> ::core::fmt::Result {
            ::core::fmt::Formatter::write_str(f, "Posts")
        }
    }
    ///An iterator over the variants of [Relation]
    #[allow(missing_copy_implementations)]
    pub struct RelationIter {
        idx: usize,
        back_idx: usize,
        marker: ::core::marker::PhantomData<()>,
    }
    impl core::fmt::Debug for RelationIter {
        fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
            f.debug_struct("RelationIter").field("len", &self.len()).finish()
        }
    }
    impl RelationIter {
        fn get(&self, idx: usize) -> Option<Relation> {
            match idx {
                0usize => ::core::option::Option::Some(Relation::Posts),
                _ => ::core::option::Option::None,
            }
        }
    }
    impl sea_orm::strum::IntoEnumIterator for Relation {
        type Iterator = RelationIter;
        fn iter() -> RelationIter {
            RelationIter {
                idx: 0,
                back_idx: 0,
                marker: ::core::marker::PhantomData,
            }
        }
    }
    impl Iterator for RelationIter {
        type Item = Relation;
        fn next(&mut self) -> Option<<Self as Iterator>::Item> {
            self.nth(0)
        }
        fn size_hint(&self) -> (usize, Option<usize>) {
            let t = if self.idx + self.back_idx >= 1usize {
                0
            } else {
                1usize - self.idx - self.back_idx
            };
            (t, Some(t))
        }
        fn nth(&mut self, n: usize) -> Option<<Self as Iterator>::Item> {
            let idx = self.idx + n + 1;
            if idx + self.back_idx > 1usize {
                self.idx = 1usize;
                ::core::option::Option::None
            } else {
                self.idx = idx;
                self.get(idx - 1)
            }
        }
    }
    impl ExactSizeIterator for RelationIter {
        fn len(&self) -> usize {
            self.size_hint().0
        }
    }
    impl DoubleEndedIterator for RelationIter {
        fn next_back(&mut self) -> Option<<Self as Iterator>::Item> {
            let back_idx = self.back_idx + 1;
            if self.idx + back_idx > 1usize {
                self.back_idx = 1usize;
                ::core::option::Option::None
            } else {
                self.back_idx = back_idx;
                self.get(1usize - self.back_idx)
            }
        }
    }
    impl Clone for RelationIter {
        fn clone(&self) -> RelationIter {
            RelationIter {
                idx: self.idx,
                back_idx: self.back_idx,
                marker: self.marker.clone(),
            }
        }
    }
    #[automatically_derived]
    impl sea_orm::entity::RelationTrait for Relation {
        fn def(&self) -> sea_orm::entity::RelationDef {
            match self {
                Self::Posts => {
                    Entity::has_many(super::post::Entity)
                        .from(Column::Id)
                        .to(super::post::Column::UserId)
                        .into()
                }
                _ => {
                    ::core::panicking::panic_fmt(
                        format_args!("No RelationDef for Relation"),
                    );
                }
            }
        }
    }
    impl Related<super::user::Entity> for Entity {
        fn to() -> RelationDef {
            Relation::Posts.def()
        }
    }
    use chrono::{NaiveDate, NaiveDateTime, DateTime, FixedOffset};
    use uuid::Uuid;
    use std::vec::Vec;
    use caustics::{SortOrder, MergeInto};
    use caustics::FromModel;
    use sea_query::{Condition, Expr};
    pub struct EntityClient<'a, C: sea_orm::ConnectionTrait> {
        conn: &'a C,
    }
    pub enum FieldOp<T> {
        Equals(T),
        NotEquals(T),
        Gt(T),
        Lt(T),
        Gte(T),
        Lte(T),
        InVec(Vec<T>),
        NotInVec(Vec<T>),
        Contains(String),
        StartsWith(String),
        EndsWith(String),
    }
    pub enum SetParam {
        Email(sea_orm::ActiveValue<String>),
        Name(sea_orm::ActiveValue<String>),
        Age(sea_orm::ActiveValue<Option<i32>>),
        CreatedAt(sea_orm::ActiveValue<DateTime<FixedOffset>>),
        UpdatedAt(sea_orm::ActiveValue<DateTime<FixedOffset>>),
        DeletedAt(sea_orm::ActiveValue<Option<DateTime<FixedOffset>>>),
        ConnectPosts(Vec<super::post::UniqueWhereParam>),
    }
    pub enum WhereParam {
        Id(FieldOp<i32>),
        Email(FieldOp<String>),
        EmailMode(caustics::QueryMode),
        Name(FieldOp<String>),
        NameMode(caustics::QueryMode),
        Age(FieldOp<Option<i32>>),
        CreatedAt(FieldOp<DateTime<FixedOffset>>),
        UpdatedAt(FieldOp<DateTime<FixedOffset>>),
        DeletedAt(FieldOp<Option<DateTime<FixedOffset>>>),
        And(Vec<super::WhereParam>),
        Or(Vec<super::WhereParam>),
        Not(Vec<super::WhereParam>),
    }
    pub enum OrderByParam {
        Id(caustics::SortOrder),
        Email(caustics::SortOrder),
        Name(caustics::SortOrder),
        Age(caustics::SortOrder),
        CreatedAt(caustics::SortOrder),
        UpdatedAt(caustics::SortOrder),
        DeletedAt(caustics::SortOrder),
    }
    pub enum UniqueWhereParam {
        IdEquals(i32),
        EmailEquals(String),
    }
    #[automatically_derived]
    impl ::core::fmt::Debug for UniqueWhereParam {
        #[inline]
        fn fmt(&self, f: &mut ::core::fmt::Formatter) -> ::core::fmt::Result {
            match self {
                UniqueWhereParam::IdEquals(__self_0) => {
                    ::core::fmt::Formatter::debug_tuple_field1_finish(
                        f,
                        "IdEquals",
                        &__self_0,
                    )
                }
                UniqueWhereParam::EmailEquals(__self_0) => {
                    ::core::fmt::Formatter::debug_tuple_field1_finish(
                        f,
                        "EmailEquals",
                        &__self_0,
                    )
                }
            }
        }
    }
    #[automatically_derived]
    impl ::core::clone::Clone for UniqueWhereParam {
        #[inline]
        fn clone(&self) -> UniqueWhereParam {
            match self {
                UniqueWhereParam::IdEquals(__self_0) => {
                    UniqueWhereParam::IdEquals(::core::clone::Clone::clone(__self_0))
                }
                UniqueWhereParam::EmailEquals(__self_0) => {
                    UniqueWhereParam::EmailEquals(::core::clone::Clone::clone(__self_0))
                }
            }
        }
    }
    #[allow(dead_code)]
    pub mod id {
        use chrono::{NaiveDate, NaiveDateTime, DateTime, FixedOffset};
        use uuid::Uuid;
        use std::vec::Vec;
        use super::*;
        pub struct Equals(pub i32);
        pub fn equals<T: From<Equals>>(value: impl Into<i32>) -> T {
            Equals(value.into()).into()
        }
        impl From<Equals> for super::UniqueWhereParam {
            fn from(Equals(v): Equals) -> Self {
                super::UniqueWhereParam::IdEquals(v)
            }
        }
        impl From<Equals> for super::WhereParam {
            fn from(Equals(v): Equals) -> Self {
                super::WhereParam::Id(FieldOp::Equals(v))
            }
        }
        pub fn order(order: caustics::SortOrder) -> super::OrderByParam {
            super::OrderByParam::Id(order)
        }
        pub fn not_equals<T: Into<i32>>(value: T) -> super::WhereParam {
            super::WhereParam::Id(FieldOp::NotEquals(value.into()))
        }
        pub fn gt<T: Into<i32>>(value: T) -> super::WhereParam {
            super::WhereParam::Id(FieldOp::Gt(value.into()))
        }
        pub fn lt<T: Into<i32>>(value: T) -> super::WhereParam {
            super::WhereParam::Id(FieldOp::Lt(value.into()))
        }
        pub fn gte<T: Into<i32>>(value: T) -> super::WhereParam {
            super::WhereParam::Id(FieldOp::Gte(value.into()))
        }
        pub fn lte<T: Into<i32>>(value: T) -> super::WhereParam {
            super::WhereParam::Id(FieldOp::Lte(value.into()))
        }
        pub fn in_vec<T: Into<i32>>(values: Vec<T>) -> super::WhereParam {
            super::WhereParam::Id(
                FieldOp::InVec(values.into_iter().map(|v| v.into()).collect()),
            )
        }
        pub fn not_in_vec<T: Into<i32>>(values: Vec<T>) -> super::WhereParam {
            super::WhereParam::Id(
                FieldOp::NotInVec(values.into_iter().map(|v| v.into()).collect()),
            )
        }
    }
    #[allow(dead_code)]
    pub mod email {
        use chrono::{NaiveDate, NaiveDateTime, DateTime, FixedOffset};
        use uuid::Uuid;
        use std::vec::Vec;
        use super::*;
        pub struct Equals(pub String);
        pub fn equals<T: From<Equals>>(value: impl Into<String>) -> T {
            Equals(value.into()).into()
        }
        impl From<Equals> for super::UniqueWhereParam {
            fn from(Equals(v): Equals) -> Self {
                super::UniqueWhereParam::EmailEquals(v)
            }
        }
        impl From<Equals> for super::WhereParam {
            fn from(Equals(v): Equals) -> Self {
                super::WhereParam::Email(FieldOp::Equals(v))
            }
        }
        pub fn order(order: caustics::SortOrder) -> super::OrderByParam {
            super::OrderByParam::Email(order)
        }
        pub fn contains<T: Into<String>>(value: T) -> super::WhereParam {
            super::WhereParam::Email(FieldOp::Contains(value.into()))
        }
        pub fn starts_with<T: Into<String>>(value: T) -> super::WhereParam {
            super::WhereParam::Email(FieldOp::StartsWith(value.into()))
        }
        pub fn ends_with<T: Into<String>>(value: T) -> super::WhereParam {
            super::WhereParam::Email(FieldOp::EndsWith(value.into()))
        }
        pub fn not_equals<T: Into<String>>(value: T) -> super::WhereParam {
            super::WhereParam::Email(FieldOp::NotEquals(value.into()))
        }
        pub fn gt<T: Into<String>>(value: T) -> super::WhereParam {
            super::WhereParam::Email(FieldOp::Gt(value.into()))
        }
        pub fn lt<T: Into<String>>(value: T) -> super::WhereParam {
            super::WhereParam::Email(FieldOp::Lt(value.into()))
        }
        pub fn gte<T: Into<String>>(value: T) -> super::WhereParam {
            super::WhereParam::Email(FieldOp::Gte(value.into()))
        }
        pub fn lte<T: Into<String>>(value: T) -> super::WhereParam {
            super::WhereParam::Email(FieldOp::Lte(value.into()))
        }
        pub fn in_vec<T: Into<String>>(values: Vec<T>) -> super::WhereParam {
            super::WhereParam::Email(
                FieldOp::InVec(values.into_iter().map(|v| v.into()).collect()),
            )
        }
        pub fn not_in_vec<T: Into<String>>(values: Vec<T>) -> super::WhereParam {
            super::WhereParam::Email(
                FieldOp::NotInVec(values.into_iter().map(|v| v.into()).collect()),
            )
        }
        pub fn mode(mode: caustics::QueryMode) -> super::WhereParam {
            super::WhereParam::EmailMode(mode)
        }
    }
    #[allow(dead_code)]
    pub mod name {
        use chrono::{NaiveDate, NaiveDateTime, DateTime, FixedOffset};
        use uuid::Uuid;
        use std::vec::Vec;
        use super::*;
        pub fn set<T: Into<String>>(value: T) -> super::SetParam {
            super::SetParam::Name(sea_orm::ActiveValue::Set(value.into()))
        }
        pub fn equals<T: Into<String>>(value: T) -> super::WhereParam {
            super::WhereParam::Name(FieldOp::Equals(value.into()))
        }
        pub fn order(order: caustics::SortOrder) -> super::OrderByParam {
            super::OrderByParam::Name(order)
        }
        pub fn contains<T: Into<String>>(value: T) -> super::WhereParam {
            super::WhereParam::Name(FieldOp::Contains(value.into()))
        }
        pub fn starts_with<T: Into<String>>(value: T) -> super::WhereParam {
            super::WhereParam::Name(FieldOp::StartsWith(value.into()))
        }
        pub fn ends_with<T: Into<String>>(value: T) -> super::WhereParam {
            super::WhereParam::Name(FieldOp::EndsWith(value.into()))
        }
        pub fn not_equals<T: Into<String>>(value: T) -> super::WhereParam {
            super::WhereParam::Name(FieldOp::NotEquals(value.into()))
        }
        pub fn gt<T: Into<String>>(value: T) -> super::WhereParam {
            super::WhereParam::Name(FieldOp::Gt(value.into()))
        }
        pub fn lt<T: Into<String>>(value: T) -> super::WhereParam {
            super::WhereParam::Name(FieldOp::Lt(value.into()))
        }
        pub fn gte<T: Into<String>>(value: T) -> super::WhereParam {
            super::WhereParam::Name(FieldOp::Gte(value.into()))
        }
        pub fn lte<T: Into<String>>(value: T) -> super::WhereParam {
            super::WhereParam::Name(FieldOp::Lte(value.into()))
        }
        pub fn in_vec<T: Into<String>>(values: Vec<T>) -> super::WhereParam {
            super::WhereParam::Name(
                FieldOp::InVec(values.into_iter().map(|v| v.into()).collect()),
            )
        }
        pub fn not_in_vec<T: Into<String>>(values: Vec<T>) -> super::WhereParam {
            super::WhereParam::Name(
                FieldOp::NotInVec(values.into_iter().map(|v| v.into()).collect()),
            )
        }
        pub fn mode(mode: caustics::QueryMode) -> super::WhereParam {
            super::WhereParam::NameMode(mode)
        }
    }
    #[allow(dead_code)]
    pub mod age {
        use chrono::{NaiveDate, NaiveDateTime, DateTime, FixedOffset};
        use uuid::Uuid;
        use std::vec::Vec;
        use super::*;
        pub fn set<T: Into<Option<i32>>>(value: T) -> super::SetParam {
            super::SetParam::Age(sea_orm::ActiveValue::Set(value.into()))
        }
        pub fn equals<T: Into<Option<i32>>>(value: T) -> super::WhereParam {
            super::WhereParam::Age(FieldOp::Equals(value.into()))
        }
        pub fn order(order: caustics::SortOrder) -> super::OrderByParam {
            super::OrderByParam::Age(order)
        }
        pub fn not_equals<T: Into<Option<i32>>>(value: T) -> super::WhereParam {
            super::WhereParam::Age(FieldOp::NotEquals(value.into()))
        }
        pub fn gt<T: Into<Option<i32>>>(value: T) -> super::WhereParam {
            super::WhereParam::Age(FieldOp::Gt(value.into()))
        }
        pub fn lt<T: Into<Option<i32>>>(value: T) -> super::WhereParam {
            super::WhereParam::Age(FieldOp::Lt(value.into()))
        }
        pub fn gte<T: Into<Option<i32>>>(value: T) -> super::WhereParam {
            super::WhereParam::Age(FieldOp::Gte(value.into()))
        }
        pub fn lte<T: Into<Option<i32>>>(value: T) -> super::WhereParam {
            super::WhereParam::Age(FieldOp::Lte(value.into()))
        }
        pub fn in_vec<T: Into<Option<i32>>>(values: Vec<T>) -> super::WhereParam {
            super::WhereParam::Age(
                FieldOp::InVec(values.into_iter().map(|v| v.into()).collect()),
            )
        }
        pub fn not_in_vec<T: Into<Option<i32>>>(values: Vec<T>) -> super::WhereParam {
            super::WhereParam::Age(
                FieldOp::NotInVec(values.into_iter().map(|v| v.into()).collect()),
            )
        }
    }
    #[allow(dead_code)]
    pub mod created_at {
        use chrono::{NaiveDate, NaiveDateTime, DateTime, FixedOffset};
        use uuid::Uuid;
        use std::vec::Vec;
        use super::*;
        pub fn set<T: Into<DateTime<FixedOffset>>>(value: T) -> super::SetParam {
            super::SetParam::CreatedAt(sea_orm::ActiveValue::Set(value.into()))
        }
        pub fn equals<T: Into<DateTime<FixedOffset>>>(value: T) -> super::WhereParam {
            super::WhereParam::CreatedAt(FieldOp::Equals(value.into()))
        }
        pub fn order(order: caustics::SortOrder) -> super::OrderByParam {
            super::OrderByParam::CreatedAt(order)
        }
        pub fn not_equals<T: Into<DateTime<FixedOffset>>>(
            value: T,
        ) -> super::WhereParam {
            super::WhereParam::CreatedAt(FieldOp::NotEquals(value.into()))
        }
        pub fn gt<T: Into<DateTime<FixedOffset>>>(value: T) -> super::WhereParam {
            super::WhereParam::CreatedAt(FieldOp::Gt(value.into()))
        }
        pub fn lt<T: Into<DateTime<FixedOffset>>>(value: T) -> super::WhereParam {
            super::WhereParam::CreatedAt(FieldOp::Lt(value.into()))
        }
        pub fn gte<T: Into<DateTime<FixedOffset>>>(value: T) -> super::WhereParam {
            super::WhereParam::CreatedAt(FieldOp::Gte(value.into()))
        }
        pub fn lte<T: Into<DateTime<FixedOffset>>>(value: T) -> super::WhereParam {
            super::WhereParam::CreatedAt(FieldOp::Lte(value.into()))
        }
        pub fn in_vec<T: Into<DateTime<FixedOffset>>>(
            values: Vec<T>,
        ) -> super::WhereParam {
            super::WhereParam::CreatedAt(
                FieldOp::InVec(values.into_iter().map(|v| v.into()).collect()),
            )
        }
        pub fn not_in_vec<T: Into<DateTime<FixedOffset>>>(
            values: Vec<T>,
        ) -> super::WhereParam {
            super::WhereParam::CreatedAt(
                FieldOp::NotInVec(values.into_iter().map(|v| v.into()).collect()),
            )
        }
    }
    #[allow(dead_code)]
    pub mod updated_at {
        use chrono::{NaiveDate, NaiveDateTime, DateTime, FixedOffset};
        use uuid::Uuid;
        use std::vec::Vec;
        use super::*;
        pub fn set<T: Into<DateTime<FixedOffset>>>(value: T) -> super::SetParam {
            super::SetParam::UpdatedAt(sea_orm::ActiveValue::Set(value.into()))
        }
        pub fn equals<T: Into<DateTime<FixedOffset>>>(value: T) -> super::WhereParam {
            super::WhereParam::UpdatedAt(FieldOp::Equals(value.into()))
        }
        pub fn order(order: caustics::SortOrder) -> super::OrderByParam {
            super::OrderByParam::UpdatedAt(order)
        }
        pub fn not_equals<T: Into<DateTime<FixedOffset>>>(
            value: T,
        ) -> super::WhereParam {
            super::WhereParam::UpdatedAt(FieldOp::NotEquals(value.into()))
        }
        pub fn gt<T: Into<DateTime<FixedOffset>>>(value: T) -> super::WhereParam {
            super::WhereParam::UpdatedAt(FieldOp::Gt(value.into()))
        }
        pub fn lt<T: Into<DateTime<FixedOffset>>>(value: T) -> super::WhereParam {
            super::WhereParam::UpdatedAt(FieldOp::Lt(value.into()))
        }
        pub fn gte<T: Into<DateTime<FixedOffset>>>(value: T) -> super::WhereParam {
            super::WhereParam::UpdatedAt(FieldOp::Gte(value.into()))
        }
        pub fn lte<T: Into<DateTime<FixedOffset>>>(value: T) -> super::WhereParam {
            super::WhereParam::UpdatedAt(FieldOp::Lte(value.into()))
        }
        pub fn in_vec<T: Into<DateTime<FixedOffset>>>(
            values: Vec<T>,
        ) -> super::WhereParam {
            super::WhereParam::UpdatedAt(
                FieldOp::InVec(values.into_iter().map(|v| v.into()).collect()),
            )
        }
        pub fn not_in_vec<T: Into<DateTime<FixedOffset>>>(
            values: Vec<T>,
        ) -> super::WhereParam {
            super::WhereParam::UpdatedAt(
                FieldOp::NotInVec(values.into_iter().map(|v| v.into()).collect()),
            )
        }
    }
    #[allow(dead_code)]
    pub mod deleted_at {
        use chrono::{NaiveDate, NaiveDateTime, DateTime, FixedOffset};
        use uuid::Uuid;
        use std::vec::Vec;
        use super::*;
        pub fn set<T: Into<Option<DateTime<FixedOffset>>>>(value: T) -> super::SetParam {
            super::SetParam::DeletedAt(sea_orm::ActiveValue::Set(value.into()))
        }
        pub fn equals<T: Into<Option<DateTime<FixedOffset>>>>(
            value: T,
        ) -> super::WhereParam {
            super::WhereParam::DeletedAt(FieldOp::Equals(value.into()))
        }
        pub fn order(order: caustics::SortOrder) -> super::OrderByParam {
            super::OrderByParam::DeletedAt(order)
        }
        pub fn not_equals<T: Into<Option<DateTime<FixedOffset>>>>(
            value: T,
        ) -> super::WhereParam {
            super::WhereParam::DeletedAt(FieldOp::NotEquals(value.into()))
        }
        pub fn gt<T: Into<Option<DateTime<FixedOffset>>>>(
            value: T,
        ) -> super::WhereParam {
            super::WhereParam::DeletedAt(FieldOp::Gt(value.into()))
        }
        pub fn lt<T: Into<Option<DateTime<FixedOffset>>>>(
            value: T,
        ) -> super::WhereParam {
            super::WhereParam::DeletedAt(FieldOp::Lt(value.into()))
        }
        pub fn gte<T: Into<Option<DateTime<FixedOffset>>>>(
            value: T,
        ) -> super::WhereParam {
            super::WhereParam::DeletedAt(FieldOp::Gte(value.into()))
        }
        pub fn lte<T: Into<Option<DateTime<FixedOffset>>>>(
            value: T,
        ) -> super::WhereParam {
            super::WhereParam::DeletedAt(FieldOp::Lte(value.into()))
        }
        pub fn in_vec<T: Into<Option<DateTime<FixedOffset>>>>(
            values: Vec<T>,
        ) -> super::WhereParam {
            super::WhereParam::DeletedAt(
                FieldOp::InVec(values.into_iter().map(|v| v.into()).collect()),
            )
        }
        pub fn not_in_vec<T: Into<Option<DateTime<FixedOffset>>>>(
            values: Vec<T>,
        ) -> super::WhereParam {
            super::WhereParam::DeletedAt(
                FieldOp::NotInVec(values.into_iter().map(|v| v.into()).collect()),
            )
        }
    }
    pub fn and(params: Vec<super::WhereParam>) -> super::WhereParam {
        super::WhereParam::And(params)
    }
    pub fn or(params: Vec<super::WhereParam>) -> super::WhereParam {
        super::WhereParam::Or(params)
    }
    pub fn not(params: Vec<super::WhereParam>) -> super::WhereParam {
        super::WhereParam::Not(params)
    }
    impl MergeInto<ActiveModel> for SetParam {
        fn merge_into(&self, model: &mut ActiveModel) {
            match self {
                SetParam::Email(value) => {
                    model.email = value.clone();
                }
                SetParam::Name(value) => {
                    model.name = value.clone();
                }
                SetParam::Age(value) => {
                    model.age = value.clone();
                }
                SetParam::CreatedAt(value) => {
                    model.created_at = value.clone();
                }
                SetParam::UpdatedAt(value) => {
                    model.updated_at = value.clone();
                }
                SetParam::DeletedAt(value) => {
                    model.deleted_at = value.clone();
                }
                _ => {}
            }
        }
    }
    impl From<WhereParam> for Condition {
        fn from(param: WhereParam) -> Self {
            match param {
                _ => ::core::panicking::panic("not yet implemented"),
            }
        }
    }
    impl From<UniqueWhereParam> for Condition {
        fn from(param: UniqueWhereParam) -> Self {
            match param {
                UniqueWhereParam::IdEquals(value) => {
                    Condition::all().add(<Entity as EntityTrait>::Column::Id.eq(value))
                }
                UniqueWhereParam::EmailEquals(value) => {
                    Condition::all()
                        .add(<Entity as EntityTrait>::Column::Email.eq(value))
                }
            }
        }
    }
    impl From<OrderByParam> for (<Entity as EntityTrait>::Column, sea_orm::Order) {
        fn from(param: OrderByParam) -> Self {
            match param {
                OrderByParam::Id(order) => {
                    let sea_order = match order {
                        SortOrder::Asc => sea_orm::Order::Asc,
                        SortOrder::Desc => sea_orm::Order::Desc,
                    };
                    (<Entity as EntityTrait>::Column::Id, sea_order)
                }
                OrderByParam::Email(order) => {
                    let sea_order = match order {
                        SortOrder::Asc => sea_orm::Order::Asc,
                        SortOrder::Desc => sea_orm::Order::Desc,
                    };
                    (<Entity as EntityTrait>::Column::Email, sea_order)
                }
                OrderByParam::Name(order) => {
                    let sea_order = match order {
                        SortOrder::Asc => sea_orm::Order::Asc,
                        SortOrder::Desc => sea_orm::Order::Desc,
                    };
                    (<Entity as EntityTrait>::Column::Name, sea_order)
                }
                OrderByParam::Age(order) => {
                    let sea_order = match order {
                        SortOrder::Asc => sea_orm::Order::Asc,
                        SortOrder::Desc => sea_orm::Order::Desc,
                    };
                    (<Entity as EntityTrait>::Column::Age, sea_order)
                }
                OrderByParam::CreatedAt(order) => {
                    let sea_order = match order {
                        SortOrder::Asc => sea_orm::Order::Asc,
                        SortOrder::Desc => sea_orm::Order::Desc,
                    };
                    (<Entity as EntityTrait>::Column::CreatedAt, sea_order)
                }
                OrderByParam::UpdatedAt(order) => {
                    let sea_order = match order {
                        SortOrder::Asc => sea_orm::Order::Asc,
                        SortOrder::Desc => sea_orm::Order::Desc,
                    };
                    (<Entity as EntityTrait>::Column::UpdatedAt, sea_order)
                }
                OrderByParam::DeletedAt(order) => {
                    let sea_order = match order {
                        SortOrder::Asc => sea_orm::Order::Asc,
                        SortOrder::Desc => sea_orm::Order::Desc,
                    };
                    (<Entity as EntityTrait>::Column::DeletedAt, sea_order)
                }
            }
        }
    }
    pub struct Create {
        pub email: String,
        pub name: String,
        pub created_at: DateTime<FixedOffset>,
        pub updated_at: DateTime<FixedOffset>,
        pub _params: Vec<SetParam>,
    }
    impl Create {
        fn into_active_model<C: sea_orm::ConnectionTrait>(
            mut self,
        ) -> (ActiveModel, Vec<caustics::DeferredLookup<C>>) {
            let mut model = ActiveModel::new();
            let mut deferred_lookups = Vec::new();
            model.email = sea_orm::ActiveValue::Set(self.email);
            model.name = sea_orm::ActiveValue::Set(self.name);
            model.created_at = sea_orm::ActiveValue::Set(self.created_at);
            model.updated_at = sea_orm::ActiveValue::Set(self.updated_at);
            for param in self._params {
                match param {
                    other => {
                        other.merge_into(&mut model);
                    }
                }
            }
            (model, deferred_lookups)
        }
    }
    pub type Filter = caustics::Filter;
    pub struct RelationFilter {
        pub relation: &'static str,
        pub filters: Vec<Filter>,
    }
    #[automatically_derived]
    impl ::core::clone::Clone for RelationFilter {
        #[inline]
        fn clone(&self) -> RelationFilter {
            RelationFilter {
                relation: ::core::clone::Clone::clone(&self.relation),
                filters: ::core::clone::Clone::clone(&self.filters),
            }
        }
    }
    impl caustics::RelationFilterTrait for RelationFilter {
        fn relation_name(&self) -> &'static str {
            self.relation
        }
        fn filters(&self) -> &[caustics::Filter] {
            &self.filters
        }
    }
    impl From<RelationFilter> for caustics::RelationFilter {
        fn from(relation_filter: RelationFilter) -> Self {
            caustics::RelationFilter {
                relation: relation_filter.relation,
                filters: relation_filter.filters,
            }
        }
    }
    pub struct ModelWithRelations {
        pub id: i32,
        pub email: String,
        pub name: String,
        pub age: Option<i32>,
        pub created_at: DateTime<FixedOffset>,
        pub updated_at: DateTime<FixedOffset>,
        pub deleted_at: Option<DateTime<FixedOffset>>,
        pub posts: Option<Vec<super::post::ModelWithRelations>>,
    }
    #[automatically_derived]
    impl ::core::fmt::Debug for ModelWithRelations {
        #[inline]
        fn fmt(&self, f: &mut ::core::fmt::Formatter) -> ::core::fmt::Result {
            let names: &'static _ = &[
                "id",
                "email",
                "name",
                "age",
                "created_at",
                "updated_at",
                "deleted_at",
                "posts",
            ];
            let values: &[&dyn ::core::fmt::Debug] = &[
                &self.id,
                &self.email,
                &self.name,
                &self.age,
                &self.created_at,
                &self.updated_at,
                &self.deleted_at,
                &&self.posts,
            ];
            ::core::fmt::Formatter::debug_struct_fields_finish(
                f,
                "ModelWithRelations",
                names,
                values,
            )
        }
    }
    #[automatically_derived]
    impl ::core::clone::Clone for ModelWithRelations {
        #[inline]
        fn clone(&self) -> ModelWithRelations {
            ModelWithRelations {
                id: ::core::clone::Clone::clone(&self.id),
                email: ::core::clone::Clone::clone(&self.email),
                name: ::core::clone::Clone::clone(&self.name),
                age: ::core::clone::Clone::clone(&self.age),
                created_at: ::core::clone::Clone::clone(&self.created_at),
                updated_at: ::core::clone::Clone::clone(&self.updated_at),
                deleted_at: ::core::clone::Clone::clone(&self.deleted_at),
                posts: ::core::clone::Clone::clone(&self.posts),
            }
        }
    }
    #[automatically_derived]
    impl ::core::marker::StructuralPartialEq for ModelWithRelations {}
    #[automatically_derived]
    impl ::core::cmp::PartialEq for ModelWithRelations {
        #[inline]
        fn eq(&self, other: &ModelWithRelations) -> bool {
            self.id == other.id && self.email == other.email && self.name == other.name
                && self.age == other.age && self.created_at == other.created_at
                && self.updated_at == other.updated_at
                && self.deleted_at == other.deleted_at && self.posts == other.posts
        }
    }
    #[automatically_derived]
    impl ::core::cmp::Eq for ModelWithRelations {
        #[inline]
        #[doc(hidden)]
        #[coverage(off)]
        fn assert_receiver_is_total_eq(&self) -> () {
            let _: ::core::cmp::AssertParamIsEq<i32>;
            let _: ::core::cmp::AssertParamIsEq<String>;
            let _: ::core::cmp::AssertParamIsEq<Option<i32>>;
            let _: ::core::cmp::AssertParamIsEq<DateTime<FixedOffset>>;
            let _: ::core::cmp::AssertParamIsEq<DateTime<FixedOffset>>;
            let _: ::core::cmp::AssertParamIsEq<Option<DateTime<FixedOffset>>>;
            let _: ::core::cmp::AssertParamIsEq<
                Option<Vec<super::post::ModelWithRelations>>,
            >;
        }
    }
    #[automatically_derived]
    impl ::core::hash::Hash for ModelWithRelations {
        #[inline]
        fn hash<__H: ::core::hash::Hasher>(&self, state: &mut __H) -> () {
            ::core::hash::Hash::hash(&self.id, state);
            ::core::hash::Hash::hash(&self.email, state);
            ::core::hash::Hash::hash(&self.name, state);
            ::core::hash::Hash::hash(&self.age, state);
            ::core::hash::Hash::hash(&self.created_at, state);
            ::core::hash::Hash::hash(&self.updated_at, state);
            ::core::hash::Hash::hash(&self.deleted_at, state);
            ::core::hash::Hash::hash(&self.posts, state)
        }
    }
    impl ModelWithRelations {
        pub fn new(
            id: i32,
            email: String,
            name: String,
            age: Option<i32>,
            created_at: DateTime<FixedOffset>,
            updated_at: DateTime<FixedOffset>,
            deleted_at: Option<DateTime<FixedOffset>>,
            posts: Option<Vec<super::post::ModelWithRelations>>,
        ) -> Self {
            Self {
                id,
                email,
                name,
                age,
                created_at,
                updated_at,
                deleted_at,
                posts,
            }
        }
        pub fn from_model(model: Model) -> Self {
            Self {
                id: model.id,
                email: model.email,
                name: model.name,
                age: model.age,
                created_at: model.created_at,
                updated_at: model.updated_at,
                deleted_at: model.deleted_at,
                posts: None,
            }
        }
    }
    impl std::default::Default for ModelWithRelations {
        fn default() -> Self {
            Self {
                id: Default::default(),
                email: Default::default(),
                name: Default::default(),
                age: Default::default(),
                created_at: Default::default(),
                updated_at: Default::default(),
                deleted_at: Default::default(),
                posts: None,
            }
        }
    }
    impl caustics::FromModel<Model> for ModelWithRelations {
        fn from_model(model: Model) -> Self {
            Self::from_model(model)
        }
    }
    static RELATION_DESCRIPTORS: &[caustics::RelationDescriptor<ModelWithRelations>] = &[
        caustics::RelationDescriptor::<ModelWithRelations> {
            name: "posts",
            set_field: |model, value| {
                let value = value
                    .downcast::<Option<Vec<super::post::ModelWithRelations>>>()
                    .expect("Type mismatch in set_field");
                model.posts = *value;
            },
            get_foreign_key: |model| Some(model.id),
            target_entity: "Path { leading_colon: None, segments: [PathSegment { ident: Ident { ident: \"super\", span: #12 bytes(102..133) }, arguments: PathArguments::None }, PathSep, PathSegment { ident: Ident { ident: \"post\", span: #12 bytes(102..133) }, arguments: PathArguments::None }] }",
            foreign_key_column: "id",
        },
    ];
    impl caustics::HasRelationMetadata<ModelWithRelations> for ModelWithRelations {
        fn relation_descriptors() -> &'static [caustics::RelationDescriptor<
            ModelWithRelations,
        >] {
            RELATION_DESCRIPTORS
        }
    }
    #[allow(dead_code)]
    impl<
        'a,
        C: sea_orm::ConnectionTrait + sea_orm::TransactionTrait,
    > EntityClient<'a, C> {
        pub fn new(conn: &'a C) -> Self {
            Self { conn }
        }
        pub fn find_unique(
            &self,
            condition: UniqueWhereParam,
        ) -> caustics::UniqueQueryBuilder<'a, C, Entity, ModelWithRelations> {
            let registry = super::get_registry();
            caustics::UniqueQueryBuilder {
                query: <Entity as EntityTrait>::find()
                    .filter::<Condition>(condition.clone().into()),
                conn: self.conn,
                relations_to_fetch: ::alloc::vec::Vec::new(),
                registry,
                _phantom: std::marker::PhantomData,
            }
        }
        pub fn find_first(
            &self,
            conditions: Vec<WhereParam>,
        ) -> caustics::FirstQueryBuilder<'a, C, Entity, ModelWithRelations> {
            let registry = super::get_registry();
            let mut query = <Entity as EntityTrait>::find();
            for cond in conditions {
                query = query.filter::<Condition>(cond.into());
            }
            caustics::FirstQueryBuilder {
                query,
                conn: self.conn,
                relations_to_fetch: ::alloc::vec::Vec::new(),
                registry,
                _phantom: std::marker::PhantomData,
            }
        }
        pub fn find_many(
            &self,
            conditions: Vec<WhereParam>,
        ) -> caustics::ManyQueryBuilder<'a, C, Entity, ModelWithRelations> {
            let registry = super::get_registry();
            let mut query = <Entity as EntityTrait>::find();
            for cond in conditions {
                query = query.filter::<Condition>(cond.into());
            }
            caustics::ManyQueryBuilder {
                query,
                conn: self.conn,
                relations_to_fetch: ::alloc::vec::Vec::new(),
                registry,
                _phantom: std::marker::PhantomData,
            }
        }
        pub fn create(
            &self,
            email: String,
            name: String,
            created_at: DateTime<FixedOffset>,
            updated_at: DateTime<FixedOffset>,
            _params: Vec<SetParam>,
        ) -> caustics::CreateQueryBuilder<
            'a,
            C,
            Entity,
            ActiveModel,
            ModelWithRelations,
        > {
            let create = Create {
                email,
                name,
                created_at,
                updated_at,
                _params,
            };
            let (model, deferred_lookups) = create.into_active_model::<C>();
            caustics::CreateQueryBuilder {
                model,
                conn: self.conn,
                deferred_lookups,
                _phantom: std::marker::PhantomData,
            }
        }
        pub fn update(
            &self,
            condition: UniqueWhereParam,
            changes: Vec<SetParam>,
        ) -> caustics::UpdateQueryBuilder<
            'a,
            C,
            Entity,
            ActiveModel,
            ModelWithRelations,
            SetParam,
        > {
            caustics::UpdateQueryBuilder {
                condition: condition.into(),
                changes,
                conn: self.conn,
                _phantom: std::marker::PhantomData,
            }
        }
        pub fn delete(
            &self,
            condition: UniqueWhereParam,
        ) -> caustics::DeleteQueryBuilder<'a, C, Entity> {
            caustics::DeleteQueryBuilder {
                condition: condition.into(),
                conn: self.conn,
                _phantom: std::marker::PhantomData,
            }
        }
        pub fn upsert(
            &self,
            condition: UniqueWhereParam,
            create: Create,
            update: Vec<SetParam>,
        ) -> caustics::UpsertQueryBuilder<
            'a,
            C,
            Entity,
            ActiveModel,
            ModelWithRelations,
            SetParam,
        > {
            let (model, deferred_lookups) = create.into_active_model::<C>();
            caustics::UpsertQueryBuilder {
                condition: condition.into(),
                create: (model, deferred_lookups),
                update,
                conn: self.conn,
                _phantom: std::marker::PhantomData,
            }
        }
        pub async fn _batch(
            &self,
            queries: Vec<
                caustics::BatchQuery<
                    'a,
                    sea_orm::DatabaseTransaction,
                    Entity,
                    ActiveModel,
                    ModelWithRelations,
                    SetParam,
                >,
            >,
        ) -> Result<Vec<caustics::BatchResult<ModelWithRelations>>, sea_orm::DbErr>
        where
            Entity: sea_orm::EntityTrait,
            ActiveModel: sea_orm::ActiveModelTrait<Entity = Entity>
                + sea_orm::ActiveModelBehavior + Send + 'static,
            ModelWithRelations: caustics::FromModel<
                <Entity as sea_orm::EntityTrait>::Model,
            >,
            SetParam: caustics::MergeInto<ActiveModel>,
            <Entity as sea_orm::EntityTrait>::Model: sea_orm::IntoActiveModel<
                ActiveModel,
            >,
        {
            let txn = self.conn.begin().await?;
            let mut results = Vec::with_capacity(queries.len());
            for query in queries {
                let res = match query {
                    caustics::BatchQuery::Insert(q) => {
                        let model = q.model;
                        let result = model
                            .insert(&txn)
                            .await
                            .map(ModelWithRelations::from_model)?;
                        caustics::BatchResult::Insert(result)
                    }
                    caustics::BatchQuery::Update(q) => {
                        caustics::BatchResult::Update(ModelWithRelations::default())
                    }
                    caustics::BatchQuery::Delete(q) => caustics::BatchResult::Delete(()),
                    caustics::BatchQuery::Upsert(q) => {
                        caustics::BatchResult::Upsert(ModelWithRelations::default())
                    }
                };
                results.push(res);
            }
            txn.commit().await?;
            Ok(results)
        }
    }
    #[allow(dead_code)]
    pub mod posts {
        pub fn fetch(filters: Vec<super::Filter>) -> super::RelationFilter {
            super::RelationFilter {
                relation: "posts",
                filters,
            }
        }
        pub fn connect(
            params: Vec<super::super::post::UniqueWhereParam>,
        ) -> super::SetParam {
            super::SetParam::ConnectPosts(params)
        }
    }
    pub(crate) fn column_from_str(
        name: &str,
    ) -> Option<<Entity as sea_orm::EntityTrait>::Column> {
        match name {
            "id" => Some(<Entity as sea_orm::EntityTrait>::Column::Id),
            "email" => Some(<Entity as sea_orm::EntityTrait>::Column::Email),
            "name" => Some(<Entity as sea_orm::EntityTrait>::Column::Name),
            "age" => Some(<Entity as sea_orm::EntityTrait>::Column::Age),
            "created_at" => Some(<Entity as sea_orm::EntityTrait>::Column::CreatedAt),
            "updated_at" => Some(<Entity as sea_orm::EntityTrait>::Column::UpdatedAt),
            "deleted_at" => Some(<Entity as sea_orm::EntityTrait>::Column::DeletedAt),
            _ => None,
        }
    }
    pub struct EntityFetcherImpl;
    impl<C: sea_orm::ConnectionTrait> caustics::EntityFetcher<C> for EntityFetcherImpl {
        fn fetch_by_foreign_key<'a>(
            &'a self,
            conn: &'a C,
            foreign_key_value: Option<i32>,
            foreign_key_column: &'a str,
            target_entity: &'a str,
            relation_name: &'a str,
        ) -> std::pin::Pin<
            Box<
                dyn std::future::Future<
                    Output = Result<Box<dyn std::any::Any + Send>, sea_orm::DbErr>,
                > + Send + 'a,
            >,
        > {
            Box::pin(async move {
                match relation_name {
                    "posts" => {
                        let query = super::post::Entity::find()
                            .filter(
                                super::post::Column::UserId
                                    .eq(foreign_key_value.unwrap_or_default()),
                            );
                        let vec_with_rel = query
                            .all(conn)
                            .await?
                            .into_iter()
                            .map(|model| super::post::ModelWithRelations::from_model(
                                model,
                            ))
                            .collect::<Vec<_>>();
                        Ok(Box::new(Some(vec_with_rel)) as Box<dyn std::any::Any + Send>)
                    }
                    _ => {
                        Err(
                            sea_orm::DbErr::Custom(
                                ::alloc::__export::must_use({
                                    ::alloc::fmt::format(
                                        format_args!("Unknown relation: {0}", relation_name),
                                    )
                                }),
                            ),
                        )
                    }
                }
            })
        }
    }
    impl FromModel<Model> for Model {
        fn from_model(m: Model) -> Self {
            m
        }
    }
    impl sea_orm::ActiveModelBehavior for ActiveModel {}
}
pub mod post {
    use caustics_macros::Caustics;
    use sea_orm::entity::prelude::*;
    #[sea_orm(table_name = "posts")]
    pub struct Model {
        #[sea_orm(primary_key)]
        pub id: i32,
        pub title: String,
        #[sea_orm(nullable)]
        pub content: Option<String>,
        #[sea_orm(created_at)]
        pub created_at: DateTime<FixedOffset>,
        #[sea_orm(updated_at)]
        pub updated_at: DateTime<FixedOffset>,
        #[sea_orm(column_name = "user_id")]
        pub user_id: i32,
        #[sea_orm(column_name = "reviewer_user_id", nullable)]
        pub reviewer_user_id: Option<i32>,
    }
    #[automatically_derived]
    impl ::core::clone::Clone for Model {
        #[inline]
        fn clone(&self) -> Model {
            Model {
                id: ::core::clone::Clone::clone(&self.id),
                title: ::core::clone::Clone::clone(&self.title),
                content: ::core::clone::Clone::clone(&self.content),
                created_at: ::core::clone::Clone::clone(&self.created_at),
                updated_at: ::core::clone::Clone::clone(&self.updated_at),
                user_id: ::core::clone::Clone::clone(&self.user_id),
                reviewer_user_id: ::core::clone::Clone::clone(&self.reviewer_user_id),
            }
        }
    }
    #[automatically_derived]
    impl ::core::fmt::Debug for Model {
        #[inline]
        fn fmt(&self, f: &mut ::core::fmt::Formatter) -> ::core::fmt::Result {
            let names: &'static _ = &[
                "id",
                "title",
                "content",
                "created_at",
                "updated_at",
                "user_id",
                "reviewer_user_id",
            ];
            let values: &[&dyn ::core::fmt::Debug] = &[
                &self.id,
                &self.title,
                &self.content,
                &self.created_at,
                &self.updated_at,
                &self.user_id,
                &&self.reviewer_user_id,
            ];
            ::core::fmt::Formatter::debug_struct_fields_finish(f, "Model", names, values)
        }
    }
    #[automatically_derived]
    impl ::core::marker::StructuralPartialEq for Model {}
    #[automatically_derived]
    impl ::core::cmp::PartialEq for Model {
        #[inline]
        fn eq(&self, other: &Model) -> bool {
            self.id == other.id && self.title == other.title
                && self.content == other.content && self.created_at == other.created_at
                && self.updated_at == other.updated_at && self.user_id == other.user_id
                && self.reviewer_user_id == other.reviewer_user_id
        }
    }
    /// Generated by sea-orm-macros
    pub enum Column {
        /// Generated by sea-orm-macros
        Id,
        /// Generated by sea-orm-macros
        Title,
        /// Generated by sea-orm-macros
        Content,
        /// Generated by sea-orm-macros
        CreatedAt,
        /// Generated by sea-orm-macros
        UpdatedAt,
        #[sea_orm(column_name = "user_id")]
        /// Generated by sea-orm-macros
        UserId,
        #[sea_orm(column_name = "reviewer_user_id")]
        /// Generated by sea-orm-macros
        ReviewerUserId,
    }
    #[automatically_derived]
    impl ::core::marker::Copy for Column {}
    #[automatically_derived]
    impl ::core::clone::Clone for Column {
        #[inline]
        fn clone(&self) -> Column {
            *self
        }
    }
    #[automatically_derived]
    impl ::core::fmt::Debug for Column {
        #[inline]
        fn fmt(&self, f: &mut ::core::fmt::Formatter) -> ::core::fmt::Result {
            ::core::fmt::Formatter::write_str(
                f,
                match self {
                    Column::Id => "Id",
                    Column::Title => "Title",
                    Column::Content => "Content",
                    Column::CreatedAt => "CreatedAt",
                    Column::UpdatedAt => "UpdatedAt",
                    Column::UserId => "UserId",
                    Column::ReviewerUserId => "ReviewerUserId",
                },
            )
        }
    }
    ///An iterator over the variants of [Column]
    #[allow(missing_copy_implementations)]
    pub struct ColumnIter {
        idx: usize,
        back_idx: usize,
        marker: ::core::marker::PhantomData<()>,
    }
    impl core::fmt::Debug for ColumnIter {
        fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
            f.debug_struct("ColumnIter").field("len", &self.len()).finish()
        }
    }
    impl ColumnIter {
        fn get(&self, idx: usize) -> Option<Column> {
            match idx {
                0usize => ::core::option::Option::Some(Column::Id),
                1usize => ::core::option::Option::Some(Column::Title),
                2usize => ::core::option::Option::Some(Column::Content),
                3usize => ::core::option::Option::Some(Column::CreatedAt),
                4usize => ::core::option::Option::Some(Column::UpdatedAt),
                5usize => ::core::option::Option::Some(Column::UserId),
                6usize => ::core::option::Option::Some(Column::ReviewerUserId),
                _ => ::core::option::Option::None,
            }
        }
    }
    impl sea_orm::strum::IntoEnumIterator for Column {
        type Iterator = ColumnIter;
        fn iter() -> ColumnIter {
            ColumnIter {
                idx: 0,
                back_idx: 0,
                marker: ::core::marker::PhantomData,
            }
        }
    }
    impl Iterator for ColumnIter {
        type Item = Column;
        fn next(&mut self) -> Option<<Self as Iterator>::Item> {
            self.nth(0)
        }
        fn size_hint(&self) -> (usize, Option<usize>) {
            let t = if self.idx + self.back_idx >= 7usize {
                0
            } else {
                7usize - self.idx - self.back_idx
            };
            (t, Some(t))
        }
        fn nth(&mut self, n: usize) -> Option<<Self as Iterator>::Item> {
            let idx = self.idx + n + 1;
            if idx + self.back_idx > 7usize {
                self.idx = 7usize;
                ::core::option::Option::None
            } else {
                self.idx = idx;
                self.get(idx - 1)
            }
        }
    }
    impl ExactSizeIterator for ColumnIter {
        fn len(&self) -> usize {
            self.size_hint().0
        }
    }
    impl DoubleEndedIterator for ColumnIter {
        fn next_back(&mut self) -> Option<<Self as Iterator>::Item> {
            let back_idx = self.back_idx + 1;
            if self.idx + back_idx > 7usize {
                self.back_idx = 7usize;
                ::core::option::Option::None
            } else {
                self.back_idx = back_idx;
                self.get(7usize - self.back_idx)
            }
        }
    }
    impl Clone for ColumnIter {
        fn clone(&self) -> ColumnIter {
            ColumnIter {
                idx: self.idx,
                back_idx: self.back_idx,
                marker: self.marker.clone(),
            }
        }
    }
    #[automatically_derived]
    impl Column {
        fn default_as_str(&self) -> &str {
            match self {
                Self::Id => "id",
                Self::Title => "title",
                Self::Content => "content",
                Self::CreatedAt => "created_at",
                Self::UpdatedAt => "updated_at",
                Self::UserId => "user_id",
                Self::ReviewerUserId => "reviewer_user_id",
            }
        }
    }
    #[automatically_derived]
    impl std::str::FromStr for Column {
        type Err = sea_orm::ColumnFromStrErr;
        fn from_str(s: &str) -> std::result::Result<Self, Self::Err> {
            match s {
                "id" | "id" => Ok(Column::Id),
                "title" | "title" => Ok(Column::Title),
                "content" | "content" => Ok(Column::Content),
                "created_at" | "createdAt" => Ok(Column::CreatedAt),
                "updated_at" | "updatedAt" => Ok(Column::UpdatedAt),
                "user_id" | "userId" => Ok(Column::UserId),
                "reviewer_user_id" | "reviewerUserId" => Ok(Column::ReviewerUserId),
                _ => Err(sea_orm::ColumnFromStrErr(s.to_owned())),
            }
        }
    }
    #[automatically_derived]
    impl sea_orm::Iden for Column {
        fn unquoted(&self, s: &mut dyn std::fmt::Write) {
            s.write_fmt(format_args!("{0}", sea_orm::IdenStatic::as_str(self))).unwrap();
        }
    }
    #[automatically_derived]
    impl sea_orm::IdenStatic for Column {
        fn as_str(&self) -> &str {
            self.default_as_str()
        }
    }
    #[automatically_derived]
    impl sea_orm::prelude::ColumnTrait for Column {
        type EntityName = Entity;
        fn def(&self) -> sea_orm::prelude::ColumnDef {
            match self {
                Self::Id => {
                    sea_orm::prelude::ColumnTypeTrait::def(
                        sea_orm::prelude::ColumnType::Integer,
                    )
                }
                Self::Title => {
                    sea_orm::prelude::ColumnTypeTrait::def(
                        sea_orm::prelude::ColumnType::String(None),
                    )
                }
                Self::Content => {
                    sea_orm::prelude::ColumnTypeTrait::def(
                            sea_orm::prelude::ColumnType::String(None),
                        )
                        .nullable()
                }
                Self::CreatedAt => {
                    sea_orm::prelude::ColumnTypeTrait::def(
                        std::convert::Into::<
                            sea_orm::sea_query::ColumnType,
                        >::into(
                            <DateTime<
                                FixedOffset,
                            > as sea_orm::sea_query::ValueType>::column_type(),
                        ),
                    )
                }
                Self::UpdatedAt => {
                    sea_orm::prelude::ColumnTypeTrait::def(
                        std::convert::Into::<
                            sea_orm::sea_query::ColumnType,
                        >::into(
                            <DateTime<
                                FixedOffset,
                            > as sea_orm::sea_query::ValueType>::column_type(),
                        ),
                    )
                }
                Self::UserId => {
                    sea_orm::prelude::ColumnTypeTrait::def(
                        sea_orm::prelude::ColumnType::Integer,
                    )
                }
                Self::ReviewerUserId => {
                    sea_orm::prelude::ColumnTypeTrait::def(
                            sea_orm::prelude::ColumnType::Integer,
                        )
                        .nullable()
                }
            }
        }
        fn select_as(
            &self,
            expr: sea_orm::sea_query::Expr,
        ) -> sea_orm::sea_query::SimpleExpr {
            match self {
                _ => sea_orm::prelude::ColumnTrait::select_enum_as(self, expr),
            }
        }
        fn save_as(
            &self,
            val: sea_orm::sea_query::Expr,
        ) -> sea_orm::sea_query::SimpleExpr {
            match self {
                _ => sea_orm::prelude::ColumnTrait::save_enum_as(self, val),
            }
        }
    }
    /// Generated by sea-orm-macros
    pub struct Entity;
    #[automatically_derived]
    impl ::core::marker::Copy for Entity {}
    #[automatically_derived]
    impl ::core::clone::Clone for Entity {
        #[inline]
        fn clone(&self) -> Entity {
            *self
        }
    }
    #[automatically_derived]
    impl ::core::default::Default for Entity {
        #[inline]
        fn default() -> Entity {
            Entity {}
        }
    }
    #[automatically_derived]
    impl ::core::fmt::Debug for Entity {
        #[inline]
        fn fmt(&self, f: &mut ::core::fmt::Formatter) -> ::core::fmt::Result {
            ::core::fmt::Formatter::write_str(f, "Entity")
        }
    }
    #[automatically_derived]
    impl sea_orm::entity::EntityTrait for Entity {
        type Model = Model;
        type Column = Column;
        type PrimaryKey = PrimaryKey;
        type Relation = Relation;
    }
    #[automatically_derived]
    impl sea_orm::Iden for Entity {
        fn unquoted(&self, s: &mut dyn std::fmt::Write) {
            s.write_fmt(format_args!("{0}", sea_orm::IdenStatic::as_str(self))).unwrap();
        }
    }
    #[automatically_derived]
    impl sea_orm::IdenStatic for Entity {
        fn as_str(&self) -> &str {
            <Self as sea_orm::EntityName>::table_name(self)
        }
    }
    #[automatically_derived]
    impl sea_orm::prelude::EntityName for Entity {
        fn schema_name(&self) -> Option<&str> {
            None
        }
        fn table_name(&self) -> &str {
            "posts"
        }
        fn comment(&self) -> Option<&str> {
            None
        }
    }
    /// Generated by sea-orm-macros
    pub enum PrimaryKey {
        /// Generated by sea-orm-macros
        Id,
    }
    #[automatically_derived]
    impl ::core::marker::Copy for PrimaryKey {}
    #[automatically_derived]
    impl ::core::clone::Clone for PrimaryKey {
        #[inline]
        fn clone(&self) -> PrimaryKey {
            *self
        }
    }
    #[automatically_derived]
    impl ::core::fmt::Debug for PrimaryKey {
        #[inline]
        fn fmt(&self, f: &mut ::core::fmt::Formatter) -> ::core::fmt::Result {
            ::core::fmt::Formatter::write_str(f, "Id")
        }
    }
    ///An iterator over the variants of [PrimaryKey]
    #[allow(missing_copy_implementations)]
    pub struct PrimaryKeyIter {
        idx: usize,
        back_idx: usize,
        marker: ::core::marker::PhantomData<()>,
    }
    impl core::fmt::Debug for PrimaryKeyIter {
        fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
            f.debug_struct("PrimaryKeyIter").field("len", &self.len()).finish()
        }
    }
    impl PrimaryKeyIter {
        fn get(&self, idx: usize) -> Option<PrimaryKey> {
            match idx {
                0usize => ::core::option::Option::Some(PrimaryKey::Id),
                _ => ::core::option::Option::None,
            }
        }
    }
    impl sea_orm::strum::IntoEnumIterator for PrimaryKey {
        type Iterator = PrimaryKeyIter;
        fn iter() -> PrimaryKeyIter {
            PrimaryKeyIter {
                idx: 0,
                back_idx: 0,
                marker: ::core::marker::PhantomData,
            }
        }
    }
    impl Iterator for PrimaryKeyIter {
        type Item = PrimaryKey;
        fn next(&mut self) -> Option<<Self as Iterator>::Item> {
            self.nth(0)
        }
        fn size_hint(&self) -> (usize, Option<usize>) {
            let t = if self.idx + self.back_idx >= 1usize {
                0
            } else {
                1usize - self.idx - self.back_idx
            };
            (t, Some(t))
        }
        fn nth(&mut self, n: usize) -> Option<<Self as Iterator>::Item> {
            let idx = self.idx + n + 1;
            if idx + self.back_idx > 1usize {
                self.idx = 1usize;
                ::core::option::Option::None
            } else {
                self.idx = idx;
                self.get(idx - 1)
            }
        }
    }
    impl ExactSizeIterator for PrimaryKeyIter {
        fn len(&self) -> usize {
            self.size_hint().0
        }
    }
    impl DoubleEndedIterator for PrimaryKeyIter {
        fn next_back(&mut self) -> Option<<Self as Iterator>::Item> {
            let back_idx = self.back_idx + 1;
            if self.idx + back_idx > 1usize {
                self.back_idx = 1usize;
                ::core::option::Option::None
            } else {
                self.back_idx = back_idx;
                self.get(1usize - self.back_idx)
            }
        }
    }
    impl Clone for PrimaryKeyIter {
        fn clone(&self) -> PrimaryKeyIter {
            PrimaryKeyIter {
                idx: self.idx,
                back_idx: self.back_idx,
                marker: self.marker.clone(),
            }
        }
    }
    #[automatically_derived]
    impl sea_orm::Iden for PrimaryKey {
        fn unquoted(&self, s: &mut dyn std::fmt::Write) {
            s.write_fmt(format_args!("{0}", sea_orm::IdenStatic::as_str(self))).unwrap();
        }
    }
    #[automatically_derived]
    impl sea_orm::IdenStatic for PrimaryKey {
        fn as_str(&self) -> &str {
            match self {
                Self::Id => "id",
            }
        }
    }
    #[automatically_derived]
    impl sea_orm::PrimaryKeyToColumn for PrimaryKey {
        type Column = Column;
        fn into_column(self) -> Self::Column {
            match self {
                Self::Id => Self::Column::Id,
            }
        }
        fn from_column(col: Self::Column) -> Option<Self> {
            match col {
                Self::Column::Id => Some(Self::Id),
                _ => None,
            }
        }
    }
    #[automatically_derived]
    impl PrimaryKeyTrait for PrimaryKey {
        type ValueType = i32;
        fn auto_increment() -> bool {
            true
        }
    }
    #[automatically_derived]
    impl sea_orm::FromQueryResult for Model {
        fn from_query_result(
            row: &sea_orm::QueryResult,
            pre: &str,
        ) -> std::result::Result<Self, sea_orm::DbErr> {
            Ok(Self {
                id: row
                    .try_get(
                        pre,
                        sea_orm::IdenStatic::as_str(
                                &<<Self as sea_orm::ModelTrait>::Entity as sea_orm::entity::EntityTrait>::Column::Id,
                            )
                            .into(),
                    )?,
                title: row
                    .try_get(
                        pre,
                        sea_orm::IdenStatic::as_str(
                                &<<Self as sea_orm::ModelTrait>::Entity as sea_orm::entity::EntityTrait>::Column::Title,
                            )
                            .into(),
                    )?,
                content: row
                    .try_get(
                        pre,
                        sea_orm::IdenStatic::as_str(
                                &<<Self as sea_orm::ModelTrait>::Entity as sea_orm::entity::EntityTrait>::Column::Content,
                            )
                            .into(),
                    )?,
                created_at: row
                    .try_get(
                        pre,
                        sea_orm::IdenStatic::as_str(
                                &<<Self as sea_orm::ModelTrait>::Entity as sea_orm::entity::EntityTrait>::Column::CreatedAt,
                            )
                            .into(),
                    )?,
                updated_at: row
                    .try_get(
                        pre,
                        sea_orm::IdenStatic::as_str(
                                &<<Self as sea_orm::ModelTrait>::Entity as sea_orm::entity::EntityTrait>::Column::UpdatedAt,
                            )
                            .into(),
                    )?,
                user_id: row
                    .try_get(
                        pre,
                        sea_orm::IdenStatic::as_str(
                                &<<Self as sea_orm::ModelTrait>::Entity as sea_orm::entity::EntityTrait>::Column::UserId,
                            )
                            .into(),
                    )?,
                reviewer_user_id: row
                    .try_get(
                        pre,
                        sea_orm::IdenStatic::as_str(
                                &<<Self as sea_orm::ModelTrait>::Entity as sea_orm::entity::EntityTrait>::Column::ReviewerUserId,
                            )
                            .into(),
                    )?,
            })
        }
    }
    #[automatically_derived]
    impl sea_orm::ModelTrait for Model {
        type Entity = Entity;
        fn get(
            &self,
            c: <Self::Entity as sea_orm::entity::EntityTrait>::Column,
        ) -> sea_orm::Value {
            match c {
                <Self::Entity as sea_orm::entity::EntityTrait>::Column::Id => {
                    self.id.clone().into()
                }
                <Self::Entity as sea_orm::entity::EntityTrait>::Column::Title => {
                    self.title.clone().into()
                }
                <Self::Entity as sea_orm::entity::EntityTrait>::Column::Content => {
                    self.content.clone().into()
                }
                <Self::Entity as sea_orm::entity::EntityTrait>::Column::CreatedAt => {
                    self.created_at.clone().into()
                }
                <Self::Entity as sea_orm::entity::EntityTrait>::Column::UpdatedAt => {
                    self.updated_at.clone().into()
                }
                <Self::Entity as sea_orm::entity::EntityTrait>::Column::UserId => {
                    self.user_id.clone().into()
                }
                <Self::Entity as sea_orm::entity::EntityTrait>::Column::ReviewerUserId => {
                    self.reviewer_user_id.clone().into()
                }
                _ => {
                    ::core::panicking::panic_fmt(
                        format_args!("field does not exist on Model"),
                    );
                }
            }
        }
        fn set(
            &mut self,
            c: <Self::Entity as sea_orm::entity::EntityTrait>::Column,
            v: sea_orm::Value,
        ) {
            match c {
                <Self::Entity as sea_orm::entity::EntityTrait>::Column::Id => {
                    self.id = v.unwrap();
                }
                <Self::Entity as sea_orm::entity::EntityTrait>::Column::Title => {
                    self.title = v.unwrap();
                }
                <Self::Entity as sea_orm::entity::EntityTrait>::Column::Content => {
                    self.content = v.unwrap();
                }
                <Self::Entity as sea_orm::entity::EntityTrait>::Column::CreatedAt => {
                    self.created_at = v.unwrap();
                }
                <Self::Entity as sea_orm::entity::EntityTrait>::Column::UpdatedAt => {
                    self.updated_at = v.unwrap();
                }
                <Self::Entity as sea_orm::entity::EntityTrait>::Column::UserId => {
                    self.user_id = v.unwrap();
                }
                <Self::Entity as sea_orm::entity::EntityTrait>::Column::ReviewerUserId => {
                    self.reviewer_user_id = v.unwrap();
                }
                _ => {
                    ::core::panicking::panic_fmt(
                        format_args!("field does not exist on Model"),
                    );
                }
            }
        }
    }
    /// Generated by sea-orm-macros
    pub struct ActiveModel {
        /// Generated by sea-orm-macros
        pub id: sea_orm::ActiveValue<i32>,
        /// Generated by sea-orm-macros
        pub title: sea_orm::ActiveValue<String>,
        /// Generated by sea-orm-macros
        pub content: sea_orm::ActiveValue<Option<String>>,
        /// Generated by sea-orm-macros
        pub created_at: sea_orm::ActiveValue<DateTime<FixedOffset>>,
        /// Generated by sea-orm-macros
        pub updated_at: sea_orm::ActiveValue<DateTime<FixedOffset>>,
        /// Generated by sea-orm-macros
        pub user_id: sea_orm::ActiveValue<i32>,
        /// Generated by sea-orm-macros
        pub reviewer_user_id: sea_orm::ActiveValue<Option<i32>>,
    }
    #[automatically_derived]
    impl ::core::clone::Clone for ActiveModel {
        #[inline]
        fn clone(&self) -> ActiveModel {
            ActiveModel {
                id: ::core::clone::Clone::clone(&self.id),
                title: ::core::clone::Clone::clone(&self.title),
                content: ::core::clone::Clone::clone(&self.content),
                created_at: ::core::clone::Clone::clone(&self.created_at),
                updated_at: ::core::clone::Clone::clone(&self.updated_at),
                user_id: ::core::clone::Clone::clone(&self.user_id),
                reviewer_user_id: ::core::clone::Clone::clone(&self.reviewer_user_id),
            }
        }
    }
    #[automatically_derived]
    impl ::core::fmt::Debug for ActiveModel {
        #[inline]
        fn fmt(&self, f: &mut ::core::fmt::Formatter) -> ::core::fmt::Result {
            let names: &'static _ = &[
                "id",
                "title",
                "content",
                "created_at",
                "updated_at",
                "user_id",
                "reviewer_user_id",
            ];
            let values: &[&dyn ::core::fmt::Debug] = &[
                &self.id,
                &self.title,
                &self.content,
                &self.created_at,
                &self.updated_at,
                &self.user_id,
                &&self.reviewer_user_id,
            ];
            ::core::fmt::Formatter::debug_struct_fields_finish(
                f,
                "ActiveModel",
                names,
                values,
            )
        }
    }
    #[automatically_derived]
    impl ::core::marker::StructuralPartialEq for ActiveModel {}
    #[automatically_derived]
    impl ::core::cmp::PartialEq for ActiveModel {
        #[inline]
        fn eq(&self, other: &ActiveModel) -> bool {
            self.id == other.id && self.title == other.title
                && self.content == other.content && self.created_at == other.created_at
                && self.updated_at == other.updated_at && self.user_id == other.user_id
                && self.reviewer_user_id == other.reviewer_user_id
        }
    }
    #[automatically_derived]
    impl std::default::Default for ActiveModel {
        fn default() -> Self {
            <Self as sea_orm::ActiveModelBehavior>::new()
        }
    }
    #[automatically_derived]
    impl std::convert::From<<Entity as EntityTrait>::Model> for ActiveModel {
        fn from(m: <Entity as EntityTrait>::Model) -> Self {
            Self {
                id: sea_orm::ActiveValue::unchanged(m.id),
                title: sea_orm::ActiveValue::unchanged(m.title),
                content: sea_orm::ActiveValue::unchanged(m.content),
                created_at: sea_orm::ActiveValue::unchanged(m.created_at),
                updated_at: sea_orm::ActiveValue::unchanged(m.updated_at),
                user_id: sea_orm::ActiveValue::unchanged(m.user_id),
                reviewer_user_id: sea_orm::ActiveValue::unchanged(m.reviewer_user_id),
            }
        }
    }
    #[automatically_derived]
    impl sea_orm::IntoActiveModel<ActiveModel> for <Entity as EntityTrait>::Model {
        fn into_active_model(self) -> ActiveModel {
            self.into()
        }
    }
    #[automatically_derived]
    impl sea_orm::ActiveModelTrait for ActiveModel {
        type Entity = Entity;
        fn take(
            &mut self,
            c: <Self::Entity as EntityTrait>::Column,
        ) -> sea_orm::ActiveValue<sea_orm::Value> {
            match c {
                <Self::Entity as EntityTrait>::Column::Id => {
                    let mut value = sea_orm::ActiveValue::not_set();
                    std::mem::swap(&mut value, &mut self.id);
                    value.into_wrapped_value()
                }
                <Self::Entity as EntityTrait>::Column::Title => {
                    let mut value = sea_orm::ActiveValue::not_set();
                    std::mem::swap(&mut value, &mut self.title);
                    value.into_wrapped_value()
                }
                <Self::Entity as EntityTrait>::Column::Content => {
                    let mut value = sea_orm::ActiveValue::not_set();
                    std::mem::swap(&mut value, &mut self.content);
                    value.into_wrapped_value()
                }
                <Self::Entity as EntityTrait>::Column::CreatedAt => {
                    let mut value = sea_orm::ActiveValue::not_set();
                    std::mem::swap(&mut value, &mut self.created_at);
                    value.into_wrapped_value()
                }
                <Self::Entity as EntityTrait>::Column::UpdatedAt => {
                    let mut value = sea_orm::ActiveValue::not_set();
                    std::mem::swap(&mut value, &mut self.updated_at);
                    value.into_wrapped_value()
                }
                <Self::Entity as EntityTrait>::Column::UserId => {
                    let mut value = sea_orm::ActiveValue::not_set();
                    std::mem::swap(&mut value, &mut self.user_id);
                    value.into_wrapped_value()
                }
                <Self::Entity as EntityTrait>::Column::ReviewerUserId => {
                    let mut value = sea_orm::ActiveValue::not_set();
                    std::mem::swap(&mut value, &mut self.reviewer_user_id);
                    value.into_wrapped_value()
                }
                _ => sea_orm::ActiveValue::not_set(),
            }
        }
        fn get(
            &self,
            c: <Self::Entity as EntityTrait>::Column,
        ) -> sea_orm::ActiveValue<sea_orm::Value> {
            match c {
                <Self::Entity as EntityTrait>::Column::Id => {
                    self.id.clone().into_wrapped_value()
                }
                <Self::Entity as EntityTrait>::Column::Title => {
                    self.title.clone().into_wrapped_value()
                }
                <Self::Entity as EntityTrait>::Column::Content => {
                    self.content.clone().into_wrapped_value()
                }
                <Self::Entity as EntityTrait>::Column::CreatedAt => {
                    self.created_at.clone().into_wrapped_value()
                }
                <Self::Entity as EntityTrait>::Column::UpdatedAt => {
                    self.updated_at.clone().into_wrapped_value()
                }
                <Self::Entity as EntityTrait>::Column::UserId => {
                    self.user_id.clone().into_wrapped_value()
                }
                <Self::Entity as EntityTrait>::Column::ReviewerUserId => {
                    self.reviewer_user_id.clone().into_wrapped_value()
                }
                _ => sea_orm::ActiveValue::not_set(),
            }
        }
        fn set(&mut self, c: <Self::Entity as EntityTrait>::Column, v: sea_orm::Value) {
            match c {
                <Self::Entity as EntityTrait>::Column::Id => {
                    self.id = sea_orm::ActiveValue::set(v.unwrap());
                }
                <Self::Entity as EntityTrait>::Column::Title => {
                    self.title = sea_orm::ActiveValue::set(v.unwrap());
                }
                <Self::Entity as EntityTrait>::Column::Content => {
                    self.content = sea_orm::ActiveValue::set(v.unwrap());
                }
                <Self::Entity as EntityTrait>::Column::CreatedAt => {
                    self.created_at = sea_orm::ActiveValue::set(v.unwrap());
                }
                <Self::Entity as EntityTrait>::Column::UpdatedAt => {
                    self.updated_at = sea_orm::ActiveValue::set(v.unwrap());
                }
                <Self::Entity as EntityTrait>::Column::UserId => {
                    self.user_id = sea_orm::ActiveValue::set(v.unwrap());
                }
                <Self::Entity as EntityTrait>::Column::ReviewerUserId => {
                    self.reviewer_user_id = sea_orm::ActiveValue::set(v.unwrap());
                }
                _ => {
                    ::core::panicking::panic_fmt(
                        format_args!("This ActiveModel does not have this field"),
                    );
                }
            }
        }
        fn not_set(&mut self, c: <Self::Entity as EntityTrait>::Column) {
            match c {
                <Self::Entity as EntityTrait>::Column::Id => {
                    self.id = sea_orm::ActiveValue::not_set();
                }
                <Self::Entity as EntityTrait>::Column::Title => {
                    self.title = sea_orm::ActiveValue::not_set();
                }
                <Self::Entity as EntityTrait>::Column::Content => {
                    self.content = sea_orm::ActiveValue::not_set();
                }
                <Self::Entity as EntityTrait>::Column::CreatedAt => {
                    self.created_at = sea_orm::ActiveValue::not_set();
                }
                <Self::Entity as EntityTrait>::Column::UpdatedAt => {
                    self.updated_at = sea_orm::ActiveValue::not_set();
                }
                <Self::Entity as EntityTrait>::Column::UserId => {
                    self.user_id = sea_orm::ActiveValue::not_set();
                }
                <Self::Entity as EntityTrait>::Column::ReviewerUserId => {
                    self.reviewer_user_id = sea_orm::ActiveValue::not_set();
                }
                _ => {}
            }
        }
        fn is_not_set(&self, c: <Self::Entity as EntityTrait>::Column) -> bool {
            match c {
                <Self::Entity as EntityTrait>::Column::Id => self.id.is_not_set(),
                <Self::Entity as EntityTrait>::Column::Title => self.title.is_not_set(),
                <Self::Entity as EntityTrait>::Column::Content => {
                    self.content.is_not_set()
                }
                <Self::Entity as EntityTrait>::Column::CreatedAt => {
                    self.created_at.is_not_set()
                }
                <Self::Entity as EntityTrait>::Column::UpdatedAt => {
                    self.updated_at.is_not_set()
                }
                <Self::Entity as EntityTrait>::Column::UserId => {
                    self.user_id.is_not_set()
                }
                <Self::Entity as EntityTrait>::Column::ReviewerUserId => {
                    self.reviewer_user_id.is_not_set()
                }
                _ => {
                    ::core::panicking::panic_fmt(
                        format_args!("This ActiveModel does not have this field"),
                    );
                }
            }
        }
        fn default() -> Self {
            Self {
                id: sea_orm::ActiveValue::not_set(),
                title: sea_orm::ActiveValue::not_set(),
                content: sea_orm::ActiveValue::not_set(),
                created_at: sea_orm::ActiveValue::not_set(),
                updated_at: sea_orm::ActiveValue::not_set(),
                user_id: sea_orm::ActiveValue::not_set(),
                reviewer_user_id: sea_orm::ActiveValue::not_set(),
            }
        }
        fn reset(&mut self, c: <Self::Entity as EntityTrait>::Column) {
            match c {
                <Self::Entity as EntityTrait>::Column::Id => self.id.reset(),
                <Self::Entity as EntityTrait>::Column::Title => self.title.reset(),
                <Self::Entity as EntityTrait>::Column::Content => self.content.reset(),
                <Self::Entity as EntityTrait>::Column::CreatedAt => {
                    self.created_at.reset()
                }
                <Self::Entity as EntityTrait>::Column::UpdatedAt => {
                    self.updated_at.reset()
                }
                <Self::Entity as EntityTrait>::Column::UserId => self.user_id.reset(),
                <Self::Entity as EntityTrait>::Column::ReviewerUserId => {
                    self.reviewer_user_id.reset()
                }
                _ => {
                    ::core::panicking::panic_fmt(
                        format_args!("This ActiveModel does not have this field"),
                    );
                }
            }
        }
    }
    #[automatically_derived]
    impl std::convert::TryFrom<ActiveModel> for <Entity as EntityTrait>::Model {
        type Error = sea_orm::DbErr;
        fn try_from(a: ActiveModel) -> Result<Self, sea_orm::DbErr> {
            if match a.id {
                sea_orm::ActiveValue::NotSet => true,
                _ => false,
            } {
                return Err(sea_orm::DbErr::AttrNotSet("id".to_owned()));
            }
            if match a.title {
                sea_orm::ActiveValue::NotSet => true,
                _ => false,
            } {
                return Err(sea_orm::DbErr::AttrNotSet("title".to_owned()));
            }
            if match a.content {
                sea_orm::ActiveValue::NotSet => true,
                _ => false,
            } {
                return Err(sea_orm::DbErr::AttrNotSet("content".to_owned()));
            }
            if match a.created_at {
                sea_orm::ActiveValue::NotSet => true,
                _ => false,
            } {
                return Err(sea_orm::DbErr::AttrNotSet("created_at".to_owned()));
            }
            if match a.updated_at {
                sea_orm::ActiveValue::NotSet => true,
                _ => false,
            } {
                return Err(sea_orm::DbErr::AttrNotSet("updated_at".to_owned()));
            }
            if match a.user_id {
                sea_orm::ActiveValue::NotSet => true,
                _ => false,
            } {
                return Err(sea_orm::DbErr::AttrNotSet("user_id".to_owned()));
            }
            if match a.reviewer_user_id {
                sea_orm::ActiveValue::NotSet => true,
                _ => false,
            } {
                return Err(sea_orm::DbErr::AttrNotSet("reviewer_user_id".to_owned()));
            }
            Ok(Self {
                id: a.id.into_value().unwrap().unwrap(),
                title: a.title.into_value().unwrap().unwrap(),
                content: a.content.into_value().unwrap().unwrap(),
                created_at: a.created_at.into_value().unwrap().unwrap(),
                updated_at: a.updated_at.into_value().unwrap().unwrap(),
                user_id: a.user_id.into_value().unwrap().unwrap(),
                reviewer_user_id: a.reviewer_user_id.into_value().unwrap().unwrap(),
            })
        }
    }
    #[automatically_derived]
    impl sea_orm::TryIntoModel<<Entity as EntityTrait>::Model> for ActiveModel {
        fn try_into_model(
            self,
        ) -> Result<<Entity as EntityTrait>::Model, sea_orm::DbErr> {
            self.try_into()
        }
    }
    pub enum Relation {
        #[sea_orm(
            belongs_to = "super::user::Entity",
            from = "Column::UserId",
            to = "super::user::Column::Id",
            on_update = "NoAction",
            on_delete = "NoAction"
        )]
        User,
        #[sea_orm(
            belongs_to = "super::user::Entity",
            from = "Column::ReviewerUserId",
            to = "super::user::Column::Id",
            on_update = "NoAction",
            on_delete = "NoAction"
        )]
        Reviewer,
    }
    #[automatically_derived]
    impl ::core::marker::Copy for Relation {}
    #[automatically_derived]
    impl ::core::clone::Clone for Relation {
        #[inline]
        fn clone(&self) -> Relation {
            *self
        }
    }
    #[automatically_derived]
    impl ::core::fmt::Debug for Relation {
        #[inline]
        fn fmt(&self, f: &mut ::core::fmt::Formatter) -> ::core::fmt::Result {
            ::core::fmt::Formatter::write_str(
                f,
                match self {
                    Relation::User => "User",
                    Relation::Reviewer => "Reviewer",
                },
            )
        }
    }
    ///An iterator over the variants of [Relation]
    #[allow(missing_copy_implementations)]
    pub struct RelationIter {
        idx: usize,
        back_idx: usize,
        marker: ::core::marker::PhantomData<()>,
    }
    impl core::fmt::Debug for RelationIter {
        fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
            f.debug_struct("RelationIter").field("len", &self.len()).finish()
        }
    }
    impl RelationIter {
        fn get(&self, idx: usize) -> Option<Relation> {
            match idx {
                0usize => ::core::option::Option::Some(Relation::User),
                1usize => ::core::option::Option::Some(Relation::Reviewer),
                _ => ::core::option::Option::None,
            }
        }
    }
    impl sea_orm::strum::IntoEnumIterator for Relation {
        type Iterator = RelationIter;
        fn iter() -> RelationIter {
            RelationIter {
                idx: 0,
                back_idx: 0,
                marker: ::core::marker::PhantomData,
            }
        }
    }
    impl Iterator for RelationIter {
        type Item = Relation;
        fn next(&mut self) -> Option<<Self as Iterator>::Item> {
            self.nth(0)
        }
        fn size_hint(&self) -> (usize, Option<usize>) {
            let t = if self.idx + self.back_idx >= 2usize {
                0
            } else {
                2usize - self.idx - self.back_idx
            };
            (t, Some(t))
        }
        fn nth(&mut self, n: usize) -> Option<<Self as Iterator>::Item> {
            let idx = self.idx + n + 1;
            if idx + self.back_idx > 2usize {
                self.idx = 2usize;
                ::core::option::Option::None
            } else {
                self.idx = idx;
                self.get(idx - 1)
            }
        }
    }
    impl ExactSizeIterator for RelationIter {
        fn len(&self) -> usize {
            self.size_hint().0
        }
    }
    impl DoubleEndedIterator for RelationIter {
        fn next_back(&mut self) -> Option<<Self as Iterator>::Item> {
            let back_idx = self.back_idx + 1;
            if self.idx + back_idx > 2usize {
                self.back_idx = 2usize;
                ::core::option::Option::None
            } else {
                self.back_idx = back_idx;
                self.get(2usize - self.back_idx)
            }
        }
    }
    impl Clone for RelationIter {
        fn clone(&self) -> RelationIter {
            RelationIter {
                idx: self.idx,
                back_idx: self.back_idx,
                marker: self.marker.clone(),
            }
        }
    }
    #[automatically_derived]
    impl sea_orm::entity::RelationTrait for Relation {
        fn def(&self) -> sea_orm::entity::RelationDef {
            match self {
                Self::User => {
                    Entity::belongs_to(super::user::Entity)
                        .from(Column::UserId)
                        .to(super::user::Column::Id)
                        .on_update(sea_orm::prelude::ForeignKeyAction::NoAction)
                        .on_delete(sea_orm::prelude::ForeignKeyAction::NoAction)
                        .into()
                }
                Self::Reviewer => {
                    Entity::belongs_to(super::user::Entity)
                        .from(Column::ReviewerUserId)
                        .to(super::user::Column::Id)
                        .on_update(sea_orm::prelude::ForeignKeyAction::NoAction)
                        .on_delete(sea_orm::prelude::ForeignKeyAction::NoAction)
                        .into()
                }
                _ => {
                    ::core::panicking::panic_fmt(
                        format_args!("No RelationDef for Relation"),
                    );
                }
            }
        }
    }
    impl Related<super::user::Entity> for Entity {
        fn to() -> RelationDef {
            Relation::User.def()
        }
    }
    use chrono::{NaiveDate, NaiveDateTime, DateTime, FixedOffset};
    use uuid::Uuid;
    use std::vec::Vec;
    use caustics::{SortOrder, MergeInto};
    use caustics::FromModel;
    use sea_query::{Condition, Expr};
    pub struct EntityClient<'a, C: sea_orm::ConnectionTrait> {
        conn: &'a C,
    }
    pub enum FieldOp<T> {
        Equals(T),
        NotEquals(T),
        Gt(T),
        Lt(T),
        Gte(T),
        Lte(T),
        InVec(Vec<T>),
        NotInVec(Vec<T>),
        Contains(String),
        StartsWith(String),
        EndsWith(String),
    }
    pub enum SetParam {
        Title(sea_orm::ActiveValue<String>),
        Content(sea_orm::ActiveValue<Option<String>>),
        CreatedAt(sea_orm::ActiveValue<DateTime<FixedOffset>>),
        UpdatedAt(sea_orm::ActiveValue<DateTime<FixedOffset>>),
        UserId(sea_orm::ActiveValue<i32>),
        ReviewerUserId(sea_orm::ActiveValue<Option<i32>>),
        ConnectUser(super::user::UniqueWhereParam),
        ConnectReviewer(super::user::UniqueWhereParam),
        DisconnectReviewer,
    }
    pub enum WhereParam {
        Id(FieldOp<i32>),
        Title(FieldOp<String>),
        TitleMode(caustics::QueryMode),
        Content(FieldOp<Option<String>>),
        ContentMode(caustics::QueryMode),
        CreatedAt(FieldOp<DateTime<FixedOffset>>),
        UpdatedAt(FieldOp<DateTime<FixedOffset>>),
        UserId(FieldOp<i32>),
        ReviewerUserId(FieldOp<Option<i32>>),
        And(Vec<super::WhereParam>),
        Or(Vec<super::WhereParam>),
        Not(Vec<super::WhereParam>),
    }
    pub enum OrderByParam {
        Id(caustics::SortOrder),
        Title(caustics::SortOrder),
        Content(caustics::SortOrder),
        CreatedAt(caustics::SortOrder),
        UpdatedAt(caustics::SortOrder),
        UserId(caustics::SortOrder),
        ReviewerUserId(caustics::SortOrder),
    }
    pub enum UniqueWhereParam {
        IdEquals(i32),
    }
    #[automatically_derived]
    impl ::core::fmt::Debug for UniqueWhereParam {
        #[inline]
        fn fmt(&self, f: &mut ::core::fmt::Formatter) -> ::core::fmt::Result {
            match self {
                UniqueWhereParam::IdEquals(__self_0) => {
                    ::core::fmt::Formatter::debug_tuple_field1_finish(
                        f,
                        "IdEquals",
                        &__self_0,
                    )
                }
            }
        }
    }
    #[automatically_derived]
    impl ::core::clone::Clone for UniqueWhereParam {
        #[inline]
        fn clone(&self) -> UniqueWhereParam {
            match self {
                UniqueWhereParam::IdEquals(__self_0) => {
                    UniqueWhereParam::IdEquals(::core::clone::Clone::clone(__self_0))
                }
            }
        }
    }
    #[allow(dead_code)]
    pub mod id {
        use chrono::{NaiveDate, NaiveDateTime, DateTime, FixedOffset};
        use uuid::Uuid;
        use std::vec::Vec;
        use super::*;
        pub struct Equals(pub i32);
        pub fn equals<T: From<Equals>>(value: impl Into<i32>) -> T {
            Equals(value.into()).into()
        }
        impl From<Equals> for super::UniqueWhereParam {
            fn from(Equals(v): Equals) -> Self {
                super::UniqueWhereParam::IdEquals(v)
            }
        }
        impl From<Equals> for super::WhereParam {
            fn from(Equals(v): Equals) -> Self {
                super::WhereParam::Id(FieldOp::Equals(v))
            }
        }
        pub fn order(order: caustics::SortOrder) -> super::OrderByParam {
            super::OrderByParam::Id(order)
        }
        pub fn not_equals<T: Into<i32>>(value: T) -> super::WhereParam {
            super::WhereParam::Id(FieldOp::NotEquals(value.into()))
        }
        pub fn gt<T: Into<i32>>(value: T) -> super::WhereParam {
            super::WhereParam::Id(FieldOp::Gt(value.into()))
        }
        pub fn lt<T: Into<i32>>(value: T) -> super::WhereParam {
            super::WhereParam::Id(FieldOp::Lt(value.into()))
        }
        pub fn gte<T: Into<i32>>(value: T) -> super::WhereParam {
            super::WhereParam::Id(FieldOp::Gte(value.into()))
        }
        pub fn lte<T: Into<i32>>(value: T) -> super::WhereParam {
            super::WhereParam::Id(FieldOp::Lte(value.into()))
        }
        pub fn in_vec<T: Into<i32>>(values: Vec<T>) -> super::WhereParam {
            super::WhereParam::Id(
                FieldOp::InVec(values.into_iter().map(|v| v.into()).collect()),
            )
        }
        pub fn not_in_vec<T: Into<i32>>(values: Vec<T>) -> super::WhereParam {
            super::WhereParam::Id(
                FieldOp::NotInVec(values.into_iter().map(|v| v.into()).collect()),
            )
        }
    }
    #[allow(dead_code)]
    pub mod title {
        use chrono::{NaiveDate, NaiveDateTime, DateTime, FixedOffset};
        use uuid::Uuid;
        use std::vec::Vec;
        use super::*;
        pub fn set<T: Into<String>>(value: T) -> super::SetParam {
            super::SetParam::Title(sea_orm::ActiveValue::Set(value.into()))
        }
        pub fn equals<T: Into<String>>(value: T) -> super::WhereParam {
            super::WhereParam::Title(FieldOp::Equals(value.into()))
        }
        pub fn order(order: caustics::SortOrder) -> super::OrderByParam {
            super::OrderByParam::Title(order)
        }
        pub fn contains<T: Into<String>>(value: T) -> super::WhereParam {
            super::WhereParam::Title(FieldOp::Contains(value.into()))
        }
        pub fn starts_with<T: Into<String>>(value: T) -> super::WhereParam {
            super::WhereParam::Title(FieldOp::StartsWith(value.into()))
        }
        pub fn ends_with<T: Into<String>>(value: T) -> super::WhereParam {
            super::WhereParam::Title(FieldOp::EndsWith(value.into()))
        }
        pub fn not_equals<T: Into<String>>(value: T) -> super::WhereParam {
            super::WhereParam::Title(FieldOp::NotEquals(value.into()))
        }
        pub fn gt<T: Into<String>>(value: T) -> super::WhereParam {
            super::WhereParam::Title(FieldOp::Gt(value.into()))
        }
        pub fn lt<T: Into<String>>(value: T) -> super::WhereParam {
            super::WhereParam::Title(FieldOp::Lt(value.into()))
        }
        pub fn gte<T: Into<String>>(value: T) -> super::WhereParam {
            super::WhereParam::Title(FieldOp::Gte(value.into()))
        }
        pub fn lte<T: Into<String>>(value: T) -> super::WhereParam {
            super::WhereParam::Title(FieldOp::Lte(value.into()))
        }
        pub fn in_vec<T: Into<String>>(values: Vec<T>) -> super::WhereParam {
            super::WhereParam::Title(
                FieldOp::InVec(values.into_iter().map(|v| v.into()).collect()),
            )
        }
        pub fn not_in_vec<T: Into<String>>(values: Vec<T>) -> super::WhereParam {
            super::WhereParam::Title(
                FieldOp::NotInVec(values.into_iter().map(|v| v.into()).collect()),
            )
        }
        pub fn mode(mode: caustics::QueryMode) -> super::WhereParam {
            super::WhereParam::TitleMode(mode)
        }
    }
    #[allow(dead_code)]
    pub mod content {
        use chrono::{NaiveDate, NaiveDateTime, DateTime, FixedOffset};
        use uuid::Uuid;
        use std::vec::Vec;
        use super::*;
        pub fn set<T: Into<Option<String>>>(value: T) -> super::SetParam {
            super::SetParam::Content(sea_orm::ActiveValue::Set(value.into()))
        }
        pub fn equals<T: Into<Option<String>>>(value: T) -> super::WhereParam {
            super::WhereParam::Content(FieldOp::Equals(value.into()))
        }
        pub fn order(order: caustics::SortOrder) -> super::OrderByParam {
            super::OrderByParam::Content(order)
        }
        pub fn contains<T: Into<String>>(value: T) -> super::WhereParam {
            super::WhereParam::Content(FieldOp::Contains(value.into()))
        }
        pub fn starts_with<T: Into<String>>(value: T) -> super::WhereParam {
            super::WhereParam::Content(FieldOp::StartsWith(value.into()))
        }
        pub fn ends_with<T: Into<String>>(value: T) -> super::WhereParam {
            super::WhereParam::Content(FieldOp::EndsWith(value.into()))
        }
        pub fn not_equals<T: Into<Option<String>>>(value: T) -> super::WhereParam {
            super::WhereParam::Content(FieldOp::NotEquals(value.into()))
        }
        pub fn gt<T: Into<Option<String>>>(value: T) -> super::WhereParam {
            super::WhereParam::Content(FieldOp::Gt(value.into()))
        }
        pub fn lt<T: Into<Option<String>>>(value: T) -> super::WhereParam {
            super::WhereParam::Content(FieldOp::Lt(value.into()))
        }
        pub fn gte<T: Into<Option<String>>>(value: T) -> super::WhereParam {
            super::WhereParam::Content(FieldOp::Gte(value.into()))
        }
        pub fn lte<T: Into<Option<String>>>(value: T) -> super::WhereParam {
            super::WhereParam::Content(FieldOp::Lte(value.into()))
        }
        pub fn in_vec<T: Into<Option<String>>>(values: Vec<T>) -> super::WhereParam {
            super::WhereParam::Content(
                FieldOp::InVec(values.into_iter().map(|v| v.into()).collect()),
            )
        }
        pub fn not_in_vec<T: Into<Option<String>>>(values: Vec<T>) -> super::WhereParam {
            super::WhereParam::Content(
                FieldOp::NotInVec(values.into_iter().map(|v| v.into()).collect()),
            )
        }
        pub fn mode(mode: caustics::QueryMode) -> super::WhereParam {
            super::WhereParam::ContentMode(mode)
        }
    }
    #[allow(dead_code)]
    pub mod created_at {
        use chrono::{NaiveDate, NaiveDateTime, DateTime, FixedOffset};
        use uuid::Uuid;
        use std::vec::Vec;
        use super::*;
        pub fn set<T: Into<DateTime<FixedOffset>>>(value: T) -> super::SetParam {
            super::SetParam::CreatedAt(sea_orm::ActiveValue::Set(value.into()))
        }
        pub fn equals<T: Into<DateTime<FixedOffset>>>(value: T) -> super::WhereParam {
            super::WhereParam::CreatedAt(FieldOp::Equals(value.into()))
        }
        pub fn order(order: caustics::SortOrder) -> super::OrderByParam {
            super::OrderByParam::CreatedAt(order)
        }
        pub fn not_equals<T: Into<DateTime<FixedOffset>>>(
            value: T,
        ) -> super::WhereParam {
            super::WhereParam::CreatedAt(FieldOp::NotEquals(value.into()))
        }
        pub fn gt<T: Into<DateTime<FixedOffset>>>(value: T) -> super::WhereParam {
            super::WhereParam::CreatedAt(FieldOp::Gt(value.into()))
        }
        pub fn lt<T: Into<DateTime<FixedOffset>>>(value: T) -> super::WhereParam {
            super::WhereParam::CreatedAt(FieldOp::Lt(value.into()))
        }
        pub fn gte<T: Into<DateTime<FixedOffset>>>(value: T) -> super::WhereParam {
            super::WhereParam::CreatedAt(FieldOp::Gte(value.into()))
        }
        pub fn lte<T: Into<DateTime<FixedOffset>>>(value: T) -> super::WhereParam {
            super::WhereParam::CreatedAt(FieldOp::Lte(value.into()))
        }
        pub fn in_vec<T: Into<DateTime<FixedOffset>>>(
            values: Vec<T>,
        ) -> super::WhereParam {
            super::WhereParam::CreatedAt(
                FieldOp::InVec(values.into_iter().map(|v| v.into()).collect()),
            )
        }
        pub fn not_in_vec<T: Into<DateTime<FixedOffset>>>(
            values: Vec<T>,
        ) -> super::WhereParam {
            super::WhereParam::CreatedAt(
                FieldOp::NotInVec(values.into_iter().map(|v| v.into()).collect()),
            )
        }
    }
    #[allow(dead_code)]
    pub mod updated_at {
        use chrono::{NaiveDate, NaiveDateTime, DateTime, FixedOffset};
        use uuid::Uuid;
        use std::vec::Vec;
        use super::*;
        pub fn set<T: Into<DateTime<FixedOffset>>>(value: T) -> super::SetParam {
            super::SetParam::UpdatedAt(sea_orm::ActiveValue::Set(value.into()))
        }
        pub fn equals<T: Into<DateTime<FixedOffset>>>(value: T) -> super::WhereParam {
            super::WhereParam::UpdatedAt(FieldOp::Equals(value.into()))
        }
        pub fn order(order: caustics::SortOrder) -> super::OrderByParam {
            super::OrderByParam::UpdatedAt(order)
        }
        pub fn not_equals<T: Into<DateTime<FixedOffset>>>(
            value: T,
        ) -> super::WhereParam {
            super::WhereParam::UpdatedAt(FieldOp::NotEquals(value.into()))
        }
        pub fn gt<T: Into<DateTime<FixedOffset>>>(value: T) -> super::WhereParam {
            super::WhereParam::UpdatedAt(FieldOp::Gt(value.into()))
        }
        pub fn lt<T: Into<DateTime<FixedOffset>>>(value: T) -> super::WhereParam {
            super::WhereParam::UpdatedAt(FieldOp::Lt(value.into()))
        }
        pub fn gte<T: Into<DateTime<FixedOffset>>>(value: T) -> super::WhereParam {
            super::WhereParam::UpdatedAt(FieldOp::Gte(value.into()))
        }
        pub fn lte<T: Into<DateTime<FixedOffset>>>(value: T) -> super::WhereParam {
            super::WhereParam::UpdatedAt(FieldOp::Lte(value.into()))
        }
        pub fn in_vec<T: Into<DateTime<FixedOffset>>>(
            values: Vec<T>,
        ) -> super::WhereParam {
            super::WhereParam::UpdatedAt(
                FieldOp::InVec(values.into_iter().map(|v| v.into()).collect()),
            )
        }
        pub fn not_in_vec<T: Into<DateTime<FixedOffset>>>(
            values: Vec<T>,
        ) -> super::WhereParam {
            super::WhereParam::UpdatedAt(
                FieldOp::NotInVec(values.into_iter().map(|v| v.into()).collect()),
            )
        }
    }
    #[allow(dead_code)]
    pub mod user_id {
        use chrono::{NaiveDate, NaiveDateTime, DateTime, FixedOffset};
        use uuid::Uuid;
        use std::vec::Vec;
        use super::*;
        pub fn set<T: Into<i32>>(value: T) -> super::SetParam {
            super::SetParam::UserId(sea_orm::ActiveValue::Set(value.into()))
        }
        pub fn equals<T: Into<i32>>(value: T) -> super::WhereParam {
            super::WhereParam::UserId(FieldOp::Equals(value.into()))
        }
        pub fn order(order: caustics::SortOrder) -> super::OrderByParam {
            super::OrderByParam::UserId(order)
        }
        pub fn not_equals<T: Into<i32>>(value: T) -> super::WhereParam {
            super::WhereParam::UserId(FieldOp::NotEquals(value.into()))
        }
        pub fn gt<T: Into<i32>>(value: T) -> super::WhereParam {
            super::WhereParam::UserId(FieldOp::Gt(value.into()))
        }
        pub fn lt<T: Into<i32>>(value: T) -> super::WhereParam {
            super::WhereParam::UserId(FieldOp::Lt(value.into()))
        }
        pub fn gte<T: Into<i32>>(value: T) -> super::WhereParam {
            super::WhereParam::UserId(FieldOp::Gte(value.into()))
        }
        pub fn lte<T: Into<i32>>(value: T) -> super::WhereParam {
            super::WhereParam::UserId(FieldOp::Lte(value.into()))
        }
        pub fn in_vec<T: Into<i32>>(values: Vec<T>) -> super::WhereParam {
            super::WhereParam::UserId(
                FieldOp::InVec(values.into_iter().map(|v| v.into()).collect()),
            )
        }
        pub fn not_in_vec<T: Into<i32>>(values: Vec<T>) -> super::WhereParam {
            super::WhereParam::UserId(
                FieldOp::NotInVec(values.into_iter().map(|v| v.into()).collect()),
            )
        }
    }
    #[allow(dead_code)]
    pub mod reviewer_user_id {
        use chrono::{NaiveDate, NaiveDateTime, DateTime, FixedOffset};
        use uuid::Uuid;
        use std::vec::Vec;
        use super::*;
        pub fn set<T: Into<Option<i32>>>(value: T) -> super::SetParam {
            super::SetParam::ReviewerUserId(sea_orm::ActiveValue::Set(value.into()))
        }
        pub fn equals<T: Into<Option<i32>>>(value: T) -> super::WhereParam {
            super::WhereParam::ReviewerUserId(FieldOp::Equals(value.into()))
        }
        pub fn order(order: caustics::SortOrder) -> super::OrderByParam {
            super::OrderByParam::ReviewerUserId(order)
        }
        pub fn not_equals<T: Into<Option<i32>>>(value: T) -> super::WhereParam {
            super::WhereParam::ReviewerUserId(FieldOp::NotEquals(value.into()))
        }
        pub fn gt<T: Into<Option<i32>>>(value: T) -> super::WhereParam {
            super::WhereParam::ReviewerUserId(FieldOp::Gt(value.into()))
        }
        pub fn lt<T: Into<Option<i32>>>(value: T) -> super::WhereParam {
            super::WhereParam::ReviewerUserId(FieldOp::Lt(value.into()))
        }
        pub fn gte<T: Into<Option<i32>>>(value: T) -> super::WhereParam {
            super::WhereParam::ReviewerUserId(FieldOp::Gte(value.into()))
        }
        pub fn lte<T: Into<Option<i32>>>(value: T) -> super::WhereParam {
            super::WhereParam::ReviewerUserId(FieldOp::Lte(value.into()))
        }
        pub fn in_vec<T: Into<Option<i32>>>(values: Vec<T>) -> super::WhereParam {
            super::WhereParam::ReviewerUserId(
                FieldOp::InVec(values.into_iter().map(|v| v.into()).collect()),
            )
        }
        pub fn not_in_vec<T: Into<Option<i32>>>(values: Vec<T>) -> super::WhereParam {
            super::WhereParam::ReviewerUserId(
                FieldOp::NotInVec(values.into_iter().map(|v| v.into()).collect()),
            )
        }
    }
    pub fn and(params: Vec<super::WhereParam>) -> super::WhereParam {
        super::WhereParam::And(params)
    }
    pub fn or(params: Vec<super::WhereParam>) -> super::WhereParam {
        super::WhereParam::Or(params)
    }
    pub fn not(params: Vec<super::WhereParam>) -> super::WhereParam {
        super::WhereParam::Not(params)
    }
    impl MergeInto<ActiveModel> for SetParam {
        fn merge_into(&self, model: &mut ActiveModel) {
            match self {
                SetParam::Title(value) => {
                    model.title = value.clone();
                }
                SetParam::Content(value) => {
                    model.content = value.clone();
                }
                SetParam::CreatedAt(value) => {
                    model.created_at = value.clone();
                }
                SetParam::UpdatedAt(value) => {
                    model.updated_at = value.clone();
                }
                SetParam::UserId(value) => {
                    model.user_id = value.clone();
                }
                SetParam::ReviewerUserId(value) => {
                    model.reviewer_user_id = value.clone();
                }
                _ => {}
            }
        }
    }
    impl From<WhereParam> for Condition {
        fn from(param: WhereParam) -> Self {
            match param {
                _ => ::core::panicking::panic("not yet implemented"),
            }
        }
    }
    impl From<UniqueWhereParam> for Condition {
        fn from(param: UniqueWhereParam) -> Self {
            match param {
                UniqueWhereParam::IdEquals(value) => {
                    Condition::all().add(<Entity as EntityTrait>::Column::Id.eq(value))
                }
            }
        }
    }
    impl From<OrderByParam> for (<Entity as EntityTrait>::Column, sea_orm::Order) {
        fn from(param: OrderByParam) -> Self {
            match param {
                OrderByParam::Id(order) => {
                    let sea_order = match order {
                        SortOrder::Asc => sea_orm::Order::Asc,
                        SortOrder::Desc => sea_orm::Order::Desc,
                    };
                    (<Entity as EntityTrait>::Column::Id, sea_order)
                }
                OrderByParam::Title(order) => {
                    let sea_order = match order {
                        SortOrder::Asc => sea_orm::Order::Asc,
                        SortOrder::Desc => sea_orm::Order::Desc,
                    };
                    (<Entity as EntityTrait>::Column::Title, sea_order)
                }
                OrderByParam::Content(order) => {
                    let sea_order = match order {
                        SortOrder::Asc => sea_orm::Order::Asc,
                        SortOrder::Desc => sea_orm::Order::Desc,
                    };
                    (<Entity as EntityTrait>::Column::Content, sea_order)
                }
                OrderByParam::CreatedAt(order) => {
                    let sea_order = match order {
                        SortOrder::Asc => sea_orm::Order::Asc,
                        SortOrder::Desc => sea_orm::Order::Desc,
                    };
                    (<Entity as EntityTrait>::Column::CreatedAt, sea_order)
                }
                OrderByParam::UpdatedAt(order) => {
                    let sea_order = match order {
                        SortOrder::Asc => sea_orm::Order::Asc,
                        SortOrder::Desc => sea_orm::Order::Desc,
                    };
                    (<Entity as EntityTrait>::Column::UpdatedAt, sea_order)
                }
                OrderByParam::UserId(order) => {
                    let sea_order = match order {
                        SortOrder::Asc => sea_orm::Order::Asc,
                        SortOrder::Desc => sea_orm::Order::Desc,
                    };
                    (<Entity as EntityTrait>::Column::UserId, sea_order)
                }
                OrderByParam::ReviewerUserId(order) => {
                    let sea_order = match order {
                        SortOrder::Asc => sea_orm::Order::Asc,
                        SortOrder::Desc => sea_orm::Order::Desc,
                    };
                    (<Entity as EntityTrait>::Column::ReviewerUserId, sea_order)
                }
            }
        }
    }
    pub struct Create {
        pub title: String,
        pub created_at: DateTime<FixedOffset>,
        pub updated_at: DateTime<FixedOffset>,
        pub user: super::user::UniqueWhereParam,
        pub _params: Vec<SetParam>,
    }
    impl Create {
        fn into_active_model<C: sea_orm::ConnectionTrait>(
            mut self,
        ) -> (ActiveModel, Vec<caustics::DeferredLookup<C>>) {
            let mut model = ActiveModel::new();
            let mut deferred_lookups = Vec::new();
            model.title = sea_orm::ActiveValue::Set(self.title);
            model.created_at = sea_orm::ActiveValue::Set(self.created_at);
            model.updated_at = sea_orm::ActiveValue::Set(self.updated_at);
            match self.user {
                super::user::UniqueWhereParam::IdEquals(id) => {
                    model.user_id = sea_orm::ActiveValue::Set(id.clone());
                }
                other => {
                    deferred_lookups
                        .push(
                            caustics::DeferredLookup::<
                                C,
                            >::new(
                                Box::new(other.clone()),
                                |model, value| {
                                    let model = model.downcast_mut::<ActiveModel>().unwrap();
                                    model.user_id = sea_orm::ActiveValue::Set(value);
                                },
                                |conn: &C, param| {
                                    let param = param
                                        .downcast_ref::<super::user::UniqueWhereParam>()
                                        .unwrap()
                                        .clone();
                                    Box::pin(async move {
                                        let condition: sea_query::Condition = param.clone().into();
                                        let result = super::user::Entity::find()
                                            .filter::<sea_query::Condition>(condition)
                                            .one(conn)
                                            .await?;
                                        result
                                            .map(|entity| entity.id)
                                            .ok_or_else(|| {
                                                sea_orm::DbErr::Custom(
                                                    ::alloc::__export::must_use({
                                                        ::alloc::fmt::format(
                                                            format_args!(
                                                                "No {0} found for condition: {1:?}",
                                                                "super :: user",
                                                                param,
                                                            ),
                                                        )
                                                    }),
                                                )
                                            })
                                    })
                                },
                            ),
                        );
                }
            }
            for param in self._params {
                match param {
                    SetParam::ConnectUser(where_param) => {
                        match where_param {
                            super::user::UniqueWhereParam::IdEquals(id) => {
                                model.user_id = sea_orm::ActiveValue::Set(id.clone());
                            }
                            other => {
                                deferred_lookups
                                    .push(
                                        caustics::DeferredLookup::<
                                            C,
                                        >::new(
                                            Box::new(other.clone()),
                                            |model, value| {
                                                let model = model.downcast_mut::<ActiveModel>().unwrap();
                                                model.user_id = sea_orm::ActiveValue::Set(value);
                                            },
                                            |conn: &C, param| {
                                                let param = param
                                                    .downcast_ref::<super::user::UniqueWhereParam>()
                                                    .unwrap()
                                                    .clone();
                                                Box::pin(async move {
                                                    let condition: sea_query::Condition = param.clone().into();
                                                    let result = super::user::Entity::find()
                                                        .filter::<sea_query::Condition>(condition)
                                                        .one(conn)
                                                        .await?;
                                                    result
                                                        .map(|entity| entity.id)
                                                        .ok_or_else(|| {
                                                            sea_orm::DbErr::Custom(
                                                                ::alloc::__export::must_use({
                                                                    ::alloc::fmt::format(
                                                                        format_args!(
                                                                            "No {0} found for condition: {1:?}",
                                                                            "super :: user",
                                                                            param,
                                                                        ),
                                                                    )
                                                                }),
                                                            )
                                                        })
                                                })
                                            },
                                        ),
                                    );
                            }
                        }
                    }
                    SetParam::ConnectReviewer(where_param) => {
                        match where_param {
                            super::user::UniqueWhereParam::IdEquals(id) => {
                                model.reviewer_user_id = sea_orm::ActiveValue::Set(
                                    Some(id.clone()),
                                );
                            }
                            other => {}
                        }
                    }
                    SetParam::DisconnectReviewer => {
                        model.reviewer_user_id = sea_orm::ActiveValue::Set(None);
                    }
                    other => {
                        other.merge_into(&mut model);
                    }
                }
            }
            (model, deferred_lookups)
        }
    }
    pub type Filter = caustics::Filter;
    pub struct RelationFilter {
        pub relation: &'static str,
        pub filters: Vec<Filter>,
    }
    #[automatically_derived]
    impl ::core::clone::Clone for RelationFilter {
        #[inline]
        fn clone(&self) -> RelationFilter {
            RelationFilter {
                relation: ::core::clone::Clone::clone(&self.relation),
                filters: ::core::clone::Clone::clone(&self.filters),
            }
        }
    }
    impl caustics::RelationFilterTrait for RelationFilter {
        fn relation_name(&self) -> &'static str {
            self.relation
        }
        fn filters(&self) -> &[caustics::Filter] {
            &self.filters
        }
    }
    impl From<RelationFilter> for caustics::RelationFilter {
        fn from(relation_filter: RelationFilter) -> Self {
            caustics::RelationFilter {
                relation: relation_filter.relation,
                filters: relation_filter.filters,
            }
        }
    }
    pub struct ModelWithRelations {
        pub id: i32,
        pub title: String,
        pub content: Option<String>,
        pub created_at: DateTime<FixedOffset>,
        pub updated_at: DateTime<FixedOffset>,
        pub user_id: i32,
        pub reviewer_user_id: Option<i32>,
        pub user: Option<super::user::ModelWithRelations>,
        pub reviewer: Option<Option<super::user::ModelWithRelations>>,
    }
    #[automatically_derived]
    impl ::core::fmt::Debug for ModelWithRelations {
        #[inline]
        fn fmt(&self, f: &mut ::core::fmt::Formatter) -> ::core::fmt::Result {
            let names: &'static _ = &[
                "id",
                "title",
                "content",
                "created_at",
                "updated_at",
                "user_id",
                "reviewer_user_id",
                "user",
                "reviewer",
            ];
            let values: &[&dyn ::core::fmt::Debug] = &[
                &self.id,
                &self.title,
                &self.content,
                &self.created_at,
                &self.updated_at,
                &self.user_id,
                &self.reviewer_user_id,
                &self.user,
                &&self.reviewer,
            ];
            ::core::fmt::Formatter::debug_struct_fields_finish(
                f,
                "ModelWithRelations",
                names,
                values,
            )
        }
    }
    #[automatically_derived]
    impl ::core::clone::Clone for ModelWithRelations {
        #[inline]
        fn clone(&self) -> ModelWithRelations {
            ModelWithRelations {
                id: ::core::clone::Clone::clone(&self.id),
                title: ::core::clone::Clone::clone(&self.title),
                content: ::core::clone::Clone::clone(&self.content),
                created_at: ::core::clone::Clone::clone(&self.created_at),
                updated_at: ::core::clone::Clone::clone(&self.updated_at),
                user_id: ::core::clone::Clone::clone(&self.user_id),
                reviewer_user_id: ::core::clone::Clone::clone(&self.reviewer_user_id),
                user: ::core::clone::Clone::clone(&self.user),
                reviewer: ::core::clone::Clone::clone(&self.reviewer),
            }
        }
    }
    #[automatically_derived]
    impl ::core::marker::StructuralPartialEq for ModelWithRelations {}
    #[automatically_derived]
    impl ::core::cmp::PartialEq for ModelWithRelations {
        #[inline]
        fn eq(&self, other: &ModelWithRelations) -> bool {
            self.id == other.id && self.title == other.title
                && self.content == other.content && self.created_at == other.created_at
                && self.updated_at == other.updated_at && self.user_id == other.user_id
                && self.reviewer_user_id == other.reviewer_user_id
                && self.user == other.user && self.reviewer == other.reviewer
        }
    }
    #[automatically_derived]
    impl ::core::cmp::Eq for ModelWithRelations {
        #[inline]
        #[doc(hidden)]
        #[coverage(off)]
        fn assert_receiver_is_total_eq(&self) -> () {
            let _: ::core::cmp::AssertParamIsEq<i32>;
            let _: ::core::cmp::AssertParamIsEq<String>;
            let _: ::core::cmp::AssertParamIsEq<Option<String>>;
            let _: ::core::cmp::AssertParamIsEq<DateTime<FixedOffset>>;
            let _: ::core::cmp::AssertParamIsEq<DateTime<FixedOffset>>;
            let _: ::core::cmp::AssertParamIsEq<Option<i32>>;
            let _: ::core::cmp::AssertParamIsEq<Option<super::user::ModelWithRelations>>;
            let _: ::core::cmp::AssertParamIsEq<
                Option<Option<super::user::ModelWithRelations>>,
            >;
        }
    }
    #[automatically_derived]
    impl ::core::hash::Hash for ModelWithRelations {
        #[inline]
        fn hash<__H: ::core::hash::Hasher>(&self, state: &mut __H) -> () {
            ::core::hash::Hash::hash(&self.id, state);
            ::core::hash::Hash::hash(&self.title, state);
            ::core::hash::Hash::hash(&self.content, state);
            ::core::hash::Hash::hash(&self.created_at, state);
            ::core::hash::Hash::hash(&self.updated_at, state);
            ::core::hash::Hash::hash(&self.user_id, state);
            ::core::hash::Hash::hash(&self.reviewer_user_id, state);
            ::core::hash::Hash::hash(&self.user, state);
            ::core::hash::Hash::hash(&self.reviewer, state)
        }
    }
    impl ModelWithRelations {
        pub fn new(
            id: i32,
            title: String,
            content: Option<String>,
            created_at: DateTime<FixedOffset>,
            updated_at: DateTime<FixedOffset>,
            user_id: i32,
            reviewer_user_id: Option<i32>,
            user: Option<super::user::ModelWithRelations>,
            reviewer: Option<Option<super::user::ModelWithRelations>>,
        ) -> Self {
            Self {
                id,
                title,
                content,
                created_at,
                updated_at,
                user_id,
                reviewer_user_id,
                user,
                reviewer,
            }
        }
        pub fn from_model(model: Model) -> Self {
            Self {
                id: model.id,
                title: model.title,
                content: model.content,
                created_at: model.created_at,
                updated_at: model.updated_at,
                user_id: model.user_id,
                reviewer_user_id: model.reviewer_user_id,
                user: None,
                reviewer: None,
            }
        }
    }
    impl std::default::Default for ModelWithRelations {
        fn default() -> Self {
            Self {
                id: Default::default(),
                title: Default::default(),
                content: Default::default(),
                created_at: Default::default(),
                updated_at: Default::default(),
                user_id: Default::default(),
                reviewer_user_id: Default::default(),
                user: None,
                reviewer: None,
            }
        }
    }
    impl caustics::FromModel<Model> for ModelWithRelations {
        fn from_model(model: Model) -> Self {
            Self::from_model(model)
        }
    }
    static RELATION_DESCRIPTORS: &[caustics::RelationDescriptor<ModelWithRelations>] = &[
        caustics::RelationDescriptor::<ModelWithRelations> {
            name: "user",
            set_field: |model, value| {
                let value = value
                    .downcast::<Option<super::user::ModelWithRelations>>()
                    .expect("Type mismatch in set_field");
                model.user = *value;
            },
            get_foreign_key: |model| Some(model.user_id),
            target_entity: "Path { leading_colon: None, segments: [PathSegment { ident: Ident { ident: \"super\", span: #30 bytes(1176..1207) }, arguments: PathArguments::None }, PathSep, PathSegment { ident: Ident { ident: \"user\", span: #30 bytes(1176..1207) }, arguments: PathArguments::None }] }",
            foreign_key_column: "user_id",
        },
        caustics::RelationDescriptor::<ModelWithRelations> {
            name: "reviewer",
            set_field: |model, value| {
                let value = value
                    .downcast::<Option<Option<super::user::ModelWithRelations>>>()
                    .expect("Type mismatch in set_field");
                model.reviewer = *value;
            },
            get_foreign_key: |model| model.reviewer_user_id,
            target_entity: "Path { leading_colon: None, segments: [PathSegment { ident: Ident { ident: \"super\", span: #30 bytes(1176..1207) }, arguments: PathArguments::None }, PathSep, PathSegment { ident: Ident { ident: \"user\", span: #30 bytes(1176..1207) }, arguments: PathArguments::None }] }",
            foreign_key_column: "reviewer_user_id",
        },
    ];
    impl caustics::HasRelationMetadata<ModelWithRelations> for ModelWithRelations {
        fn relation_descriptors() -> &'static [caustics::RelationDescriptor<
            ModelWithRelations,
        >] {
            RELATION_DESCRIPTORS
        }
    }
    #[allow(dead_code)]
    impl<
        'a,
        C: sea_orm::ConnectionTrait + sea_orm::TransactionTrait,
    > EntityClient<'a, C> {
        pub fn new(conn: &'a C) -> Self {
            Self { conn }
        }
        pub fn find_unique(
            &self,
            condition: UniqueWhereParam,
        ) -> caustics::UniqueQueryBuilder<'a, C, Entity, ModelWithRelations> {
            let registry = super::get_registry();
            caustics::UniqueQueryBuilder {
                query: <Entity as EntityTrait>::find()
                    .filter::<Condition>(condition.clone().into()),
                conn: self.conn,
                relations_to_fetch: ::alloc::vec::Vec::new(),
                registry,
                _phantom: std::marker::PhantomData,
            }
        }
        pub fn find_first(
            &self,
            conditions: Vec<WhereParam>,
        ) -> caustics::FirstQueryBuilder<'a, C, Entity, ModelWithRelations> {
            let registry = super::get_registry();
            let mut query = <Entity as EntityTrait>::find();
            for cond in conditions {
                query = query.filter::<Condition>(cond.into());
            }
            caustics::FirstQueryBuilder {
                query,
                conn: self.conn,
                relations_to_fetch: ::alloc::vec::Vec::new(),
                registry,
                _phantom: std::marker::PhantomData,
            }
        }
        pub fn find_many(
            &self,
            conditions: Vec<WhereParam>,
        ) -> caustics::ManyQueryBuilder<'a, C, Entity, ModelWithRelations> {
            let registry = super::get_registry();
            let mut query = <Entity as EntityTrait>::find();
            for cond in conditions {
                query = query.filter::<Condition>(cond.into());
            }
            caustics::ManyQueryBuilder {
                query,
                conn: self.conn,
                relations_to_fetch: ::alloc::vec::Vec::new(),
                registry,
                _phantom: std::marker::PhantomData,
            }
        }
        pub fn create(
            &self,
            title: String,
            created_at: DateTime<FixedOffset>,
            updated_at: DateTime<FixedOffset>,
            user: super::user::UniqueWhereParam,
            _params: Vec<SetParam>,
        ) -> caustics::CreateQueryBuilder<
            'a,
            C,
            Entity,
            ActiveModel,
            ModelWithRelations,
        > {
            let create = Create {
                title,
                created_at,
                updated_at,
                user,
                _params,
            };
            let (model, deferred_lookups) = create.into_active_model::<C>();
            caustics::CreateQueryBuilder {
                model,
                conn: self.conn,
                deferred_lookups,
                _phantom: std::marker::PhantomData,
            }
        }
        pub fn update(
            &self,
            condition: UniqueWhereParam,
            changes: Vec<SetParam>,
        ) -> caustics::UpdateQueryBuilder<
            'a,
            C,
            Entity,
            ActiveModel,
            ModelWithRelations,
            SetParam,
        > {
            caustics::UpdateQueryBuilder {
                condition: condition.into(),
                changes,
                conn: self.conn,
                _phantom: std::marker::PhantomData,
            }
        }
        pub fn delete(
            &self,
            condition: UniqueWhereParam,
        ) -> caustics::DeleteQueryBuilder<'a, C, Entity> {
            caustics::DeleteQueryBuilder {
                condition: condition.into(),
                conn: self.conn,
                _phantom: std::marker::PhantomData,
            }
        }
        pub fn upsert(
            &self,
            condition: UniqueWhereParam,
            create: Create,
            update: Vec<SetParam>,
        ) -> caustics::UpsertQueryBuilder<
            'a,
            C,
            Entity,
            ActiveModel,
            ModelWithRelations,
            SetParam,
        > {
            let (model, deferred_lookups) = create.into_active_model::<C>();
            caustics::UpsertQueryBuilder {
                condition: condition.into(),
                create: (model, deferred_lookups),
                update,
                conn: self.conn,
                _phantom: std::marker::PhantomData,
            }
        }
        pub async fn _batch(
            &self,
            queries: Vec<
                caustics::BatchQuery<
                    'a,
                    sea_orm::DatabaseTransaction,
                    Entity,
                    ActiveModel,
                    ModelWithRelations,
                    SetParam,
                >,
            >,
        ) -> Result<Vec<caustics::BatchResult<ModelWithRelations>>, sea_orm::DbErr>
        where
            Entity: sea_orm::EntityTrait,
            ActiveModel: sea_orm::ActiveModelTrait<Entity = Entity>
                + sea_orm::ActiveModelBehavior + Send + 'static,
            ModelWithRelations: caustics::FromModel<
                <Entity as sea_orm::EntityTrait>::Model,
            >,
            SetParam: caustics::MergeInto<ActiveModel>,
            <Entity as sea_orm::EntityTrait>::Model: sea_orm::IntoActiveModel<
                ActiveModel,
            >,
        {
            let txn = self.conn.begin().await?;
            let mut results = Vec::with_capacity(queries.len());
            for query in queries {
                let res = match query {
                    caustics::BatchQuery::Insert(q) => {
                        let model = q.model;
                        let result = model
                            .insert(&txn)
                            .await
                            .map(ModelWithRelations::from_model)?;
                        caustics::BatchResult::Insert(result)
                    }
                    caustics::BatchQuery::Update(q) => {
                        caustics::BatchResult::Update(ModelWithRelations::default())
                    }
                    caustics::BatchQuery::Delete(q) => caustics::BatchResult::Delete(()),
                    caustics::BatchQuery::Upsert(q) => {
                        caustics::BatchResult::Upsert(ModelWithRelations::default())
                    }
                };
                results.push(res);
            }
            txn.commit().await?;
            Ok(results)
        }
    }
    #[allow(dead_code)]
    pub mod user {
        pub fn fetch() -> super::RelationFilter {
            super::RelationFilter {
                relation: "user",
                filters: ::alloc::vec::Vec::new(),
            }
        }
        pub fn connect(
            where_param: super::super::user::UniqueWhereParam,
        ) -> super::SetParam {
            super::SetParam::ConnectUser(where_param)
        }
    }
    #[allow(dead_code)]
    pub mod reviewer {
        pub fn fetch() -> super::RelationFilter {
            super::RelationFilter {
                relation: "reviewer",
                filters: ::alloc::vec::Vec::new(),
            }
        }
        pub fn connect(
            where_param: super::super::user::UniqueWhereParam,
        ) -> super::SetParam {
            super::SetParam::ConnectReviewer(where_param)
        }
        pub fn disconnect() -> super::SetParam {
            super::SetParam::DisconnectReviewer
        }
    }
    pub(crate) fn column_from_str(
        name: &str,
    ) -> Option<<Entity as sea_orm::EntityTrait>::Column> {
        match name {
            "id" => Some(<Entity as sea_orm::EntityTrait>::Column::Id),
            "title" => Some(<Entity as sea_orm::EntityTrait>::Column::Title),
            "content" => Some(<Entity as sea_orm::EntityTrait>::Column::Content),
            "created_at" => Some(<Entity as sea_orm::EntityTrait>::Column::CreatedAt),
            "updated_at" => Some(<Entity as sea_orm::EntityTrait>::Column::UpdatedAt),
            "user_id" => Some(<Entity as sea_orm::EntityTrait>::Column::UserId),
            "reviewer_user_id" => {
                Some(<Entity as sea_orm::EntityTrait>::Column::ReviewerUserId)
            }
            _ => None,
        }
    }
    pub struct EntityFetcherImpl;
    impl<C: sea_orm::ConnectionTrait> caustics::EntityFetcher<C> for EntityFetcherImpl {
        fn fetch_by_foreign_key<'a>(
            &'a self,
            conn: &'a C,
            foreign_key_value: Option<i32>,
            foreign_key_column: &'a str,
            target_entity: &'a str,
            relation_name: &'a str,
        ) -> std::pin::Pin<
            Box<
                dyn std::future::Future<
                    Output = Result<Box<dyn std::any::Any + Send>, sea_orm::DbErr>,
                > + Send + 'a,
            >,
        > {
            Box::pin(async move {
                match relation_name {
                    "user" => {
                        if let Some(fk_value) = foreign_key_value {
                            let condition = super::user::UniqueWhereParam::IdEquals(
                                fk_value,
                            );
                            let opt_model = <super::user::Entity as EntityTrait>::find()
                                .filter::<sea_query::Condition>(condition.into())
                                .one(conn)
                                .await?;
                            let with_rel = opt_model
                                .map(super::user::ModelWithRelations::from_model);
                            return Ok(
                                Box::new(with_rel) as Box<dyn std::any::Any + Send>,
                            );
                        } else {
                            Ok(Box::new(()) as Box<dyn std::any::Any + Send>)
                        }
                    }
                    "reviewer" => {
                        if let Some(fk_value) = foreign_key_value {
                            let condition = super::user::UniqueWhereParam::IdEquals(
                                fk_value,
                            );
                            let opt_model = <super::user::Entity as EntityTrait>::find()
                                .filter::<sea_query::Condition>(condition.into())
                                .one(conn)
                                .await?;
                            let with_rel = opt_model
                                .map(super::user::ModelWithRelations::from_model);
                            let result: Option<
                                Option<super::user::ModelWithRelations>,
                            > = Some(with_rel);
                            return Ok(Box::new(result) as Box<dyn std::any::Any + Send>);
                        } else {
                            return Ok(
                                Box::new(None::<Option<super::user::ModelWithRelations>>)
                                    as Box<dyn std::any::Any + Send>,
                            );
                        }
                    }
                    _ => {
                        Err(
                            sea_orm::DbErr::Custom(
                                ::alloc::__export::must_use({
                                    ::alloc::fmt::format(
                                        format_args!("Unknown relation: {0}", relation_name),
                                    )
                                }),
                            ),
                        )
                    }
                }
            })
        }
    }
    impl FromModel<Model> for Model {
        fn from_model(m: Model) -> Self {
            m
        }
    }
    impl sea_orm::ActiveModelBehavior for ActiveModel {}
}
pub mod helpers {
    use sea_orm::{Database, DatabaseConnection, Schema};
    use super::{post, user};
    pub async fn setup_test_db() -> DatabaseConnection {
        use sea_orm::ConnectionTrait;
        let db = Database::connect("sqlite::memory:?mode=rwc").await.unwrap();
        let schema = Schema::new(db.get_database_backend());
        let mut user_table = schema.create_table_from_entity(user::Entity);
        let create_users = user_table.if_not_exists();
        let create_users_sql = db.get_database_backend().build(create_users);
        db.execute(create_users_sql).await.unwrap();
        let mut post_table = schema.create_table_from_entity(post::Entity);
        let create_posts = post_table.if_not_exists();
        let create_posts_sql = db.get_database_backend().build(create_posts);
        db.execute(create_posts_sql).await.unwrap();
        db
    }
}
mod client_tests {
    use super::helpers::setup_test_db;
    use super::*;
    extern crate test;
    #[rustc_test_marker = "client_tests::test_caustics_client"]
    #[doc(hidden)]
    pub const test_caustics_client: test::TestDescAndFn = test::TestDescAndFn {
        desc: test::TestDesc {
            name: test::StaticTestName("client_tests::test_caustics_client"),
            ignore: false,
            ignore_message: ::core::option::Option::None,
            source_file: "caustics/tests/blog_test.rs",
            start_line: 131usize,
            start_col: 14usize,
            end_line: 131usize,
            end_col: 34usize,
            compile_fail: false,
            no_run: false,
            should_panic: test::ShouldPanic::No,
            test_type: test::TestType::IntegrationTest,
        },
        testfn: test::StaticTestFn(
            #[coverage(off)]
            || test::assert_test_result(test_caustics_client()),
        ),
    };
    fn test_caustics_client() {
        let body = async {
            let db = setup_test_db().await;
            let client = CausticsClient::new(db.clone());
            if !client.db().ping().await.is_ok() {
                ::core::panicking::panic(
                    "assertion failed: client.db().ping().await.is_ok()",
                )
            }
        };
        let mut body = body;
        #[allow(unused_mut)]
        let mut body = unsafe {
            ::tokio::macros::support::Pin::new_unchecked(&mut body)
        };
        let body: ::core::pin::Pin<&mut dyn ::core::future::Future<Output = ()>> = body;
        #[allow(
            clippy::expect_used,
            clippy::diverging_sub_expression,
            clippy::needless_return
        )]
        {
            return tokio::runtime::Builder::new_current_thread()
                .enable_all()
                .build()
                .expect("Failed building the Runtime")
                .block_on(body);
        }
    }
}
mod query_builder_tests {
    use std::str::FromStr;
    use caustics::{QueryError, SortOrder};
    use chrono::{DateTime, FixedOffset};
    use super::helpers::setup_test_db;
    use super::*;
    extern crate test;
    #[rustc_test_marker = "query_builder_tests::test_find_operations"]
    #[doc(hidden)]
    pub const test_find_operations: test::TestDescAndFn = test::TestDescAndFn {
        desc: test::TestDesc {
            name: test::StaticTestName("query_builder_tests::test_find_operations"),
            ignore: false,
            ignore_message: ::core::option::Option::None,
            source_file: "caustics/tests/blog_test.rs",
            start_line: 152usize,
            start_col: 14usize,
            end_line: 152usize,
            end_col: 34usize,
            compile_fail: false,
            no_run: false,
            should_panic: test::ShouldPanic::No,
            test_type: test::TestType::IntegrationTest,
        },
        testfn: test::StaticTestFn(
            #[coverage(off)]
            || test::assert_test_result(test_find_operations()),
        ),
    };
    fn test_find_operations() {
        let body = async {
            let db = setup_test_db().await;
            let client = CausticsClient::new(db.clone());
            let user = client
                .user()
                .find_unique(user::id::equals(1))
                .exec()
                .await
                .unwrap();
            if !user.is_none() {
                ::core::panicking::panic("assertion failed: user.is_none()")
            }
            let user = client
                .user()
                .find_first(
                    <[_]>::into_vec(
                        ::alloc::boxed::box_new([
                            user::name::equals("John"),
                            user::age::gt(18),
                        ]),
                    ),
                )
                .exec()
                .await
                .unwrap();
            if !user.is_none() {
                ::core::panicking::panic("assertion failed: user.is_none()")
            }
            let users = client
                .user()
                .find_many(<[_]>::into_vec(::alloc::boxed::box_new([user::age::gt(18)])))
                .exec()
                .await
                .unwrap();
            if !users.is_empty() {
                ::core::panicking::panic("assertion failed: users.is_empty()")
            }
        };
        let mut body = body;
        #[allow(unused_mut)]
        let mut body = unsafe {
            ::tokio::macros::support::Pin::new_unchecked(&mut body)
        };
        let body: ::core::pin::Pin<&mut dyn ::core::future::Future<Output = ()>> = body;
        #[allow(
            clippy::expect_used,
            clippy::diverging_sub_expression,
            clippy::needless_return
        )]
        {
            return tokio::runtime::Builder::new_current_thread()
                .enable_all()
                .build()
                .expect("Failed building the Runtime")
                .block_on(body);
        }
    }
    extern crate test;
    #[rustc_test_marker = "query_builder_tests::test_create_operations"]
    #[doc(hidden)]
    pub const test_create_operations: test::TestDescAndFn = test::TestDescAndFn {
        desc: test::TestDesc {
            name: test::StaticTestName("query_builder_tests::test_create_operations"),
            ignore: false,
            ignore_message: ::core::option::Option::None,
            source_file: "caustics/tests/blog_test.rs",
            start_line: 185usize,
            start_col: 14usize,
            end_line: 185usize,
            end_col: 36usize,
            compile_fail: false,
            no_run: false,
            should_panic: test::ShouldPanic::No,
            test_type: test::TestType::IntegrationTest,
        },
        testfn: test::StaticTestFn(
            #[coverage(off)]
            || test::assert_test_result(test_create_operations()),
        ),
    };
    fn test_create_operations() {
        let body = async {
            let db = setup_test_db().await;
            let client = CausticsClient::new(db.clone());
            let email = ::alloc::__export::must_use({
                ::alloc::fmt::format(
                    format_args!("john_{0}@example.com", chrono::Utc::now().timestamp()),
                )
            });
            let user = client
                .user()
                .create(
                    email.clone(),
                    "John".to_string(),
                    DateTime::<FixedOffset>::from_str("2021-01-01T00:00:00Z").unwrap(),
                    DateTime::<FixedOffset>::from_str("2021-01-01T00:00:00Z").unwrap(),
                    <[_]>::into_vec(
                        ::alloc::boxed::box_new([
                            user::age::set(Some(25)),
                            user::deleted_at::set(None),
                        ]),
                    ),
                )
                .exec()
                .await
                .unwrap();
            let found_user = client
                .user()
                .find_first(
                    <[_]>::into_vec(
                        ::alloc::boxed::box_new([user::email::equals(&email)]),
                    ),
                )
                .exec()
                .await
                .unwrap()
                .unwrap();
            match (&found_user.name, &"John") {
                (left_val, right_val) => {
                    if !(*left_val == *right_val) {
                        let kind = ::core::panicking::AssertKind::Eq;
                        ::core::panicking::assert_failed(
                            kind,
                            &*left_val,
                            &*right_val,
                            ::core::option::Option::None,
                        );
                    }
                }
            };
            match (&found_user.email, &email) {
                (left_val, right_val) => {
                    if !(*left_val == *right_val) {
                        let kind = ::core::panicking::AssertKind::Eq;
                        ::core::panicking::assert_failed(
                            kind,
                            &*left_val,
                            &*right_val,
                            ::core::option::Option::None,
                        );
                    }
                }
            };
            match (&found_user.age, &Some(25)) {
                (left_val, right_val) => {
                    if !(*left_val == *right_val) {
                        let kind = ::core::panicking::AssertKind::Eq;
                        ::core::panicking::assert_failed(
                            kind,
                            &*left_val,
                            &*right_val,
                            ::core::option::Option::None,
                        );
                    }
                }
            };
            let post = client
                .post()
                .create(
                    "Hello World".to_string(),
                    DateTime::<FixedOffset>::from_str("2021-01-01T00:00:00Z").unwrap(),
                    DateTime::<FixedOffset>::from_str("2021-01-01T00:00:00Z").unwrap(),
                    user::id::equals(user.id),
                    <[_]>::into_vec(
                        ::alloc::boxed::box_new([
                            post::content::set(Some("This is my first post".to_string())),
                        ]),
                    ),
                )
                .exec()
                .await
                .unwrap();
            let found_post = client
                .post()
                .find_first(
                    <[_]>::into_vec(::alloc::boxed::box_new([post::id::equals(post.id)])),
                )
                .exec()
                .await
                .unwrap()
                .unwrap();
            match (&found_post.title, &"Hello World") {
                (left_val, right_val) => {
                    if !(*left_val == *right_val) {
                        let kind = ::core::panicking::AssertKind::Eq;
                        ::core::panicking::assert_failed(
                            kind,
                            &*left_val,
                            &*right_val,
                            ::core::option::Option::None,
                        );
                    }
                }
            };
            match (&found_post.content, &Some("This is my first post".to_string())) {
                (left_val, right_val) => {
                    if !(*left_val == *right_val) {
                        let kind = ::core::panicking::AssertKind::Eq;
                        ::core::panicking::assert_failed(
                            kind,
                            &*left_val,
                            &*right_val,
                            ::core::option::Option::None,
                        );
                    }
                }
            };
            match (&found_post.user_id, &user.id) {
                (left_val, right_val) => {
                    if !(*left_val == *right_val) {
                        let kind = ::core::panicking::AssertKind::Eq;
                        ::core::panicking::assert_failed(
                            kind,
                            &*left_val,
                            &*right_val,
                            ::core::option::Option::None,
                        );
                    }
                }
            };
        };
        let mut body = body;
        #[allow(unused_mut)]
        let mut body = unsafe {
            ::tokio::macros::support::Pin::new_unchecked(&mut body)
        };
        let body: ::core::pin::Pin<&mut dyn ::core::future::Future<Output = ()>> = body;
        #[allow(
            clippy::expect_used,
            clippy::diverging_sub_expression,
            clippy::needless_return
        )]
        {
            return tokio::runtime::Builder::new_current_thread()
                .enable_all()
                .build()
                .expect("Failed building the Runtime")
                .block_on(body);
        }
    }
    extern crate test;
    #[rustc_test_marker = "query_builder_tests::test_update_operations"]
    #[doc(hidden)]
    pub const test_update_operations: test::TestDescAndFn = test::TestDescAndFn {
        desc: test::TestDesc {
            name: test::StaticTestName("query_builder_tests::test_update_operations"),
            ignore: false,
            ignore_message: ::core::option::Option::None,
            source_file: "caustics/tests/blog_test.rs",
            start_line: 247usize,
            start_col: 14usize,
            end_line: 247usize,
            end_col: 36usize,
            compile_fail: false,
            no_run: false,
            should_panic: test::ShouldPanic::No,
            test_type: test::TestType::IntegrationTest,
        },
        testfn: test::StaticTestFn(
            #[coverage(off)]
            || test::assert_test_result(test_update_operations()),
        ),
    };
    fn test_update_operations() {
        let body = async {
            let db = setup_test_db().await;
            let client = CausticsClient::new(db.clone());
            let email = ::alloc::__export::must_use({
                ::alloc::fmt::format(
                    format_args!("john_{0}@example.com", chrono::Utc::now().timestamp()),
                )
            });
            let user = client
                .user()
                .create(
                    email.clone(),
                    "John".to_string(),
                    DateTime::<FixedOffset>::from_str("2021-01-01T00:00:00Z").unwrap(),
                    DateTime::<FixedOffset>::from_str("2021-01-01T00:00:00Z").unwrap(),
                    <[_]>::into_vec(
                        ::alloc::boxed::box_new([
                            user::age::set(Some(25)),
                            user::deleted_at::set(None),
                        ]),
                    ),
                )
                .exec()
                .await
                .unwrap();
            let updated_user = client
                .user()
                .update(
                    user::id::equals(user.id),
                    <[_]>::into_vec(
                        ::alloc::boxed::box_new([
                            user::name::set("John Doe"),
                            user::age::set(Some(26)),
                        ]),
                    ),
                )
                .exec()
                .await
                .unwrap();
            match (&updated_user.name, &"John Doe") {
                (left_val, right_val) => {
                    if !(*left_val == *right_val) {
                        let kind = ::core::panicking::AssertKind::Eq;
                        ::core::panicking::assert_failed(
                            kind,
                            &*left_val,
                            &*right_val,
                            ::core::option::Option::None,
                        );
                    }
                }
            };
            match (&updated_user.age, &Some(26)) {
                (left_val, right_val) => {
                    if !(*left_val == *right_val) {
                        let kind = ::core::panicking::AssertKind::Eq;
                        ::core::panicking::assert_failed(
                            kind,
                            &*left_val,
                            &*right_val,
                            ::core::option::Option::None,
                        );
                    }
                }
            };
        };
        let mut body = body;
        #[allow(unused_mut)]
        let mut body = unsafe {
            ::tokio::macros::support::Pin::new_unchecked(&mut body)
        };
        let body: ::core::pin::Pin<&mut dyn ::core::future::Future<Output = ()>> = body;
        #[allow(
            clippy::expect_used,
            clippy::diverging_sub_expression,
            clippy::needless_return
        )]
        {
            return tokio::runtime::Builder::new_current_thread()
                .enable_all()
                .build()
                .expect("Failed building the Runtime")
                .block_on(body);
        }
    }
    extern crate test;
    #[rustc_test_marker = "query_builder_tests::test_pagination_and_sorting"]
    #[doc(hidden)]
    pub const test_pagination_and_sorting: test::TestDescAndFn = test::TestDescAndFn {
        desc: test::TestDesc {
            name: test::StaticTestName(
                "query_builder_tests::test_pagination_and_sorting",
            ),
            ignore: false,
            ignore_message: ::core::option::Option::None,
            source_file: "caustics/tests/blog_test.rs",
            start_line: 281usize,
            start_col: 14usize,
            end_line: 281usize,
            end_col: 41usize,
            compile_fail: false,
            no_run: false,
            should_panic: test::ShouldPanic::No,
            test_type: test::TestType::IntegrationTest,
        },
        testfn: test::StaticTestFn(
            #[coverage(off)]
            || test::assert_test_result(test_pagination_and_sorting()),
        ),
    };
    fn test_pagination_and_sorting() {
        let body = async {
            let db = setup_test_db().await;
            let client = CausticsClient::new(db.clone());
            for i in 0..5 {
                client
                    .user()
                    .create(
                        ::alloc::__export::must_use({
                            ::alloc::fmt::format(format_args!("user{0}@example.com", i))
                        }),
                        ::alloc::__export::must_use({
                            ::alloc::fmt::format(format_args!("User {0}", i))
                        }),
                        DateTime::<FixedOffset>::from_str("2021-01-01T00:00:00Z")
                            .unwrap(),
                        DateTime::<FixedOffset>::from_str("2021-01-01T00:00:00Z")
                            .unwrap(),
                        <[_]>::into_vec(
                            ::alloc::boxed::box_new([
                                user::age::set(Some(20 + i)),
                                user::deleted_at::set(None),
                            ]),
                        ),
                    )
                    .exec()
                    .await
                    .unwrap();
            }
            let users = client
                .user()
                .find_many(::alloc::vec::Vec::new())
                .take(2)
                .skip(1)
                .order_by(user::age::order(SortOrder::Desc))
                .exec()
                .await
                .unwrap();
            match (&users.len(), &2) {
                (left_val, right_val) => {
                    if !(*left_val == *right_val) {
                        let kind = ::core::panicking::AssertKind::Eq;
                        ::core::panicking::assert_failed(
                            kind,
                            &*left_val,
                            &*right_val,
                            ::core::option::Option::None,
                        );
                    }
                }
            };
            match (&users[0].age, &Some(23)) {
                (left_val, right_val) => {
                    if !(*left_val == *right_val) {
                        let kind = ::core::panicking::AssertKind::Eq;
                        ::core::panicking::assert_failed(
                            kind,
                            &*left_val,
                            &*right_val,
                            ::core::option::Option::None,
                        );
                    }
                }
            };
            match (&users[1].age, &Some(22)) {
                (left_val, right_val) => {
                    if !(*left_val == *right_val) {
                        let kind = ::core::panicking::AssertKind::Eq;
                        ::core::panicking::assert_failed(
                            kind,
                            &*left_val,
                            &*right_val,
                            ::core::option::Option::None,
                        );
                    }
                }
            };
        };
        let mut body = body;
        #[allow(unused_mut)]
        let mut body = unsafe {
            ::tokio::macros::support::Pin::new_unchecked(&mut body)
        };
        let body: ::core::pin::Pin<&mut dyn ::core::future::Future<Output = ()>> = body;
        #[allow(
            clippy::expect_used,
            clippy::diverging_sub_expression,
            clippy::needless_return
        )]
        {
            return tokio::runtime::Builder::new_current_thread()
                .enable_all()
                .build()
                .expect("Failed building the Runtime")
                .block_on(body);
        }
    }
    extern crate test;
    #[rustc_test_marker = "query_builder_tests::test_delete_operations"]
    #[doc(hidden)]
    pub const test_delete_operations: test::TestDescAndFn = test::TestDescAndFn {
        desc: test::TestDesc {
            name: test::StaticTestName("query_builder_tests::test_delete_operations"),
            ignore: false,
            ignore_message: ::core::option::Option::None,
            source_file: "caustics/tests/blog_test.rs",
            start_line: 318usize,
            start_col: 14usize,
            end_line: 318usize,
            end_col: 36usize,
            compile_fail: false,
            no_run: false,
            should_panic: test::ShouldPanic::No,
            test_type: test::TestType::IntegrationTest,
        },
        testfn: test::StaticTestFn(
            #[coverage(off)]
            || test::assert_test_result(test_delete_operations()),
        ),
    };
    fn test_delete_operations() {
        let body = async {
            let db = setup_test_db().await;
            let client = CausticsClient::new(db.clone());
            let email = ::alloc::__export::must_use({
                ::alloc::fmt::format(
                    format_args!("john_{0}@example.com", chrono::Utc::now().timestamp()),
                )
            });
            let user = client
                .user()
                .create(
                    email.clone(),
                    "John".to_string(),
                    DateTime::<FixedOffset>::from_str("2021-01-01T00:00:00Z").unwrap(),
                    DateTime::<FixedOffset>::from_str("2021-01-01T00:00:00Z").unwrap(),
                    <[_]>::into_vec(
                        ::alloc::boxed::box_new([
                            user::age::set(Some(25)),
                            user::deleted_at::set(None),
                        ]),
                    ),
                )
                .exec()
                .await
                .unwrap();
            client.user().delete(user::id::equals(user.id)).exec().await.unwrap();
            let deleted_user = client
                .user()
                .find_unique(user::id::equals(user.id))
                .exec()
                .await
                .unwrap();
            if !deleted_user.is_none() {
                ::core::panicking::panic("assertion failed: deleted_user.is_none()")
            }
        };
        let mut body = body;
        #[allow(unused_mut)]
        let mut body = unsafe {
            ::tokio::macros::support::Pin::new_unchecked(&mut body)
        };
        let body: ::core::pin::Pin<&mut dyn ::core::future::Future<Output = ()>> = body;
        #[allow(
            clippy::expect_used,
            clippy::diverging_sub_expression,
            clippy::needless_return
        )]
        {
            return tokio::runtime::Builder::new_current_thread()
                .enable_all()
                .build()
                .expect("Failed building the Runtime")
                .block_on(body);
        }
    }
    extern crate test;
    #[rustc_test_marker = "query_builder_tests::test_upsert_operations"]
    #[doc(hidden)]
    pub const test_upsert_operations: test::TestDescAndFn = test::TestDescAndFn {
        desc: test::TestDesc {
            name: test::StaticTestName("query_builder_tests::test_upsert_operations"),
            ignore: false,
            ignore_message: ::core::option::Option::None,
            source_file: "caustics/tests/blog_test.rs",
            start_line: 356usize,
            start_col: 14usize,
            end_line: 356usize,
            end_col: 36usize,
            compile_fail: false,
            no_run: false,
            should_panic: test::ShouldPanic::No,
            test_type: test::TestType::IntegrationTest,
        },
        testfn: test::StaticTestFn(
            #[coverage(off)]
            || test::assert_test_result(test_upsert_operations()),
        ),
    };
    fn test_upsert_operations() {
        let body = async {
            let db = setup_test_db().await;
            let client = CausticsClient::new(db.clone());
            let user = client
                .user()
                .upsert(
                    user::email::equals("john@example.com"),
                    user::Create {
                        name: "John".to_string(),
                        email: "john@example.com".to_string(),
                        created_at: DateTime::<
                            FixedOffset,
                        >::from_str("2021-01-01T00:00:00Z")
                            .unwrap(),
                        updated_at: DateTime::<
                            FixedOffset,
                        >::from_str("2021-01-01T00:00:00Z")
                            .unwrap(),
                        _params: ::alloc::vec::Vec::new(),
                    },
                    <[_]>::into_vec(
                        ::alloc::boxed::box_new([
                            user::name::set("John"),
                            user::age::set(25),
                        ]),
                    ),
                )
                .exec()
                .await
                .unwrap();
            match (&user.name, &"John") {
                (left_val, right_val) => {
                    if !(*left_val == *right_val) {
                        let kind = ::core::panicking::AssertKind::Eq;
                        ::core::panicking::assert_failed(
                            kind,
                            &*left_val,
                            &*right_val,
                            ::core::option::Option::None,
                        );
                    }
                }
            };
            match (&user.age, &Some(25)) {
                (left_val, right_val) => {
                    if !(*left_val == *right_val) {
                        let kind = ::core::panicking::AssertKind::Eq;
                        ::core::panicking::assert_failed(
                            kind,
                            &*left_val,
                            &*right_val,
                            ::core::option::Option::None,
                        );
                    }
                }
            };
            let updated_user = client
                .user()
                .upsert(
                    user::email::equals("john@example.com"),
                    user::Create {
                        name: "John".to_string(),
                        email: "john@example.com".to_string(),
                        created_at: DateTime::<
                            FixedOffset,
                        >::from_str("2021-01-01T00:00:00Z")
                            .unwrap(),
                        updated_at: DateTime::<
                            FixedOffset,
                        >::from_str("2021-01-01T00:00:00Z")
                            .unwrap(),
                        _params: ::alloc::vec::Vec::new(),
                    },
                    <[_]>::into_vec(
                        ::alloc::boxed::box_new([
                            user::name::set("John Doe"),
                            user::age::set(26),
                        ]),
                    ),
                )
                .exec()
                .await
                .unwrap();
            match (&updated_user.name, &"John Doe") {
                (left_val, right_val) => {
                    if !(*left_val == *right_val) {
                        let kind = ::core::panicking::AssertKind::Eq;
                        ::core::panicking::assert_failed(
                            kind,
                            &*left_val,
                            &*right_val,
                            ::core::option::Option::None,
                        );
                    }
                }
            };
            match (&updated_user.age, &Some(26)) {
                (left_val, right_val) => {
                    if !(*left_val == *right_val) {
                        let kind = ::core::panicking::AssertKind::Eq;
                        ::core::panicking::assert_failed(
                            kind,
                            &*left_val,
                            &*right_val,
                            ::core::option::Option::None,
                        );
                    }
                }
            };
        };
        let mut body = body;
        #[allow(unused_mut)]
        let mut body = unsafe {
            ::tokio::macros::support::Pin::new_unchecked(&mut body)
        };
        let body: ::core::pin::Pin<&mut dyn ::core::future::Future<Output = ()>> = body;
        #[allow(
            clippy::expect_used,
            clippy::diverging_sub_expression,
            clippy::needless_return
        )]
        {
            return tokio::runtime::Builder::new_current_thread()
                .enable_all()
                .build()
                .expect("Failed building the Runtime")
                .block_on(body);
        }
    }
    extern crate test;
    #[rustc_test_marker = "query_builder_tests::test_transaction_commit"]
    #[doc(hidden)]
    pub const test_transaction_commit: test::TestDescAndFn = test::TestDescAndFn {
        desc: test::TestDesc {
            name: test::StaticTestName("query_builder_tests::test_transaction_commit"),
            ignore: false,
            ignore_message: ::core::option::Option::None,
            source_file: "caustics/tests/blog_test.rs",
            start_line: 402usize,
            start_col: 14usize,
            end_line: 402usize,
            end_col: 37usize,
            compile_fail: false,
            no_run: false,
            should_panic: test::ShouldPanic::No,
            test_type: test::TestType::IntegrationTest,
        },
        testfn: test::StaticTestFn(
            #[coverage(off)]
            || test::assert_test_result(test_transaction_commit()),
        ),
    };
    fn test_transaction_commit() {
        let body = async {
            let db = setup_test_db().await;
            let client = CausticsClient::new(db.clone());
            let email = ::alloc::__export::must_use({
                ::alloc::fmt::format(
                    format_args!("john_{0}@example.com", chrono::Utc::now().timestamp()),
                )
            });
            let email_for_check = email.clone();
            let result = client
                ._transaction()
                .run(|tx| {
                    Box::pin(async move {
                        let user = tx
                            .user()
                            .create(
                                email.clone(),
                                "John".to_string(),
                                DateTime::<FixedOffset>::from_str("2021-01-01T00:00:00Z")
                                    .unwrap(),
                                DateTime::<FixedOffset>::from_str("2021-01-01T00:00:00Z")
                                    .unwrap(),
                                ::alloc::vec::Vec::new(),
                            )
                            .exec()
                            .await?;
                        let post = tx
                            .post()
                            .create(
                                "Hello World".to_string(),
                                DateTime::<FixedOffset>::from_str("2021-01-01T00:00:00Z")
                                    .unwrap(),
                                DateTime::<FixedOffset>::from_str("2021-01-01T00:00:00Z")
                                    .unwrap(),
                                user::id::equals(user.id),
                                <[_]>::into_vec(
                                    ::alloc::boxed::box_new([
                                        post::content::set("This is my first post".to_string()),
                                    ]),
                                ),
                            )
                            .exec()
                            .await?;
                        Ok((user, post))
                    })
                })
                .await
                .expect("Transaction failed");
            match (&result.0.name, &"John") {
                (left_val, right_val) => {
                    if !(*left_val == *right_val) {
                        let kind = ::core::panicking::AssertKind::Eq;
                        ::core::panicking::assert_failed(
                            kind,
                            &*left_val,
                            &*right_val,
                            ::core::option::Option::None,
                        );
                    }
                }
            };
            match (&result.1.title, &"Hello World") {
                (left_val, right_val) => {
                    if !(*left_val == *right_val) {
                        let kind = ::core::panicking::AssertKind::Eq;
                        ::core::panicking::assert_failed(
                            kind,
                            &*left_val,
                            &*right_val,
                            ::core::option::Option::None,
                        );
                    }
                }
            };
            let found_user = client
                .user()
                .find_first(
                    <[_]>::into_vec(
                        ::alloc::boxed::box_new([user::email::equals(&email_for_check)]),
                    ),
                )
                .exec()
                .await
                .expect("Failed to query user")
                .expect("User not found");
            match (&found_user.name, &"John") {
                (left_val, right_val) => {
                    if !(*left_val == *right_val) {
                        let kind = ::core::panicking::AssertKind::Eq;
                        ::core::panicking::assert_failed(
                            kind,
                            &*left_val,
                            &*right_val,
                            ::core::option::Option::None,
                        );
                    }
                }
            };
        };
        let mut body = body;
        #[allow(unused_mut)]
        let mut body = unsafe {
            ::tokio::macros::support::Pin::new_unchecked(&mut body)
        };
        let body: ::core::pin::Pin<&mut dyn ::core::future::Future<Output = ()>> = body;
        #[allow(
            clippy::expect_used,
            clippy::diverging_sub_expression,
            clippy::needless_return
        )]
        {
            return tokio::runtime::Builder::new_current_thread()
                .enable_all()
                .build()
                .expect("Failed building the Runtime")
                .block_on(body);
        }
    }
    extern crate test;
    #[rustc_test_marker = "query_builder_tests::test_transaction_rollback"]
    #[doc(hidden)]
    pub const test_transaction_rollback: test::TestDescAndFn = test::TestDescAndFn {
        desc: test::TestDesc {
            name: test::StaticTestName("query_builder_tests::test_transaction_rollback"),
            ignore: false,
            ignore_message: ::core::option::Option::None,
            source_file: "caustics/tests/blog_test.rs",
            start_line: 459usize,
            start_col: 14usize,
            end_line: 459usize,
            end_col: 39usize,
            compile_fail: false,
            no_run: false,
            should_panic: test::ShouldPanic::No,
            test_type: test::TestType::IntegrationTest,
        },
        testfn: test::StaticTestFn(
            #[coverage(off)]
            || test::assert_test_result(test_transaction_rollback()),
        ),
    };
    fn test_transaction_rollback() {
        let body = async {
            let db = setup_test_db().await;
            let client = CausticsClient::new(db.clone());
            let email = ::alloc::__export::must_use({
                ::alloc::fmt::format(
                    format_args!(
                        "rollback_{0}@example.com",
                        chrono::Utc::now().timestamp(),
                    ),
                )
            });
            let email_for_check = email.clone();
            let result: Result<(), QueryError> = client
                ._transaction()
                .run(|tx| {
                    Box::pin(async move {
                        let _user = tx
                            .user()
                            .create(
                                email.clone(),
                                "Rollback".to_string(),
                                DateTime::<FixedOffset>::from_str("2021-01-01T00:00:00Z")
                                    .unwrap(),
                                DateTime::<FixedOffset>::from_str("2021-01-01T00:00:00Z")
                                    .unwrap(),
                                ::alloc::vec::Vec::new(),
                            )
                            .exec()
                            .await?;
                        Err(QueryError::Custom("Intentional rollback".into()))
                    })
                })
                .await;
            if !result.is_err() {
                ::core::panicking::panic("assertion failed: result.is_err()")
            }
            let found_user = client
                .user()
                .find_first(
                    <[_]>::into_vec(
                        ::alloc::boxed::box_new([user::email::equals(&email_for_check)]),
                    ),
                )
                .exec()
                .await
                .expect("Failed to query user");
            if !found_user.is_none() {
                ::core::panicking::panic("assertion failed: found_user.is_none()")
            }
        };
        let mut body = body;
        #[allow(unused_mut)]
        let mut body = unsafe {
            ::tokio::macros::support::Pin::new_unchecked(&mut body)
        };
        let body: ::core::pin::Pin<&mut dyn ::core::future::Future<Output = ()>> = body;
        #[allow(
            clippy::expect_used,
            clippy::diverging_sub_expression,
            clippy::needless_return
        )]
        {
            return tokio::runtime::Builder::new_current_thread()
                .enable_all()
                .build()
                .expect("Failed building the Runtime")
                .block_on(body);
        }
    }
    extern crate test;
    #[rustc_test_marker = "query_builder_tests::test_relations"]
    #[doc(hidden)]
    pub const test_relations: test::TestDescAndFn = test::TestDescAndFn {
        desc: test::TestDesc {
            name: test::StaticTestName("query_builder_tests::test_relations"),
            ignore: false,
            ignore_message: ::core::option::Option::None,
            source_file: "caustics/tests/blog_test.rs",
            start_line: 501usize,
            start_col: 14usize,
            end_line: 501usize,
            end_col: 28usize,
            compile_fail: false,
            no_run: false,
            should_panic: test::ShouldPanic::No,
            test_type: test::TestType::IntegrationTest,
        },
        testfn: test::StaticTestFn(
            #[coverage(off)]
            || test::assert_test_result(test_relations()),
        ),
    };
    fn test_relations() {
        let body = async {
            let db = setup_test_db().await;
            let client = CausticsClient::new(db.clone());
            let author = client
                .user()
                .create(
                    "john@example.com".to_string(),
                    "John".to_string(),
                    DateTime::<FixedOffset>::from_str("2021-01-01T00:00:00Z").unwrap(),
                    DateTime::<FixedOffset>::from_str("2021-01-01T00:00:00Z").unwrap(),
                    ::alloc::vec::Vec::new(),
                )
                .exec()
                .await
                .unwrap();
            if !author.posts.is_none() {
                ::core::panicking::panic("assertion failed: author.posts.is_none()")
            }
            let reviewer = client
                .user()
                .create(
                    "jane@example.com".to_string(),
                    "Jane".to_string(),
                    DateTime::<FixedOffset>::from_str("2021-01-01T00:00:00Z").unwrap(),
                    DateTime::<FixedOffset>::from_str("2021-01-01T00:00:00Z").unwrap(),
                    ::alloc::vec::Vec::new(),
                )
                .exec()
                .await
                .unwrap();
            let post_with_reviewer = client
                .post()
                .create(
                    "Reviewed Post".to_string(),
                    DateTime::<FixedOffset>::from_str("2021-01-01T00:00:00Z").unwrap(),
                    DateTime::<FixedOffset>::from_str("2021-01-01T00:00:00Z").unwrap(),
                    user::email::equals(author.email),
                    <[_]>::into_vec(
                        ::alloc::boxed::box_new([
                            post::content::set(
                                "This post has been reviewed".to_string(),
                            ),
                            post::reviewer::connect(user::id::equals(reviewer.id)),
                        ]),
                    ),
                )
                .exec()
                .await
                .unwrap();
            let post_without_reviewer = client
                .post()
                .create(
                    "Unreviewed Post".to_string(),
                    DateTime::<FixedOffset>::from_str("2021-01-01T00:00:00Z").unwrap(),
                    DateTime::<FixedOffset>::from_str("2021-01-01T00:00:00Z").unwrap(),
                    user::id::equals(author.id),
                    <[_]>::into_vec(
                        ::alloc::boxed::box_new([
                            post::content::set(
                                "This post hasn't been reviewed yet".to_string(),
                            ),
                        ]),
                    ),
                )
                .exec()
                .await
                .unwrap();
            let user_with_posts = client
                .user()
                .find_unique(user::id::equals(author.id))
                .with(user::posts::fetch(::alloc::vec::Vec::new()))
                .exec()
                .await
                .unwrap()
                .unwrap();
            let posts = user_with_posts.posts.unwrap();
            match (&posts.len(), &2) {
                (left_val, right_val) => {
                    if !(*left_val == *right_val) {
                        let kind = ::core::panicking::AssertKind::Eq;
                        ::core::panicking::assert_failed(
                            kind,
                            &*left_val,
                            &*right_val,
                            ::core::option::Option::None,
                        );
                    }
                }
            };
            match (&posts[0].title, &"Reviewed Post") {
                (left_val, right_val) => {
                    if !(*left_val == *right_val) {
                        let kind = ::core::panicking::AssertKind::Eq;
                        ::core::panicking::assert_failed(
                            kind,
                            &*left_val,
                            &*right_val,
                            ::core::option::Option::None,
                        );
                    }
                }
            };
            match (&posts[1].title, &"Unreviewed Post") {
                (left_val, right_val) => {
                    if !(*left_val == *right_val) {
                        let kind = ::core::panicking::AssertKind::Eq;
                        ::core::panicking::assert_failed(
                            kind,
                            &*left_val,
                            &*right_val,
                            ::core::option::Option::None,
                        );
                    }
                }
            };
            let post_with_reviewer = client
                .post()
                .find_unique(post::id::equals(post_with_reviewer.id))
                .with(post::reviewer::fetch())
                .exec()
                .await
                .unwrap()
                .unwrap();
            let reviewer = post_with_reviewer.reviewer.unwrap().unwrap();
            match (&reviewer.name, &"Jane") {
                (left_val, right_val) => {
                    if !(*left_val == *right_val) {
                        let kind = ::core::panicking::AssertKind::Eq;
                        ::core::panicking::assert_failed(
                            kind,
                            &*left_val,
                            &*right_val,
                            ::core::option::Option::None,
                        );
                    }
                }
            };
            match (&reviewer.email, &"jane@example.com") {
                (left_val, right_val) => {
                    if !(*left_val == *right_val) {
                        let kind = ::core::panicking::AssertKind::Eq;
                        ::core::panicking::assert_failed(
                            kind,
                            &*left_val,
                            &*right_val,
                            ::core::option::Option::None,
                        );
                    }
                }
            };
            let post_without_reviewer = client
                .post()
                .find_unique(post::id::equals(post_without_reviewer.id))
                .with(post::reviewer::fetch())
                .exec()
                .await
                .unwrap()
                .unwrap();
            if !(post_without_reviewer.reviewer.is_none()
                || post_without_reviewer.reviewer.as_ref().unwrap().is_none())
            {
                ::core::panicking::panic(
                    "assertion failed: post_without_reviewer.reviewer.is_none() ||\n    post_without_reviewer.reviewer.as_ref().unwrap().is_none()",
                )
            }
        };
        let mut body = body;
        #[allow(unused_mut)]
        let mut body = unsafe {
            ::tokio::macros::support::Pin::new_unchecked(&mut body)
        };
        let body: ::core::pin::Pin<&mut dyn ::core::future::Future<Output = ()>> = body;
        #[allow(
            clippy::expect_used,
            clippy::diverging_sub_expression,
            clippy::needless_return
        )]
        {
            return tokio::runtime::Builder::new_current_thread()
                .enable_all()
                .build()
                .expect("Failed building the Runtime")
                .block_on(body);
        }
    }
    extern crate test;
    #[rustc_test_marker = "query_builder_tests::test_batch_operations"]
    #[doc(hidden)]
    pub const test_batch_operations: test::TestDescAndFn = test::TestDescAndFn {
        desc: test::TestDesc {
            name: test::StaticTestName("query_builder_tests::test_batch_operations"),
            ignore: false,
            ignore_message: ::core::option::Option::None,
            source_file: "caustics/tests/blog_test.rs",
            start_line: 605usize,
            start_col: 14usize,
            end_line: 605usize,
            end_col: 35usize,
            compile_fail: false,
            no_run: false,
            should_panic: test::ShouldPanic::No,
            test_type: test::TestType::IntegrationTest,
        },
        testfn: test::StaticTestFn(
            #[coverage(off)]
            || test::assert_test_result(test_batch_operations()),
        ),
    };
    fn test_batch_operations() {
        let body = async {
            let db = setup_test_db().await;
            let client = CausticsClient::new(db.clone());
            let timestamp = chrono::Utc::now().timestamp();
            let (user1, user2) = client
                ._batch((
                    client
                        .user()
                        .create(
                            ::alloc::__export::must_use({
                                ::alloc::fmt::format(
                                    format_args!("john_{0}@example.com", timestamp),
                                )
                            }),
                            "John".to_string(),
                            DateTime::<FixedOffset>::from_str("2021-01-01T00:00:00Z")
                                .unwrap(),
                            DateTime::<FixedOffset>::from_str("2021-01-01T00:00:00Z")
                                .unwrap(),
                            <[_]>::into_vec(
                                ::alloc::boxed::box_new([
                                    user::age::set(Some(25)),
                                    user::deleted_at::set(None),
                                ]),
                            ),
                        ),
                    client
                        .user()
                        .create(
                            ::alloc::__export::must_use({
                                ::alloc::fmt::format(
                                    format_args!("jane_{0}@example.com", timestamp),
                                )
                            }),
                            "Jane".to_string(),
                            DateTime::<FixedOffset>::from_str("2021-01-01T00:00:00Z")
                                .unwrap(),
                            DateTime::<FixedOffset>::from_str("2021-01-01T00:00:00Z")
                                .unwrap(),
                            <[_]>::into_vec(
                                ::alloc::boxed::box_new([user::age::set(Some(30))]),
                            ),
                        ),
                ))
                .await
                .expect("Batch operation failed");
            match (&user1.name, &"John") {
                (left_val, right_val) => {
                    if !(*left_val == *right_val) {
                        let kind = ::core::panicking::AssertKind::Eq;
                        ::core::panicking::assert_failed(
                            kind,
                            &*left_val,
                            &*right_val,
                            ::core::option::Option::None,
                        );
                    }
                }
            };
            match (&user2.name, &"Jane") {
                (left_val, right_val) => {
                    if !(*left_val == *right_val) {
                        let kind = ::core::panicking::AssertKind::Eq;
                        ::core::panicking::assert_failed(
                            kind,
                            &*left_val,
                            &*right_val,
                            ::core::option::Option::None,
                        );
                    }
                }
            };
            let found_users = client
                .user()
                .find_many(::alloc::vec::Vec::new())
                .exec()
                .await
                .unwrap();
            match (&found_users.len(), &2) {
                (left_val, right_val) => {
                    if !(*left_val == *right_val) {
                        let kind = ::core::panicking::AssertKind::Eq;
                        ::core::panicking::assert_failed(
                            kind,
                            &*left_val,
                            &*right_val,
                            ::core::option::Option::None,
                        );
                    }
                }
            };
        };
        let mut body = body;
        #[allow(unused_mut)]
        let mut body = unsafe {
            ::tokio::macros::support::Pin::new_unchecked(&mut body)
        };
        let body: ::core::pin::Pin<&mut dyn ::core::future::Future<Output = ()>> = body;
        #[allow(
            clippy::expect_used,
            clippy::diverging_sub_expression,
            clippy::needless_return
        )]
        {
            return tokio::runtime::Builder::new_current_thread()
                .enable_all()
                .build()
                .expect("Failed building the Runtime")
                .block_on(body);
        }
    }
    extern crate test;
    #[rustc_test_marker = "query_builder_tests::test_string_operators"]
    #[doc(hidden)]
    pub const test_string_operators: test::TestDescAndFn = test::TestDescAndFn {
        desc: test::TestDesc {
            name: test::StaticTestName("query_builder_tests::test_string_operators"),
            ignore: false,
            ignore_message: ::core::option::Option::None,
            source_file: "caustics/tests/blog_test.rs",
            start_line: 639usize,
            start_col: 14usize,
            end_line: 639usize,
            end_col: 35usize,
            compile_fail: false,
            no_run: false,
            should_panic: test::ShouldPanic::No,
            test_type: test::TestType::IntegrationTest,
        },
        testfn: test::StaticTestFn(
            #[coverage(off)]
            || test::assert_test_result(test_string_operators()),
        ),
    };
    fn test_string_operators() {
        let body = async {
            use chrono::TimeZone;
            let db = helpers::setup_test_db().await;
            let client = CausticsClient::new(db.clone());
            let now = chrono::FixedOffset::east_opt(0)
                .unwrap()
                .with_ymd_and_hms(2024, 1, 1, 0, 0, 0)
                .unwrap();
            let _user1 = client
                .user()
                .create(
                    "john.doe@example.com".to_string(),
                    "John Doe".to_string(),
                    now,
                    now,
                    <[_]>::into_vec(
                        ::alloc::boxed::box_new([
                            user::age::set(Some(30)),
                            user::deleted_at::set(None),
                        ]),
                    ),
                )
                .exec()
                .await
                .unwrap();
            let _user2 = client
                .user()
                .create(
                    "jane.smith@example.com".to_string(),
                    "Jane Smith".to_string(),
                    now,
                    now,
                    <[_]>::into_vec(
                        ::alloc::boxed::box_new([
                            user::age::set(Some(28)),
                            user::deleted_at::set(None),
                        ]),
                    ),
                )
                .exec()
                .await
                .unwrap();
            let _user3 = client
                .user()
                .create(
                    "bob.johnson@test.org".to_string(),
                    "Bob Johnson".to_string(),
                    now,
                    now,
                    <[_]>::into_vec(
                        ::alloc::boxed::box_new([
                            user::age::set(Some(40)),
                            user::deleted_at::set(None),
                        ]),
                    ),
                )
                .exec()
                .await
                .unwrap();
            let users_with_doe = client
                .user()
                .find_many(
                    <[_]>::into_vec(
                        ::alloc::boxed::box_new([user::name::contains("Doe")]),
                    ),
                )
                .exec()
                .await
                .unwrap();
            match (&users_with_doe.len(), &1) {
                (left_val, right_val) => {
                    if !(*left_val == *right_val) {
                        let kind = ::core::panicking::AssertKind::Eq;
                        ::core::panicking::assert_failed(
                            kind,
                            &*left_val,
                            &*right_val,
                            ::core::option::Option::None,
                        );
                    }
                }
            };
            match (&users_with_doe[0].name, &"John Doe") {
                (left_val, right_val) => {
                    if !(*left_val == *right_val) {
                        let kind = ::core::panicking::AssertKind::Eq;
                        ::core::panicking::assert_failed(
                            kind,
                            &*left_val,
                            &*right_val,
                            ::core::option::Option::None,
                        );
                    }
                }
            };
            let users_starting_with_j = client
                .user()
                .find_many(
                    <[_]>::into_vec(
                        ::alloc::boxed::box_new([user::name::starts_with("J")]),
                    ),
                )
                .exec()
                .await
                .unwrap();
            match (&users_starting_with_j.len(), &2) {
                (left_val, right_val) => {
                    if !(*left_val == *right_val) {
                        let kind = ::core::panicking::AssertKind::Eq;
                        ::core::panicking::assert_failed(
                            kind,
                            &*left_val,
                            &*right_val,
                            ::core::option::Option::None,
                        );
                    }
                }
            };
            if !users_starting_with_j.iter().all(|u| u.name.starts_with("J")) {
                ::core::panicking::panic(
                    "assertion failed: users_starting_with_j.iter().all(|u| u.name.starts_with(\"J\"))",
                )
            }
            let users_ending_with_son = client
                .user()
                .find_many(
                    <[_]>::into_vec(
                        ::alloc::boxed::box_new([user::name::ends_with("son")]),
                    ),
                )
                .exec()
                .await
                .unwrap();
            match (&users_ending_with_son.len(), &1) {
                (left_val, right_val) => {
                    if !(*left_val == *right_val) {
                        let kind = ::core::panicking::AssertKind::Eq;
                        ::core::panicking::assert_failed(
                            kind,
                            &*left_val,
                            &*right_val,
                            ::core::option::Option::None,
                        );
                    }
                }
            };
            match (&users_ending_with_son[0].name, &"Bob Johnson") {
                (left_val, right_val) => {
                    if !(*left_val == *right_val) {
                        let kind = ::core::panicking::AssertKind::Eq;
                        ::core::panicking::assert_failed(
                            kind,
                            &*left_val,
                            &*right_val,
                            ::core::option::Option::None,
                        );
                    }
                }
            };
            let users_with_example_email = client
                .user()
                .find_many(
                    <[_]>::into_vec(
                        ::alloc::boxed::box_new([user::email::contains("example.com")]),
                    ),
                )
                .exec()
                .await
                .unwrap();
            match (&users_with_example_email.len(), &2) {
                (left_val, right_val) => {
                    if !(*left_val == *right_val) {
                        let kind = ::core::panicking::AssertKind::Eq;
                        ::core::panicking::assert_failed(
                            kind,
                            &*left_val,
                            &*right_val,
                            ::core::option::Option::None,
                        );
                    }
                }
            };
            let users_with_test_email = client
                .user()
                .find_many(
                    <[_]>::into_vec(
                        ::alloc::boxed::box_new([user::email::ends_with("test.org")]),
                    ),
                )
                .exec()
                .await
                .unwrap();
            match (&users_with_test_email.len(), &1) {
                (left_val, right_val) => {
                    if !(*left_val == *right_val) {
                        let kind = ::core::panicking::AssertKind::Eq;
                        ::core::panicking::assert_failed(
                            kind,
                            &*left_val,
                            &*right_val,
                            ::core::option::Option::None,
                        );
                    }
                }
            };
            match (&users_with_test_email[0].email, &"bob.johnson@test.org") {
                (left_val, right_val) => {
                    if !(*left_val == *right_val) {
                        let kind = ::core::panicking::AssertKind::Eq;
                        ::core::panicking::assert_failed(
                            kind,
                            &*left_val,
                            &*right_val,
                            ::core::option::Option::None,
                        );
                    }
                }
            };
        };
        let mut body = body;
        #[allow(unused_mut)]
        let mut body = unsafe {
            ::tokio::macros::support::Pin::new_unchecked(&mut body)
        };
        let body: ::core::pin::Pin<&mut dyn ::core::future::Future<Output = ()>> = body;
        #[allow(
            clippy::expect_used,
            clippy::diverging_sub_expression,
            clippy::needless_return
        )]
        {
            return tokio::runtime::Builder::new_current_thread()
                .enable_all()
                .build()
                .expect("Failed building the Runtime")
                .block_on(body);
        }
    }
    extern crate test;
    #[rustc_test_marker = "query_builder_tests::test_comparison_operators"]
    #[doc(hidden)]
    pub const test_comparison_operators: test::TestDescAndFn = test::TestDescAndFn {
        desc: test::TestDesc {
            name: test::StaticTestName("query_builder_tests::test_comparison_operators"),
            ignore: false,
            ignore_message: ::core::option::Option::None,
            source_file: "caustics/tests/blog_test.rs",
            start_line: 715usize,
            start_col: 14usize,
            end_line: 715usize,
            end_col: 39usize,
            compile_fail: false,
            no_run: false,
            should_panic: test::ShouldPanic::No,
            test_type: test::TestType::IntegrationTest,
        },
        testfn: test::StaticTestFn(
            #[coverage(off)]
            || test::assert_test_result(test_comparison_operators()),
        ),
    };
    fn test_comparison_operators() {
        let body = async {
            use chrono::TimeZone;
            let db = helpers::setup_test_db().await;
            let client = CausticsClient::new(db.clone());
            let now = chrono::FixedOffset::east_opt(0)
                .unwrap()
                .with_ymd_and_hms(2024, 1, 1, 0, 0, 0)
                .unwrap();
            let _user1 = client
                .user()
                .create(
                    "john@example.com".to_string(),
                    "John".to_string(),
                    now,
                    now,
                    <[_]>::into_vec(
                        ::alloc::boxed::box_new([
                            user::age::set(Some(25)),
                            user::deleted_at::set(None),
                        ]),
                    ),
                )
                .exec()
                .await
                .unwrap();
            let _user2 = client
                .user()
                .create(
                    "jane@example.com".to_string(),
                    "Jane".to_string(),
                    now,
                    now,
                    <[_]>::into_vec(
                        ::alloc::boxed::box_new([
                            user::age::set(Some(30)),
                            user::deleted_at::set(None),
                        ]),
                    ),
                )
                .exec()
                .await
                .unwrap();
            let _user3 = client
                .user()
                .create(
                    "bob@example.com".to_string(),
                    "Bob".to_string(),
                    now,
                    now,
                    <[_]>::into_vec(
                        ::alloc::boxed::box_new([
                            user::age::set(Some(35)),
                            user::deleted_at::set(None),
                        ]),
                    ),
                )
                .exec()
                .await
                .unwrap();
            let users_older_than_25 = client
                .user()
                .find_many(<[_]>::into_vec(::alloc::boxed::box_new([user::age::gt(25)])))
                .exec()
                .await
                .unwrap();
            match (&users_older_than_25.len(), &2) {
                (left_val, right_val) => {
                    if !(*left_val == *right_val) {
                        let kind = ::core::panicking::AssertKind::Eq;
                        ::core::panicking::assert_failed(
                            kind,
                            &*left_val,
                            &*right_val,
                            ::core::option::Option::None,
                        );
                    }
                }
            };
            if !users_older_than_25.iter().all(|u| u.age.unwrap() > 25) {
                ::core::panicking::panic(
                    "assertion failed: users_older_than_25.iter().all(|u| u.age.unwrap() > 25)",
                )
            }
            let users_30_or_older = client
                .user()
                .find_many(
                    <[_]>::into_vec(::alloc::boxed::box_new([user::age::gte(30)])),
                )
                .exec()
                .await
                .unwrap();
            match (&users_30_or_older.len(), &2) {
                (left_val, right_val) => {
                    if !(*left_val == *right_val) {
                        let kind = ::core::panicking::AssertKind::Eq;
                        ::core::panicking::assert_failed(
                            kind,
                            &*left_val,
                            &*right_val,
                            ::core::option::Option::None,
                        );
                    }
                }
            };
            if !users_30_or_older.iter().all(|u| u.age.unwrap() >= 30) {
                ::core::panicking::panic(
                    "assertion failed: users_30_or_older.iter().all(|u| u.age.unwrap() >= 30)",
                )
            }
            let users_younger_than_35 = client
                .user()
                .find_many(<[_]>::into_vec(::alloc::boxed::box_new([user::age::lt(35)])))
                .exec()
                .await
                .unwrap();
            match (&users_younger_than_35.len(), &2) {
                (left_val, right_val) => {
                    if !(*left_val == *right_val) {
                        let kind = ::core::panicking::AssertKind::Eq;
                        ::core::panicking::assert_failed(
                            kind,
                            &*left_val,
                            &*right_val,
                            ::core::option::Option::None,
                        );
                    }
                }
            };
            if !users_younger_than_35.iter().all(|u| u.age.unwrap() < 35) {
                ::core::panicking::panic(
                    "assertion failed: users_younger_than_35.iter().all(|u| u.age.unwrap() < 35)",
                )
            }
            let users_30_or_younger = client
                .user()
                .find_many(
                    <[_]>::into_vec(::alloc::boxed::box_new([user::age::lte(30)])),
                )
                .exec()
                .await
                .unwrap();
            match (&users_30_or_younger.len(), &2) {
                (left_val, right_val) => {
                    if !(*left_val == *right_val) {
                        let kind = ::core::panicking::AssertKind::Eq;
                        ::core::panicking::assert_failed(
                            kind,
                            &*left_val,
                            &*right_val,
                            ::core::option::Option::None,
                        );
                    }
                }
            };
            if !users_30_or_younger.iter().all(|u| u.age.unwrap() <= 30) {
                ::core::panicking::panic(
                    "assertion failed: users_30_or_younger.iter().all(|u| u.age.unwrap() <= 30)",
                )
            }
            let future_date = now + chrono::Duration::days(1);
            let users_created_before_future = client
                .user()
                .find_many(
                    <[_]>::into_vec(
                        ::alloc::boxed::box_new([user::created_at::lt(future_date)]),
                    ),
                )
                .exec()
                .await
                .unwrap();
            match (&users_created_before_future.len(), &3) {
                (left_val, right_val) => {
                    if !(*left_val == *right_val) {
                        let kind = ::core::panicking::AssertKind::Eq;
                        ::core::panicking::assert_failed(
                            kind,
                            &*left_val,
                            &*right_val,
                            ::core::option::Option::None,
                        );
                    }
                }
            };
            let users_name_after_j = client
                .user()
                .find_many(
                    <[_]>::into_vec(::alloc::boxed::box_new([user::name::gt("J")])),
                )
                .exec()
                .await
                .unwrap();
            match (&users_name_after_j.len(), &2) {
                (left_val, right_val) => {
                    if !(*left_val == *right_val) {
                        let kind = ::core::panicking::AssertKind::Eq;
                        ::core::panicking::assert_failed(
                            kind,
                            &*left_val,
                            &*right_val,
                            ::core::option::Option::None,
                        );
                    }
                }
            };
            let names: Vec<_> = users_name_after_j
                .iter()
                .map(|u| u.name.as_str())
                .collect();
            if !names.contains(&"John") {
                ::core::panicking::panic("assertion failed: names.contains(&\"John\")")
            }
            if !names.contains(&"Jane") {
                ::core::panicking::panic("assertion failed: names.contains(&\"Jane\")")
            }
            let users_age_between_25_and_35 = client
                .user()
                .find_many(
                    <[_]>::into_vec(
                        ::alloc::boxed::box_new([user::age::gte(25), user::age::lte(35)]),
                    ),
                )
                .exec()
                .await
                .unwrap();
            match (&users_age_between_25_and_35.len(), &3) {
                (left_val, right_val) => {
                    if !(*left_val == *right_val) {
                        let kind = ::core::panicking::AssertKind::Eq;
                        ::core::panicking::assert_failed(
                            kind,
                            &*left_val,
                            &*right_val,
                            ::core::option::Option::None,
                        );
                    }
                }
            };
            if !users_age_between_25_and_35
                .iter()
                .all(|u| {
                    let age = u.age.unwrap();
                    age >= 25 && age <= 35
                })
            {
                ::core::panicking::panic(
                    "assertion failed: users_age_between_25_and_35.iter().all(|u|\n        { let age = u.age.unwrap(); age >= 25 && age <= 35 })",
                )
            }
            let deleted_time = now + chrono::Duration::days(2);
            let _user4 = client
                .user()
                .create(
                    "deleted@example.com".to_string(),
                    "Deleted".to_string(),
                    now,
                    now,
                    <[_]>::into_vec(
                        ::alloc::boxed::box_new([
                            user::age::set(Some(40)),
                            user::deleted_at::set(Some(deleted_time)),
                        ]),
                    ),
                )
                .exec()
                .await
                .unwrap();
            let users_deleted_after = client
                .user()
                .find_many(
                    <[_]>::into_vec(
                        ::alloc::boxed::box_new([
                            user::deleted_at::gt(now + chrono::Duration::days(1)),
                        ]),
                    ),
                )
                .exec()
                .await
                .unwrap();
            match (&users_deleted_after.len(), &1) {
                (left_val, right_val) => {
                    if !(*left_val == *right_val) {
                        let kind = ::core::panicking::AssertKind::Eq;
                        ::core::panicking::assert_failed(
                            kind,
                            &*left_val,
                            &*right_val,
                            ::core::option::Option::None,
                        );
                    }
                }
            };
            match (&users_deleted_after[0].email, &"deleted@example.com") {
                (left_val, right_val) => {
                    if !(*left_val == *right_val) {
                        let kind = ::core::panicking::AssertKind::Eq;
                        ::core::panicking::assert_failed(
                            kind,
                            &*left_val,
                            &*right_val,
                            ::core::option::Option::None,
                        );
                    }
                }
            };
            let users_deleted_on_or_before = client
                .user()
                .find_many(
                    <[_]>::into_vec(
                        ::alloc::boxed::box_new([user::deleted_at::lte(deleted_time)]),
                    ),
                )
                .exec()
                .await
                .unwrap();
            match (&users_deleted_on_or_before.len(), &1) {
                (left_val, right_val) => {
                    if !(*left_val == *right_val) {
                        let kind = ::core::panicking::AssertKind::Eq;
                        ::core::panicking::assert_failed(
                            kind,
                            &*left_val,
                            &*right_val,
                            ::core::option::Option::None,
                        );
                    }
                }
            };
            match (&users_deleted_on_or_before[0].email, &"deleted@example.com") {
                (left_val, right_val) => {
                    if !(*left_val == *right_val) {
                        let kind = ::core::panicking::AssertKind::Eq;
                        ::core::panicking::assert_failed(
                            kind,
                            &*left_val,
                            &*right_val,
                            ::core::option::Option::None,
                        );
                    }
                }
            };
            let _post1 = client
                .post()
                .create(
                    "Post 1".to_string(),
                    now,
                    now,
                    user::id::equals(1),
                    <[_]>::into_vec(
                        ::alloc::boxed::box_new([
                            post::content::set(Some("Hello".to_string())),
                        ]),
                    ),
                )
                .exec()
                .await
                .unwrap();
            let _post2 = client
                .post()
                .create(
                    "Post 2".to_string(),
                    now,
                    now,
                    user::id::equals(1),
                    <[_]>::into_vec(
                        ::alloc::boxed::box_new([
                            post::content::set(Some("World".to_string())),
                        ]),
                    ),
                )
                .exec()
                .await
                .unwrap();
            let _post3 = client
                .post()
                .create(
                    "Post 3".to_string(),
                    now,
                    now,
                    user::id::equals(1),
                    <[_]>::into_vec(::alloc::boxed::box_new([post::content::set(None)])),
                )
                .exec()
                .await
                .unwrap();
            let posts_gt_hello = client
                .post()
                .find_many(
                    <[_]>::into_vec(
                        ::alloc::boxed::box_new([
                            post::content::gt(Some("Hello".to_string())),
                        ]),
                    ),
                )
                .exec()
                .await
                .unwrap();
            if !posts_gt_hello.iter().any(|p| p.title == "Post 2") {
                ::core::panicking::panic(
                    "assertion failed: posts_gt_hello.iter().any(|p| p.title == \"Post 2\")",
                )
            }
            let posts_lte_world = client
                .post()
                .find_many(
                    <[_]>::into_vec(
                        ::alloc::boxed::box_new([
                            post::content::lte(Some("World".to_string())),
                        ]),
                    ),
                )
                .exec()
                .await
                .unwrap();
            if !posts_lte_world.iter().any(|p| p.title == "Post 1") {
                ::core::panicking::panic(
                    "assertion failed: posts_lte_world.iter().any(|p| p.title == \"Post 1\")",
                )
            }
            if !posts_lte_world.iter().any(|p| p.title == "Post 2") {
                ::core::panicking::panic(
                    "assertion failed: posts_lte_world.iter().any(|p| p.title == \"Post 2\")",
                )
            }
            let posts_lt_world = client
                .post()
                .find_many(
                    <[_]>::into_vec(
                        ::alloc::boxed::box_new([
                            post::content::lt(Some("World".to_string())),
                        ]),
                    ),
                )
                .exec()
                .await
                .unwrap();
            if !posts_lt_world.iter().any(|p| p.title == "Post 1") {
                ::core::panicking::panic(
                    "assertion failed: posts_lt_world.iter().any(|p| p.title == \"Post 1\")",
                )
            }
            let posts_gte_hello = client
                .post()
                .find_many(
                    <[_]>::into_vec(
                        ::alloc::boxed::box_new([
                            post::content::gte(Some("Hello".to_string())),
                        ]),
                    ),
                )
                .exec()
                .await
                .unwrap();
            if !posts_gte_hello.iter().any(|p| p.title == "Post 1") {
                ::core::panicking::panic(
                    "assertion failed: posts_gte_hello.iter().any(|p| p.title == \"Post 1\")",
                )
            }
            if !posts_gte_hello.iter().any(|p| p.title == "Post 2") {
                ::core::panicking::panic(
                    "assertion failed: posts_gte_hello.iter().any(|p| p.title == \"Post 2\")",
                )
            }
        };
        let mut body = body;
        #[allow(unused_mut)]
        let mut body = unsafe {
            ::tokio::macros::support::Pin::new_unchecked(&mut body)
        };
        let body: ::core::pin::Pin<&mut dyn ::core::future::Future<Output = ()>> = body;
        #[allow(
            clippy::expect_used,
            clippy::diverging_sub_expression,
            clippy::needless_return
        )]
        {
            return tokio::runtime::Builder::new_current_thread()
                .enable_all()
                .build()
                .expect("Failed building the Runtime")
                .block_on(body);
        }
    }
}
#[rustc_main]
#[coverage(off)]
#[doc(hidden)]
pub fn main() -> () {
    extern crate test;
    test::test_main_static(
        &[
            &test_caustics_client,
            &test_batch_operations,
            &test_comparison_operators,
            &test_create_operations,
            &test_delete_operations,
            &test_find_operations,
            &test_pagination_and_sorting,
            &test_relations,
            &test_string_operators,
            &test_transaction_commit,
            &test_transaction_rollback,
            &test_update_operations,
            &test_upsert_operations,
        ],
    )
}
