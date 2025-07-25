   Compiling caustics-macros v0.1.0 (/home/adriano/Sviluppo/caustics/caustics-macros)
   Compiling caustics v0.1.0 (/home/adriano/Sviluppo/caustics/caustics)
warning: unused import: `crate::where_param::generate_where_param_logic`
 --> caustics-macros/src/entity.rs:6:5
  |
6 | use crate::where_param::generate_where_param_logic;
  |     ^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^
  |
  = note: `#[warn(unused_imports)]` on by default
    Finished `dev` profile [unoptimized + debuginfo] target(s) in 0.79s

#![feature(prelude_import)]
#[prelude_import]
use std::prelude::rust_2021::*;
#[macro_use]
extern crate std;
use sea_orm::{DatabaseConnection, DatabaseTransaction, TransactionTrait};
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
impl<C: sea_orm::ConnectionTrait> crate::EntityRegistry<C> for CompositeEntityRegistry {
    fn get_fetcher(&self, entity_name: &str) -> Option<&dyn crate::EntityFetcher<C>> {
        match entity_name {
            _ => None,
        }
    }
}
impl<C: sea_orm::ConnectionTrait> crate::EntityRegistry<C>
for &'static CompositeEntityRegistry {
    fn get_fetcher(&self, entity_name: &str) -> Option<&dyn crate::EntityFetcher<C>> {
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
        ModelWithRelations: crate::FromModel<<Entity as sea_orm::EntityTrait>::Model>,
        T: crate::MergeInto<ActiveModel>,
        <Entity as sea_orm::EntityTrait>::Model: sea_orm::IntoActiveModel<ActiveModel>,
        Container: crate::BatchContainer<
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
                crate::BatchQuery::Insert(q) => {
                    let result = q.exec_in_txn(&txn).await?;
                    crate::BatchResult::Insert(result)
                }
                crate::BatchQuery::Update(q) => {
                    let result = q.exec_in_txn(&txn).await?;
                    crate::BatchResult::Update(result)
                }
                crate::BatchQuery::Delete(q) => {
                    q.exec_in_txn(&txn).await?;
                    crate::BatchResult::Delete(())
                }
                crate::BatchQuery::Upsert(q) => {
                    let result = q.exec_in_txn(&txn).await?;
                    crate::BatchResult::Upsert(result)
                }
            };
            results.push(res);
        }
        txn.commit().await?;
        Ok(Container::from_results(results))
    }
}
#[allow(dead_code)]
impl TransactionCausticsClient {
    pub fn new(tx: std::sync::Arc<DatabaseTransaction>) -> Self {
        Self { tx }
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
pub mod query_builders {
    use sea_orm::{
        ConnectionTrait, EntityTrait, Select, QuerySelect, QueryOrder, IntoActiveModel,
        QueryFilter, DatabaseTransaction,
    };
    use crate::{
        EntityRegistry, RelationFetcher, FromModel, HasRelationMetadata, MergeInto,
        RelationFilter,
    };
    use std::any::Any;
    /// Trait to make DeferredLookup work with both regular connections and transactions
    pub trait ConnectionLike: ConnectionTrait {}
    impl<T: ConnectionTrait> ConnectionLike for T {}
    /// Internal structure for storing deferred foreign key lookups
    pub struct DeferredLookup<C: ConnectionTrait> {
        pub unique_param: Box<dyn Any + Send>,
        pub assign: fn(&mut (dyn Any + 'static), i32),
        pub entity_resolver: Box<
            dyn for<'a> Fn(
                &'a C,
                &dyn Any,
            ) -> std::pin::Pin<
                    Box<
                        dyn std::future::Future<
                            Output = Result<i32, sea_orm::DbErr>,
                        > + Send + 'a,
                    >,
                > + Send,
        >,
    }
    impl<C: ConnectionTrait> DeferredLookup<C> {
        pub fn new(
            unique_param: Box<dyn Any + Send>,
            assign: fn(&mut (dyn Any + 'static), i32),
            entity_resolver: impl for<'a> Fn(
                &'a C,
                &dyn Any,
            ) -> std::pin::Pin<
                    Box<
                        dyn std::future::Future<
                            Output = Result<i32, sea_orm::DbErr>,
                        > + Send + 'a,
                    >,
                > + Send + 'static,
        ) -> Self {
            Self {
                unique_param,
                assign,
                entity_resolver: Box::new(entity_resolver),
            }
        }
    }
    /// Query builder for finding a unique entity record
    pub struct UniqueQueryBuilder<
        'a,
        C: ConnectionTrait,
        Entity: EntityTrait,
        ModelWithRelations,
    > {
        pub query: Select<Entity>,
        pub conn: &'a C,
        pub relations_to_fetch: Vec<RelationFilter>,
        pub registry: &'a dyn EntityRegistry<C>,
        pub _phantom: std::marker::PhantomData<ModelWithRelations>,
    }
    impl<
        'a,
        C: ConnectionTrait,
        Entity: EntityTrait,
        ModelWithRelations,
    > UniqueQueryBuilder<'a, C, Entity, ModelWithRelations>
    where
        ModelWithRelations: FromModel<Entity::Model>
            + HasRelationMetadata<ModelWithRelations> + Send + 'static,
    {
        /// Execute the query and return a single result
        pub async fn exec(self) -> Result<Option<ModelWithRelations>, sea_orm::DbErr> {
            if self.relations_to_fetch.is_empty() {
                self.query
                    .one(self.conn)
                    .await
                    .map(|opt| opt.map(|model| ModelWithRelations::from_model(model)))
            } else {
                self.exec_with_relations().await
            }
        }
        /// Add a relation to fetch with the query
        pub fn with<T: Into<RelationFilter>>(mut self, relation: T) -> Self {
            self.relations_to_fetch.push(relation.into());
            self
        }
        /// Execute query with relations
        async fn exec_with_relations(
            self,
        ) -> Result<Option<ModelWithRelations>, sea_orm::DbErr>
        where
            ModelWithRelations: FromModel<Entity::Model>,
        {
            let Self { query, conn, relations_to_fetch, registry, .. } = self;
            let main_result = query.one(conn).await?;
            if let Some(main_model) = main_result {
                let mut model_with_relations = ModelWithRelations::from_model(
                    main_model,
                );
                for relation_filter in relations_to_fetch {
                    Self::fetch_relation_for_model(
                            conn,
                            &mut model_with_relations,
                            relation_filter.relation,
                            &relation_filter.filters,
                            registry,
                        )
                        .await?;
                }
                Ok(Some(model_with_relations))
            } else {
                Ok(None)
            }
        }
        /// Fetch a single relation for a model
        async fn fetch_relation_for_model(
            conn: &C,
            model_with_relations: &mut ModelWithRelations,
            relation_name: &str,
            _filters: &[crate::types::Filter],
            registry: &dyn EntityRegistry<C>,
        ) -> Result<(), sea_orm::DbErr> {
            let relation_name_snake = heck::ToSnakeCase::to_snake_case(relation_name);
            let descriptor = ModelWithRelations::get_relation_descriptor(
                    &relation_name_snake,
                )
                .ok_or_else(|| sea_orm::DbErr::Custom(
                    ::alloc::__export::must_use({
                        ::alloc::fmt::format(
                            format_args!("Relation \'{0}\' not found", relation_name),
                        )
                    }),
                ))?;
            let type_name = std::any::type_name::<ModelWithRelations>();
            let fetcher_entity_name = type_name
                .rsplit("::")
                .nth(1)
                .unwrap_or("")
                .to_lowercase();
            let fetcher = registry
                .get_fetcher(&fetcher_entity_name)
                .ok_or_else(|| sea_orm::DbErr::Custom(
                    ::alloc::__export::must_use({
                        ::alloc::fmt::format(
                            format_args!(
                                "No fetcher found for entity: {0}",
                                fetcher_entity_name,
                            ),
                        )
                    }),
                ))?;
            let fetched_result = fetcher
                .fetch_by_foreign_key(
                    conn,
                    (descriptor.get_foreign_key)(model_with_relations),
                    descriptor.foreign_key_column,
                    &fetcher_entity_name,
                    relation_name,
                )
                .await?;
            (descriptor.set_field)(model_with_relations, fetched_result);
            Ok(())
        }
    }
    /// Query builder for finding the first entity record matching conditions
    pub struct FirstQueryBuilder<
        'a,
        C: ConnectionTrait,
        Entity: EntityTrait,
        ModelWithRelations,
    > {
        pub query: Select<Entity>,
        pub conn: &'a C,
        pub relations_to_fetch: Vec<RelationFilter>,
        pub registry: &'a dyn EntityRegistry<C>,
        pub _phantom: std::marker::PhantomData<ModelWithRelations>,
    }
    impl<
        'a,
        C: ConnectionTrait,
        Entity: EntityTrait,
        ModelWithRelations,
    > FirstQueryBuilder<'a, C, Entity, ModelWithRelations>
    where
        ModelWithRelations: FromModel<Entity::Model>
            + HasRelationMetadata<ModelWithRelations> + Send + 'static,
    {
        /// Execute the query and return a single result
        pub async fn exec(self) -> Result<Option<ModelWithRelations>, sea_orm::DbErr> {
            if self.relations_to_fetch.is_empty() {
                self.query
                    .one(self.conn)
                    .await
                    .map(|opt| opt.map(|model| ModelWithRelations::from_model(model)))
            } else {
                self.exec_with_relations().await
            }
        }
        /// Add a relation to fetch with the query
        pub fn with<T: Into<RelationFilter>>(mut self, relation: T) -> Self {
            self.relations_to_fetch.push(relation.into());
            self
        }
        /// Execute query with relations
        async fn exec_with_relations(
            self,
        ) -> Result<Option<ModelWithRelations>, sea_orm::DbErr>
        where
            ModelWithRelations: FromModel<Entity::Model>,
        {
            let Self { query, conn, relations_to_fetch, registry, .. } = self;
            let main_result = query.one(conn).await?;
            if let Some(main_model) = main_result {
                let mut model_with_relations = ModelWithRelations::from_model(
                    main_model,
                );
                for relation_filter in relations_to_fetch {
                    Self::fetch_relation_for_model(
                            conn,
                            &mut model_with_relations,
                            relation_filter.relation,
                            &relation_filter.filters,
                            registry,
                        )
                        .await?;
                }
                Ok(Some(model_with_relations))
            } else {
                Ok(None)
            }
        }
        /// Fetch a single relation for a model
        async fn fetch_relation_for_model(
            conn: &C,
            model_with_relations: &mut ModelWithRelations,
            relation_name: &str,
            _filters: &[crate::types::Filter],
            registry: &dyn EntityRegistry<C>,
        ) -> Result<(), sea_orm::DbErr> {
            let descriptor = ModelWithRelations::get_relation_descriptor(relation_name)
                .ok_or_else(|| sea_orm::DbErr::Custom(
                    ::alloc::__export::must_use({
                        ::alloc::fmt::format(
                            format_args!("Relation \'{0}\' not found", relation_name),
                        )
                    }),
                ))?;
            let foreign_key_value = (descriptor.get_foreign_key)(model_with_relations);
            let extracted_entity_name = extract_entity_name_from_path(
                &descriptor.target_entity,
            );
            let extracted_entity_name = extracted_entity_name.clone();
            let foreign_key_column = descriptor.foreign_key_column;
            let is_has_many = foreign_key_column == "id";
            let fetcher_entity_name = if is_has_many {
                let type_name = std::any::type_name::<ModelWithRelations>();
                type_name.rsplit("::").nth(1).unwrap_or("").to_lowercase()
            } else {
                extracted_entity_name.clone()
            };
            let fetcher = registry
                .get_fetcher(&fetcher_entity_name)
                .ok_or_else(|| sea_orm::DbErr::Custom(
                    ::alloc::__export::must_use({
                        ::alloc::fmt::format(
                            format_args!(
                                "No fetcher found for entity: {0}",
                                fetcher_entity_name,
                            ),
                        )
                    }),
                ))?;
            let fetched_result = fetcher
                .fetch_by_foreign_key(
                    conn,
                    foreign_key_value,
                    foreign_key_column,
                    &fetcher_entity_name,
                    relation_name,
                )
                .await?;
            (descriptor.set_field)(model_with_relations, fetched_result);
            Ok(())
        }
    }
    /// Query builder for finding multiple entity records matching conditions
    pub struct ManyQueryBuilder<
        'a,
        C: ConnectionTrait,
        Entity: EntityTrait,
        ModelWithRelations,
    > {
        pub query: Select<Entity>,
        pub conn: &'a C,
        pub relations_to_fetch: Vec<RelationFilter>,
        pub registry: &'a dyn EntityRegistry<C>,
        pub _phantom: std::marker::PhantomData<ModelWithRelations>,
    }
    impl<
        'a,
        C: ConnectionTrait,
        Entity: EntityTrait,
        ModelWithRelations,
    > ManyQueryBuilder<'a, C, Entity, ModelWithRelations>
    where
        ModelWithRelations: FromModel<Entity::Model>
            + HasRelationMetadata<ModelWithRelations> + Send + 'static,
    {
        /// Limit the number of results
        pub fn take(mut self, limit: u64) -> Self {
            self.query = self.query.limit(limit);
            self
        }
        /// Skip a number of results (for pagination)
        pub fn skip(mut self, offset: u64) -> Self {
            self.query = self.query.offset(offset);
            self
        }
        /// Order the results by a column
        pub fn order_by<Col>(
            mut self,
            col_and_order: impl Into<(Col, sea_orm::Order)>,
        ) -> Self
        where
            Col: sea_orm::ColumnTrait + sea_orm::IntoSimpleExpr,
        {
            let (col, order) = col_and_order.into();
            self.query = self.query.order_by(col, order);
            self
        }
        /// Execute the query and return multiple results
        pub async fn exec(self) -> Result<Vec<ModelWithRelations>, sea_orm::DbErr>
        where
            ModelWithRelations: FromModel<Entity::Model>,
        {
            if self.relations_to_fetch.is_empty() {
                self.query
                    .all(self.conn)
                    .await
                    .map(|models| {
                        models
                            .into_iter()
                            .map(|model| ModelWithRelations::from_model(model))
                            .collect()
                    })
            } else {
                self.exec_with_relations().await
            }
        }
        /// Add a relation to fetch with the query
        pub fn with<T: Into<RelationFilter>>(mut self, relation: T) -> Self {
            self.relations_to_fetch.push(relation.into());
            self
        }
        /// Execute query with relations
        async fn exec_with_relations(
            self,
        ) -> Result<Vec<ModelWithRelations>, sea_orm::DbErr>
        where
            ModelWithRelations: FromModel<Entity::Model>,
        {
            let Self { query, conn, relations_to_fetch, registry, .. } = self;
            let main_results = query.all(conn).await?;
            let mut models_with_relations = Vec::new();
            for main_model in main_results {
                let mut model_with_relations = ModelWithRelations::from_model(
                    main_model,
                );
                for relation_filter in &relations_to_fetch {
                    Self::fetch_relation_for_model(
                            conn,
                            &mut model_with_relations,
                            relation_filter.relation,
                            &relation_filter.filters,
                            registry,
                        )
                        .await?;
                }
                models_with_relations.push(model_with_relations);
            }
            Ok(models_with_relations)
        }
        /// Fetch a single relation for a model
        async fn fetch_relation_for_model(
            conn: &C,
            model_with_relations: &mut ModelWithRelations,
            relation_name: &str,
            _filters: &[crate::types::Filter],
            registry: &dyn EntityRegistry<C>,
        ) -> Result<(), sea_orm::DbErr> {
            let descriptor = ModelWithRelations::get_relation_descriptor(relation_name)
                .ok_or_else(|| sea_orm::DbErr::Custom(
                    ::alloc::__export::must_use({
                        ::alloc::fmt::format(
                            format_args!("Relation \'{0}\' not found", relation_name),
                        )
                    }),
                ))?;
            let foreign_key_value = (descriptor.get_foreign_key)(model_with_relations);
            let extracted_entity_name = extract_entity_name_from_path(
                &descriptor.target_entity,
            );
            let extracted_entity_name = extracted_entity_name.clone();
            let foreign_key_column = descriptor.foreign_key_column;
            let is_has_many = foreign_key_column == "id";
            let fetcher_entity_name = if is_has_many {
                let type_name = std::any::type_name::<ModelWithRelations>();
                type_name.rsplit("::").nth(1).unwrap_or("").to_lowercase()
            } else {
                extracted_entity_name.clone()
            };
            let fetcher = registry
                .get_fetcher(&fetcher_entity_name)
                .ok_or_else(|| sea_orm::DbErr::Custom(
                    ::alloc::__export::must_use({
                        ::alloc::fmt::format(
                            format_args!(
                                "No fetcher found for entity: {0}",
                                fetcher_entity_name,
                            ),
                        )
                    }),
                ))?;
            let fetched_result = fetcher
                .fetch_by_foreign_key(
                    conn,
                    foreign_key_value,
                    foreign_key_column,
                    &fetcher_entity_name,
                    relation_name,
                )
                .await?;
            (descriptor.set_field)(model_with_relations, fetched_result);
            Ok(())
        }
    }
    /// Query builder for creating a new entity record
    pub struct CreateQueryBuilder<
        'a,
        C: ConnectionTrait,
        Entity: EntityTrait,
        ActiveModel: sea_orm::ActiveModelTrait<Entity = Entity>
            + sea_orm::ActiveModelBehavior + Send + 'static,
        ModelWithRelations,
    > {
        pub model: ActiveModel,
        pub conn: &'a C,
        pub deferred_lookups: Vec<DeferredLookup<C>>,
        pub _phantom: std::marker::PhantomData<(Entity, ModelWithRelations)>,
    }
    impl<
        'a,
        C,
        Entity,
        ActiveModel,
        ModelWithRelations,
    > CreateQueryBuilder<'a, C, Entity, ActiveModel, ModelWithRelations>
    where
        C: ConnectionTrait,
        Entity: EntityTrait,
        ActiveModel: sea_orm::ActiveModelTrait<Entity = Entity>
            + sea_orm::ActiveModelBehavior + Send + 'static,
        ModelWithRelations: FromModel<<Entity as EntityTrait>::Model>,
    {
        pub async fn exec(self) -> Result<ModelWithRelations, sea_orm::DbErr>
        where
            <Entity as EntityTrait>::Model: sea_orm::IntoActiveModel<ActiveModel>,
        {
            let mut model = self.model;
            for lookup in &self.deferred_lookups {
                let lookup_result = (lookup
                    .entity_resolver)(self.conn, &*lookup.unique_param)
                    .await?;
                (lookup.assign)(&mut model as &mut (dyn Any + 'static), lookup_result);
            }
            model.insert(self.conn).await.map(ModelWithRelations::from_model)
        }
        /// Execute the query within a transaction
        pub async fn exec_in_txn(
            self,
            txn: &DatabaseTransaction,
        ) -> Result<ModelWithRelations, sea_orm::DbErr>
        where
            <Entity as EntityTrait>::Model: sea_orm::IntoActiveModel<ActiveModel>,
        {
            let mut model = self.model;
            for lookup in &self.deferred_lookups {
                let conn_ref = unsafe {
                    std::mem::transmute::<&DatabaseTransaction, &C>(txn)
                };
                let lookup_result = (lookup
                    .entity_resolver)(conn_ref, &*lookup.unique_param)
                    .await?;
                (lookup.assign)(&mut model as &mut (dyn Any + 'static), lookup_result);
            }
            model.insert(txn).await.map(ModelWithRelations::from_model)
        }
    }
    /// Query builder for deleting entity records matching a condition
    pub struct DeleteQueryBuilder<'a, C: ConnectionTrait, Entity: EntityTrait> {
        pub condition: sea_orm::Condition,
        pub conn: &'a C,
        pub _phantom: std::marker::PhantomData<Entity>,
    }
    impl<'a, C, Entity> DeleteQueryBuilder<'a, C, Entity>
    where
        C: ConnectionTrait,
        Entity: EntityTrait,
    {
        pub async fn exec(self) -> Result<(), sea_orm::DbErr> {
            Entity::delete_many()
                .filter::<sea_orm::Condition>(self.condition)
                .exec(self.conn)
                .await?;
            Ok(())
        }
        /// Execute the query within a transaction
        pub async fn exec_in_txn(
            self,
            txn: &DatabaseTransaction,
        ) -> Result<(), sea_orm::DbErr> {
            Entity::delete_many()
                .filter::<sea_orm::Condition>(self.condition)
                .exec(txn)
                .await?;
            Ok(())
        }
    }
    /// Query builder for upserting (insert or update) entity records
    pub struct UpsertQueryBuilder<
        'a,
        C: ConnectionTrait,
        Entity: EntityTrait,
        ActiveModel: sea_orm::ActiveModelTrait<Entity = Entity>
            + sea_orm::ActiveModelBehavior + Send + 'static,
        ModelWithRelations,
        T: MergeInto<ActiveModel>,
    > {
        pub condition: sea_orm::Condition,
        pub create: (ActiveModel, Vec<DeferredLookup<C>>),
        pub update: Vec<T>,
        pub conn: &'a C,
        pub _phantom: std::marker::PhantomData<(Entity, ModelWithRelations)>,
    }
    impl<
        'a,
        C,
        Entity,
        ActiveModel,
        ModelWithRelations,
        T,
    > UpsertQueryBuilder<'a, C, Entity, ActiveModel, ModelWithRelations, T>
    where
        C: ConnectionTrait,
        Entity: EntityTrait,
        ActiveModel: sea_orm::ActiveModelTrait<Entity = Entity>
            + sea_orm::ActiveModelBehavior + Send + 'static,
        ModelWithRelations: FromModel<<Entity as EntityTrait>::Model>,
        T: MergeInto<ActiveModel>,
        <Entity as EntityTrait>::Model: sea_orm::IntoActiveModel<ActiveModel>,
    {
        pub async fn exec(self) -> Result<ModelWithRelations, sea_orm::DbErr> {
            let existing = Entity::find()
                .filter::<sea_orm::Condition>(self.condition.clone())
                .one(self.conn)
                .await?;
            match existing {
                Some(active_model) => {
                    let mut active_model = active_model.into_active_model();
                    for change in self.update {
                        change.merge_into(&mut active_model);
                    }
                    active_model
                        .update(self.conn)
                        .await
                        .map(ModelWithRelations::from_model)
                }
                None => {
                    let (mut active_model, deferred_lookups) = self.create;
                    for lookup in &deferred_lookups {
                        let lookup_result = (lookup
                            .entity_resolver)(self.conn, &*lookup.unique_param)
                            .await?;
                        (lookup
                            .assign)(
                            &mut active_model as &mut (dyn Any + 'static),
                            lookup_result,
                        );
                    }
                    for change in self.update {
                        change.merge_into(&mut active_model);
                    }
                    active_model
                        .insert(self.conn)
                        .await
                        .map(ModelWithRelations::from_model)
                }
            }
        }
        /// Execute the query within a transaction
        pub async fn exec_in_txn(
            self,
            txn: &DatabaseTransaction,
        ) -> Result<ModelWithRelations, sea_orm::DbErr> {
            let existing = Entity::find()
                .filter::<sea_orm::Condition>(self.condition.clone())
                .one(txn)
                .await?;
            match existing {
                Some(active_model) => {
                    let mut active_model = active_model.into_active_model();
                    for change in self.update {
                        change.merge_into(&mut active_model);
                    }
                    active_model.update(txn).await.map(ModelWithRelations::from_model)
                }
                None => {
                    let (mut active_model, deferred_lookups) = self.create;
                    for lookup in &deferred_lookups {
                        let conn_ref = unsafe {
                            std::mem::transmute::<&DatabaseTransaction, &C>(txn)
                        };
                        let lookup_result = (lookup
                            .entity_resolver)(conn_ref, &*lookup.unique_param)
                            .await?;
                        (lookup
                            .assign)(
                            &mut active_model as &mut (dyn Any + 'static),
                            lookup_result,
                        );
                    }
                    for change in self.update {
                        change.merge_into(&mut active_model);
                    }
                    active_model.insert(txn).await.map(ModelWithRelations::from_model)
                }
            }
        }
    }
    /// Query builder for updating entity records
    pub struct UpdateQueryBuilder<
        'a,
        C: ConnectionTrait,
        Entity: EntityTrait,
        ActiveModel: sea_orm::ActiveModelTrait<Entity = Entity>
            + sea_orm::ActiveModelBehavior + Send,
        ModelWithRelations,
        T: MergeInto<ActiveModel>,
    > {
        pub condition: sea_orm::Condition,
        pub changes: Vec<T>,
        pub conn: &'a C,
        pub _phantom: std::marker::PhantomData<
            (Entity, ActiveModel, ModelWithRelations),
        >,
    }
    impl<
        'a,
        C,
        Entity,
        ActiveModel,
        ModelWithRelations,
        T,
    > UpdateQueryBuilder<'a, C, Entity, ActiveModel, ModelWithRelations, T>
    where
        C: ConnectionTrait,
        Entity: EntityTrait,
        ActiveModel: sea_orm::ActiveModelTrait<Entity = Entity>
            + sea_orm::ActiveModelBehavior + Send,
        ModelWithRelations: FromModel<<Entity as EntityTrait>::Model>,
        T: MergeInto<ActiveModel>,
        <Entity as EntityTrait>::Model: sea_orm::IntoActiveModel<ActiveModel>,
    {
        pub async fn exec(self) -> Result<ModelWithRelations, sea_orm::DbErr> {
            let entity = <Entity as EntityTrait>::find()
                .filter::<sea_orm::Condition>(self.condition)
                .one(self.conn)
                .await?;
            if let Some(entity) = entity {
                let mut active_model = entity.into_active_model();
                for change in self.changes {
                    change.merge_into(&mut active_model);
                }
                active_model.update(self.conn).await.map(ModelWithRelations::from_model)
            } else {
                Err(
                    sea_orm::DbErr::RecordNotFound(
                        "No record found to update".to_string(),
                    ),
                )
            }
        }
        /// Execute the query within a transaction
        pub async fn exec_in_txn(
            self,
            txn: &DatabaseTransaction,
        ) -> Result<ModelWithRelations, sea_orm::DbErr> {
            let entity = <Entity as EntityTrait>::find()
                .filter::<sea_orm::Condition>(self.condition)
                .one(txn)
                .await?;
            if let Some(entity) = entity {
                let mut active_model = entity.into_active_model();
                for change in self.changes {
                    change.merge_into(&mut active_model);
                }
                active_model.update(txn).await.map(ModelWithRelations::from_model)
            } else {
                Err(
                    sea_orm::DbErr::RecordNotFound(
                        "No record found to update".to_string(),
                    ),
                )
            }
        }
    }
    /// SeaORM-specific relation fetcher implementation
    pub struct SeaOrmRelationFetcher<R: EntityRegistry<C>, C: ConnectionTrait> {
        pub entity_registry: R,
        pub _phantom: std::marker::PhantomData<C>,
    }
    impl<
        C: ConnectionTrait,
        ModelWithRelations,
        R: EntityRegistry<C>,
    > RelationFetcher<C, ModelWithRelations> for SeaOrmRelationFetcher<R, C>
    where
        ModelWithRelations: HasRelationMetadata<ModelWithRelations> + 'static,
        R: Send + Sync,
        C: Send + Sync,
    {
        fn fetch_relation_for_model(
            &self,
            conn: &C,
            model_with_relations: &mut ModelWithRelations,
            relation_name: &str,
            _filters: &[crate::types::Filter],
        ) -> Result<(), sea_orm::DbErr> {
            let descriptor = ModelWithRelations::get_relation_descriptor(relation_name)
                .ok_or_else(|| sea_orm::DbErr::Custom(
                    ::alloc::__export::must_use({
                        ::alloc::fmt::format(
                            format_args!("Relation \'{0}\' not found", relation_name),
                        )
                    }),
                ))?;
            let type_name = std::any::type_name::<ModelWithRelations>();
            let fetcher_entity_name = type_name
                .rsplit("::")
                .nth(1)
                .unwrap_or("")
                .to_lowercase();
            if let Some(fetcher) = self.entity_registry.get_fetcher(&fetcher_entity_name)
            {
                let rt = tokio::runtime::Handle::current();
                let result = rt
                    .block_on(async {
                        fetcher
                            .fetch_by_foreign_key(
                                conn,
                                (descriptor.get_foreign_key)(model_with_relations),
                                descriptor.foreign_key_column,
                                &fetcher_entity_name,
                                relation_name,
                            )
                            .await
                    })?;
                (descriptor.set_field)(model_with_relations, result);
                Ok(())
            } else {
                Err(
                    sea_orm::DbErr::Custom(
                        ::alloc::__export::must_use({
                            ::alloc::fmt::format(
                                format_args!(
                                    "No fetcher found for entity: {0}",
                                    fetcher_entity_name,
                                ),
                            )
                        }),
                    ),
                )
            }
        }
    }
    /// Batch query types that can be executed in a transaction
    pub enum BatchQuery<
        'a,
        C: ConnectionTrait,
        Entity: EntityTrait,
        ActiveModel: sea_orm::ActiveModelTrait<Entity = Entity>
            + sea_orm::ActiveModelBehavior + Send + 'static,
        ModelWithRelations,
        T: MergeInto<ActiveModel>,
    > {
        Insert(CreateQueryBuilder<'a, C, Entity, ActiveModel, ModelWithRelations>),
        Update(UpdateQueryBuilder<'a, C, Entity, ActiveModel, ModelWithRelations, T>),
        Delete(DeleteQueryBuilder<'a, C, Entity>),
        Upsert(UpsertQueryBuilder<'a, C, Entity, ActiveModel, ModelWithRelations, T>),
    }
    /// Result types for batch operations
    pub enum BatchResult<ModelWithRelations> {
        Insert(ModelWithRelations),
        Update(ModelWithRelations),
        Delete(()),
        Upsert(ModelWithRelations),
    }
    /// Extract entity name from a path string representation
    fn extract_entity_name_from_path(path_str: &str) -> String {
        let mut last_entity_name = "unknown".to_string();
        let mut pos = 0;
        while let Some(start) = path_str[pos..].find("ident: \"") {
            let full_start = pos + start + 8;
            if let Some(end) = path_str[full_start..].find("\"") {
                let entity_name = path_str[full_start..full_start + end].to_string();
                last_entity_name = entity_name;
                pos = full_start + end + 1;
            } else {
                break;
            }
        }
        last_entity_name
    }
}
pub mod types {
    use std::any::Any;
    pub type QueryError = sea_orm::DbErr;
    use crate::query_builders::{BatchQuery, BatchResult};
    pub enum SortOrder {
        Asc,
        Desc,
    }
    #[automatically_derived]
    impl ::core::marker::Copy for SortOrder {}
    #[automatically_derived]
    impl ::core::clone::Clone for SortOrder {
        #[inline]
        fn clone(&self) -> SortOrder {
            *self
        }
    }
    #[automatically_derived]
    impl ::core::fmt::Debug for SortOrder {
        #[inline]
        fn fmt(&self, f: &mut ::core::fmt::Formatter) -> ::core::fmt::Result {
            ::core::fmt::Formatter::write_str(
                f,
                match self {
                    SortOrder::Asc => "Asc",
                    SortOrder::Desc => "Desc",
                },
            )
        }
    }
    pub enum QueryMode {
        Default,
        Insensitive,
    }
    #[automatically_derived]
    impl ::core::marker::Copy for QueryMode {}
    #[automatically_derived]
    impl ::core::clone::Clone for QueryMode {
        #[inline]
        fn clone(&self) -> QueryMode {
            *self
        }
    }
    #[automatically_derived]
    impl ::core::fmt::Debug for QueryMode {
        #[inline]
        fn fmt(&self, f: &mut ::core::fmt::Formatter) -> ::core::fmt::Result {
            ::core::fmt::Formatter::write_str(
                f,
                match self {
                    QueryMode::Default => "Default",
                    QueryMode::Insensitive => "Insensitive",
                },
            )
        }
    }
    #[automatically_derived]
    impl ::core::marker::StructuralPartialEq for QueryMode {}
    #[automatically_derived]
    impl ::core::cmp::PartialEq for QueryMode {
        #[inline]
        fn eq(&self, other: &QueryMode) -> bool {
            let __self_discr = ::core::intrinsics::discriminant_value(self);
            let __arg1_discr = ::core::intrinsics::discriminant_value(other);
            __self_discr == __arg1_discr
        }
    }
    #[automatically_derived]
    impl ::core::cmp::Eq for QueryMode {
        #[inline]
        #[doc(hidden)]
        #[coverage(off)]
        fn assert_receiver_is_total_eq(&self) -> () {}
    }
    /// Trait for converting a model to a model with relations
    pub trait FromModel<M> {
        fn from_model(model: M) -> Self;
    }
    /// Trait for merging values into an ActiveModel
    pub trait MergeInto<AM> {
        fn merge_into(&self, model: &mut AM);
    }
    impl<AM> MergeInto<AM> for () {
        fn merge_into(&self, _model: &mut AM) {}
    }
    /// Trait for relation filters that can be used with .with()
    pub trait RelationFilterTrait: Clone {
        fn relation_name(&self) -> &'static str;
        fn filters(&self) -> &[Filter];
    }
    /// Generic filter structure that matches the generated Filter type
    pub struct Filter {
        pub field: String,
        pub value: String,
    }
    #[automatically_derived]
    impl ::core::fmt::Debug for Filter {
        #[inline]
        fn fmt(&self, f: &mut ::core::fmt::Formatter) -> ::core::fmt::Result {
            ::core::fmt::Formatter::debug_struct_field2_finish(
                f,
                "Filter",
                "field",
                &self.field,
                "value",
                &&self.value,
            )
        }
    }
    #[automatically_derived]
    impl ::core::clone::Clone for Filter {
        #[inline]
        fn clone(&self) -> Filter {
            Filter {
                field: ::core::clone::Clone::clone(&self.field),
                value: ::core::clone::Clone::clone(&self.value),
            }
        }
    }
    /// Generic relation filter structure that matches the generated RelationFilter type
    pub struct RelationFilter {
        pub relation: &'static str,
        pub filters: Vec<Filter>,
    }
    #[automatically_derived]
    impl ::core::fmt::Debug for RelationFilter {
        #[inline]
        fn fmt(&self, f: &mut ::core::fmt::Formatter) -> ::core::fmt::Result {
            ::core::fmt::Formatter::debug_struct_field2_finish(
                f,
                "RelationFilter",
                "relation",
                &self.relation,
                "filters",
                &&self.filters,
            )
        }
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
    impl RelationFilterTrait for RelationFilter {
        fn relation_name(&self) -> &'static str {
            self.relation
        }
        fn filters(&self) -> &[Filter] {
            &self.filters
        }
    }
    /// Trait for dynamic relation fetching
    pub trait RelationFetcher<C: sea_orm::ConnectionTrait, ModelWithRelations> {
        fn fetch_relation_for_model(
            &self,
            conn: &C,
            model_with_relations: &mut ModelWithRelations,
            relation_name: &str,
            filters: &[Filter],
        ) -> Result<(), sea_orm::DbErr>;
    }
    impl<
        C: sea_orm::ConnectionTrait,
        ModelWithRelations,
    > RelationFetcher<C, ModelWithRelations> for () {
        fn fetch_relation_for_model(
            &self,
            _conn: &C,
            _model_with_relations: &mut ModelWithRelations,
            _relation_name: &str,
            _filters: &[Filter],
        ) -> Result<(), sea_orm::DbErr> {
            Ok(())
        }
    }
    /// Descriptor for a relation, used for dynamic lookup
    pub struct RelationDescriptor<ModelWithRelations> {
        pub name: &'static str,
        pub set_field: fn(&mut ModelWithRelations, Box<dyn Any + Send>),
        pub get_foreign_key: fn(&ModelWithRelations) -> Option<i32>,
        pub target_entity: &'static str,
        pub foreign_key_column: &'static str,
    }
    /// Trait for types that provide relation metadata
    pub trait HasRelationMetadata<ModelWithRelations> {
        fn relation_descriptors() -> &'static [RelationDescriptor<ModelWithRelations>];
        fn get_relation_descriptor(
            name: &str,
        ) -> Option<&'static RelationDescriptor<ModelWithRelations>> {
            Self::relation_descriptors().iter().find(|desc| desc.name == name)
        }
    }
    /// Trait for dynamic entity fetching without hardcoding
    pub trait EntityFetcher<C: sea_orm::ConnectionTrait> {
        /// Fetch entities by foreign key value
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
                    Output = Result<Box<dyn Any + Send>, sea_orm::DbErr>,
                > + Send + 'a,
            >,
        >;
    }
    /// Registry for mapping entity names to their fetchers
    pub trait EntityRegistry<C: sea_orm::ConnectionTrait> {
        /// Get the fetcher for a given entity name
        fn get_fetcher(&self, entity_name: &str) -> Option<&dyn EntityFetcher<C>>;
    }
    /// Helper type for dynamic entity resolution
    pub struct EntityResolver<C: sea_orm::ConnectionTrait> {
        pub registry: Box<dyn EntityRegistry<C> + Send + Sync>,
    }
    impl<C: sea_orm::ConnectionTrait> EntityResolver<C> {
        pub fn new(registry: Box<dyn EntityRegistry<C> + Send + Sync>) -> Self {
            Self { registry }
        }
        pub async fn resolve_entity(
            &self,
            conn: &C,
            foreign_key_value: Option<i32>,
            foreign_key_column: &str,
            target_entity: &str,
            relation_name: &str,
        ) -> Result<Box<dyn Any + Send>, sea_orm::DbErr> {
            if let Some(fetcher) = self.registry.get_fetcher(target_entity) {
                fetcher
                    .fetch_by_foreign_key(
                        conn,
                        foreign_key_value,
                        foreign_key_column,
                        target_entity,
                        relation_name,
                    )
                    .await
            } else {
                Err(
                    sea_orm::DbErr::Custom(
                        ::alloc::__export::must_use({
                            ::alloc::fmt::format(
                                format_args!(
                                    "No fetcher found for entity: {0}",
                                    target_entity,
                                ),
                            )
                        }),
                    ),
                )
            }
        }
    }
    /// Trait for batch containers that can hold multiple queries (like Prisma Client Rust)
    pub trait BatchContainer<'a, C, Entity, ActiveModel, ModelWithRelations, T>
    where
        C: sea_orm::ConnectionTrait,
        Entity: sea_orm::EntityTrait,
        ActiveModel: sea_orm::ActiveModelTrait<Entity = Entity>
            + sea_orm::ActiveModelBehavior + Send + 'static,
        ModelWithRelations: FromModel<<Entity as sea_orm::EntityTrait>::Model>,
        T: MergeInto<ActiveModel>,
    {
        type ReturnType;
        fn into_queries(
            self,
        ) -> Vec<BatchQuery<'a, C, Entity, ActiveModel, ModelWithRelations, T>>;
        fn from_results(
            results: Vec<BatchResult<ModelWithRelations>>,
        ) -> Self::ReturnType;
    }
    /// Helper function to create batch queries
    pub async fn batch<'a, C, Entity, ActiveModel, ModelWithRelations, T, Container>(
        queries: Container,
        conn: &'a C,
    ) -> Result<Container::ReturnType, sea_orm::DbErr>
    where
        C: sea_orm::ConnectionTrait + sea_orm::TransactionTrait,
        Entity: sea_orm::EntityTrait,
        ActiveModel: sea_orm::ActiveModelTrait<Entity = Entity>
            + sea_orm::ActiveModelBehavior + Send + 'static,
        ModelWithRelations: FromModel<<Entity as sea_orm::EntityTrait>::Model>,
        T: MergeInto<ActiveModel>,
        Container: BatchContainer<'a, C, Entity, ActiveModel, ModelWithRelations, T>,
        <Entity as sea_orm::EntityTrait>::Model: sea_orm::IntoActiveModel<ActiveModel>,
    {
        let txn = conn.begin().await?;
        let batch_queries = Container::into_queries(queries);
        let mut results = Vec::with_capacity(batch_queries.len());
        for query in batch_queries {
            let res = match query {
                BatchQuery::Insert(q) => {
                    let model = q.model;
                    let result = model.insert(&txn).await.map(FromModel::from_model)?;
                    BatchResult::Insert(result)
                }
                BatchQuery::Update(_) => {
                    return Err(
                        sea_orm::DbErr::Custom(
                            "Update operations not supported in batch mode".to_string(),
                        ),
                    );
                }
                BatchQuery::Delete(_) => {
                    return Err(
                        sea_orm::DbErr::Custom(
                            "Delete operations not supported in batch mode".to_string(),
                        ),
                    );
                }
                BatchQuery::Upsert(_) => {
                    return Err(
                        sea_orm::DbErr::Custom(
                            "Upsert operations not supported in batch mode".to_string(),
                        ),
                    );
                }
            };
            results.push(res);
        }
        txn.commit().await?;
        Ok(Container::from_results(results))
    }
    impl<
        'a,
        C,
        Entity,
        ActiveModel,
        ModelWithRelations,
        T,
    > BatchContainer<'a, C, Entity, ActiveModel, ModelWithRelations, T>
    for Vec<BatchQuery<'a, C, Entity, ActiveModel, ModelWithRelations, T>>
    where
        C: sea_orm::ConnectionTrait,
        Entity: sea_orm::EntityTrait,
        ActiveModel: sea_orm::ActiveModelTrait<Entity = Entity>
            + sea_orm::ActiveModelBehavior + Send + 'static,
        ModelWithRelations: FromModel<<Entity as sea_orm::EntityTrait>::Model>,
        T: MergeInto<ActiveModel>,
    {
        type ReturnType = Vec<BatchResult<ModelWithRelations>>;
        fn into_queries(
            self,
        ) -> Vec<BatchQuery<'a, C, Entity, ActiveModel, ModelWithRelations, T>> {
            self
        }
        fn from_results(
            results: Vec<BatchResult<ModelWithRelations>>,
        ) -> Self::ReturnType {
            results
        }
    }
    impl<
        'a,
        Entity,
        ActiveModel,
        ModelWithRelations,
    > BatchContainer<
        'a,
        sea_orm::DatabaseConnection,
        Entity,
        ActiveModel,
        ModelWithRelations,
        (),
    >
    for (
        crate::query_builders::CreateQueryBuilder<
            'a,
            sea_orm::DatabaseConnection,
            Entity,
            ActiveModel,
            ModelWithRelations,
        >,
    )
    where
        Entity: sea_orm::EntityTrait,
        ActiveModel: sea_orm::ActiveModelTrait<Entity = Entity>
            + sea_orm::ActiveModelBehavior + Send + 'static,
        ModelWithRelations: FromModel<<Entity as sea_orm::EntityTrait>::Model>,
        (): MergeInto<ActiveModel>,
    {
        type ReturnType = (ModelWithRelations,);
        fn into_queries(
            self,
        ) -> Vec<
            BatchQuery<
                'a,
                sea_orm::DatabaseConnection,
                Entity,
                ActiveModel,
                ModelWithRelations,
                (),
            >,
        > {
            <[_]>::into_vec(::alloc::boxed::box_new([BatchQuery::Insert(self.0)]))
        }
        fn from_results(
            mut results: Vec<BatchResult<ModelWithRelations>>,
        ) -> Self::ReturnType {
            let result1 = match results.remove(0) {
                BatchResult::Insert(model) => model,
                _ => {
                    ::core::panicking::panic_fmt(format_args!("Expected Insert result"));
                }
            };
            (result1,)
        }
    }
    impl<
        'a,
        Entity,
        ActiveModel,
        ModelWithRelations,
    > BatchContainer<
        'a,
        sea_orm::DatabaseConnection,
        Entity,
        ActiveModel,
        ModelWithRelations,
        (),
    >
    for (
        crate::query_builders::CreateQueryBuilder<
            'a,
            sea_orm::DatabaseConnection,
            Entity,
            ActiveModel,
            ModelWithRelations,
        >,
        crate::query_builders::CreateQueryBuilder<
            'a,
            sea_orm::DatabaseConnection,
            Entity,
            ActiveModel,
            ModelWithRelations,
        >,
    )
    where
        Entity: sea_orm::EntityTrait,
        ActiveModel: sea_orm::ActiveModelTrait<Entity = Entity>
            + sea_orm::ActiveModelBehavior + Send + 'static,
        ModelWithRelations: FromModel<<Entity as sea_orm::EntityTrait>::Model>,
        (): MergeInto<ActiveModel>,
    {
        type ReturnType = (ModelWithRelations, ModelWithRelations);
        fn into_queries(
            self,
        ) -> Vec<
            BatchQuery<
                'a,
                sea_orm::DatabaseConnection,
                Entity,
                ActiveModel,
                ModelWithRelations,
                (),
            >,
        > {
            <[_]>::into_vec(
                ::alloc::boxed::box_new([
                    BatchQuery::Insert(self.0),
                    BatchQuery::Insert(self.1),
                ]),
            )
        }
        fn from_results(
            mut results: Vec<BatchResult<ModelWithRelations>>,
        ) -> Self::ReturnType {
            let result1 = match results.remove(0) {
                BatchResult::Insert(model) => model,
                _ => {
                    ::core::panicking::panic_fmt(format_args!("Expected Insert result"));
                }
            };
            let result2 = match results.remove(0) {
                BatchResult::Insert(model) => model,
                _ => {
                    ::core::panicking::panic_fmt(format_args!("Expected Insert result"));
                }
            };
            (result1, result2)
        }
    }
    impl<
        'a,
        Entity,
        ActiveModel,
        ModelWithRelations,
    > BatchContainer<
        'a,
        sea_orm::DatabaseConnection,
        Entity,
        ActiveModel,
        ModelWithRelations,
        (),
    >
    for (
        crate::query_builders::CreateQueryBuilder<
            'a,
            sea_orm::DatabaseConnection,
            Entity,
            ActiveModel,
            ModelWithRelations,
        >,
        crate::query_builders::CreateQueryBuilder<
            'a,
            sea_orm::DatabaseConnection,
            Entity,
            ActiveModel,
            ModelWithRelations,
        >,
        crate::query_builders::CreateQueryBuilder<
            'a,
            sea_orm::DatabaseConnection,
            Entity,
            ActiveModel,
            ModelWithRelations,
        >,
    )
    where
        Entity: sea_orm::EntityTrait,
        ActiveModel: sea_orm::ActiveModelTrait<Entity = Entity>
            + sea_orm::ActiveModelBehavior + Send + 'static,
        ModelWithRelations: FromModel<<Entity as sea_orm::EntityTrait>::Model>,
        (): MergeInto<ActiveModel>,
    {
        type ReturnType = (ModelWithRelations, ModelWithRelations, ModelWithRelations);
        fn into_queries(
            self,
        ) -> Vec<
            BatchQuery<
                'a,
                sea_orm::DatabaseConnection,
                Entity,
                ActiveModel,
                ModelWithRelations,
                (),
            >,
        > {
            <[_]>::into_vec(
                ::alloc::boxed::box_new([
                    BatchQuery::Insert(self.0),
                    BatchQuery::Insert(self.1),
                    BatchQuery::Insert(self.2),
                ]),
            )
        }
        fn from_results(
            mut results: Vec<BatchResult<ModelWithRelations>>,
        ) -> Self::ReturnType {
            let result1 = match results.remove(0) {
                BatchResult::Insert(model) => model,
                _ => {
                    ::core::panicking::panic_fmt(format_args!("Expected Insert result"));
                }
            };
            let result2 = match results.remove(0) {
                BatchResult::Insert(model) => model,
                _ => {
                    ::core::panicking::panic_fmt(format_args!("Expected Insert result"));
                }
            };
            let result3 = match results.remove(0) {
                BatchResult::Insert(model) => model,
                _ => {
                    ::core::panicking::panic_fmt(format_args!("Expected Insert result"));
                }
            };
            (result1, result2, result3)
        }
    }
    impl<
        'a,
        Entity,
        ActiveModel,
        ModelWithRelations,
    > BatchContainer<
        'a,
        sea_orm::DatabaseConnection,
        Entity,
        ActiveModel,
        ModelWithRelations,
        (),
    >
    for (
        crate::query_builders::CreateQueryBuilder<
            'a,
            sea_orm::DatabaseConnection,
            Entity,
            ActiveModel,
            ModelWithRelations,
        >,
        crate::query_builders::CreateQueryBuilder<
            'a,
            sea_orm::DatabaseConnection,
            Entity,
            ActiveModel,
            ModelWithRelations,
        >,
        crate::query_builders::CreateQueryBuilder<
            'a,
            sea_orm::DatabaseConnection,
            Entity,
            ActiveModel,
            ModelWithRelations,
        >,
        crate::query_builders::CreateQueryBuilder<
            'a,
            sea_orm::DatabaseConnection,
            Entity,
            ActiveModel,
            ModelWithRelations,
        >,
    )
    where
        Entity: sea_orm::EntityTrait,
        ActiveModel: sea_orm::ActiveModelTrait<Entity = Entity>
            + sea_orm::ActiveModelBehavior + Send + 'static,
        ModelWithRelations: FromModel<<Entity as sea_orm::EntityTrait>::Model>,
        (): MergeInto<ActiveModel>,
    {
        type ReturnType = (
            ModelWithRelations,
            ModelWithRelations,
            ModelWithRelations,
            ModelWithRelations,
        );
        fn into_queries(
            self,
        ) -> Vec<
            BatchQuery<
                'a,
                sea_orm::DatabaseConnection,
                Entity,
                ActiveModel,
                ModelWithRelations,
                (),
            >,
        > {
            <[_]>::into_vec(
                ::alloc::boxed::box_new([
                    BatchQuery::Insert(self.0),
                    BatchQuery::Insert(self.1),
                    BatchQuery::Insert(self.2),
                    BatchQuery::Insert(self.3),
                ]),
            )
        }
        fn from_results(
            mut results: Vec<BatchResult<ModelWithRelations>>,
        ) -> Self::ReturnType {
            let result1 = match results.remove(0) {
                BatchResult::Insert(model) => model,
                _ => {
                    ::core::panicking::panic_fmt(format_args!("Expected Insert result"));
                }
            };
            let result2 = match results.remove(0) {
                BatchResult::Insert(model) => model,
                _ => {
                    ::core::panicking::panic_fmt(format_args!("Expected Insert result"));
                }
            };
            let result3 = match results.remove(0) {
                BatchResult::Insert(model) => model,
                _ => {
                    ::core::panicking::panic_fmt(format_args!("Expected Insert result"));
                }
            };
            let result4 = match results.remove(0) {
                BatchResult::Insert(model) => model,
                _ => {
                    ::core::panicking::panic_fmt(format_args!("Expected Insert result"));
                }
            };
            (result1, result2, result3, result4)
        }
    }
    /// String filter operations for use in where clauses (Caustics equivalent of Prisma's StringFilter)
    pub enum StringFilter {
        Equals(String),
        In(Vec<String>),
        NotIn(Vec<String>),
        Lt(String),
        Lte(String),
        Gt(String),
        Gte(String),
        Contains(String),
        StartsWith(String),
        EndsWith(String),
        Mode(QueryMode),
        Not(Box<StringFilter>),
    }
    #[automatically_derived]
    impl ::core::fmt::Debug for StringFilter {
        #[inline]
        fn fmt(&self, f: &mut ::core::fmt::Formatter) -> ::core::fmt::Result {
            match self {
                StringFilter::Equals(__self_0) => {
                    ::core::fmt::Formatter::debug_tuple_field1_finish(
                        f,
                        "Equals",
                        &__self_0,
                    )
                }
                StringFilter::In(__self_0) => {
                    ::core::fmt::Formatter::debug_tuple_field1_finish(f, "In", &__self_0)
                }
                StringFilter::NotIn(__self_0) => {
                    ::core::fmt::Formatter::debug_tuple_field1_finish(
                        f,
                        "NotIn",
                        &__self_0,
                    )
                }
                StringFilter::Lt(__self_0) => {
                    ::core::fmt::Formatter::debug_tuple_field1_finish(f, "Lt", &__self_0)
                }
                StringFilter::Lte(__self_0) => {
                    ::core::fmt::Formatter::debug_tuple_field1_finish(
                        f,
                        "Lte",
                        &__self_0,
                    )
                }
                StringFilter::Gt(__self_0) => {
                    ::core::fmt::Formatter::debug_tuple_field1_finish(f, "Gt", &__self_0)
                }
                StringFilter::Gte(__self_0) => {
                    ::core::fmt::Formatter::debug_tuple_field1_finish(
                        f,
                        "Gte",
                        &__self_0,
                    )
                }
                StringFilter::Contains(__self_0) => {
                    ::core::fmt::Formatter::debug_tuple_field1_finish(
                        f,
                        "Contains",
                        &__self_0,
                    )
                }
                StringFilter::StartsWith(__self_0) => {
                    ::core::fmt::Formatter::debug_tuple_field1_finish(
                        f,
                        "StartsWith",
                        &__self_0,
                    )
                }
                StringFilter::EndsWith(__self_0) => {
                    ::core::fmt::Formatter::debug_tuple_field1_finish(
                        f,
                        "EndsWith",
                        &__self_0,
                    )
                }
                StringFilter::Mode(__self_0) => {
                    ::core::fmt::Formatter::debug_tuple_field1_finish(
                        f,
                        "Mode",
                        &__self_0,
                    )
                }
                StringFilter::Not(__self_0) => {
                    ::core::fmt::Formatter::debug_tuple_field1_finish(
                        f,
                        "Not",
                        &__self_0,
                    )
                }
            }
        }
    }
    #[automatically_derived]
    impl ::core::clone::Clone for StringFilter {
        #[inline]
        fn clone(&self) -> StringFilter {
            match self {
                StringFilter::Equals(__self_0) => {
                    StringFilter::Equals(::core::clone::Clone::clone(__self_0))
                }
                StringFilter::In(__self_0) => {
                    StringFilter::In(::core::clone::Clone::clone(__self_0))
                }
                StringFilter::NotIn(__self_0) => {
                    StringFilter::NotIn(::core::clone::Clone::clone(__self_0))
                }
                StringFilter::Lt(__self_0) => {
                    StringFilter::Lt(::core::clone::Clone::clone(__self_0))
                }
                StringFilter::Lte(__self_0) => {
                    StringFilter::Lte(::core::clone::Clone::clone(__self_0))
                }
                StringFilter::Gt(__self_0) => {
                    StringFilter::Gt(::core::clone::Clone::clone(__self_0))
                }
                StringFilter::Gte(__self_0) => {
                    StringFilter::Gte(::core::clone::Clone::clone(__self_0))
                }
                StringFilter::Contains(__self_0) => {
                    StringFilter::Contains(::core::clone::Clone::clone(__self_0))
                }
                StringFilter::StartsWith(__self_0) => {
                    StringFilter::StartsWith(::core::clone::Clone::clone(__self_0))
                }
                StringFilter::EndsWith(__self_0) => {
                    StringFilter::EndsWith(::core::clone::Clone::clone(__self_0))
                }
                StringFilter::Mode(__self_0) => {
                    StringFilter::Mode(::core::clone::Clone::clone(__self_0))
                }
                StringFilter::Not(__self_0) => {
                    StringFilter::Not(::core::clone::Clone::clone(__self_0))
                }
            }
        }
    }
    #[automatically_derived]
    impl ::core::marker::StructuralPartialEq for StringFilter {}
    #[automatically_derived]
    impl ::core::cmp::PartialEq for StringFilter {
        #[inline]
        fn eq(&self, other: &StringFilter) -> bool {
            let __self_discr = ::core::intrinsics::discriminant_value(self);
            let __arg1_discr = ::core::intrinsics::discriminant_value(other);
            __self_discr == __arg1_discr
                && match (self, other) {
                    (StringFilter::Equals(__self_0), StringFilter::Equals(__arg1_0)) => {
                        __self_0 == __arg1_0
                    }
                    (StringFilter::In(__self_0), StringFilter::In(__arg1_0)) => {
                        __self_0 == __arg1_0
                    }
                    (StringFilter::NotIn(__self_0), StringFilter::NotIn(__arg1_0)) => {
                        __self_0 == __arg1_0
                    }
                    (StringFilter::Lt(__self_0), StringFilter::Lt(__arg1_0)) => {
                        __self_0 == __arg1_0
                    }
                    (StringFilter::Lte(__self_0), StringFilter::Lte(__arg1_0)) => {
                        __self_0 == __arg1_0
                    }
                    (StringFilter::Gt(__self_0), StringFilter::Gt(__arg1_0)) => {
                        __self_0 == __arg1_0
                    }
                    (StringFilter::Gte(__self_0), StringFilter::Gte(__arg1_0)) => {
                        __self_0 == __arg1_0
                    }
                    (
                        StringFilter::Contains(__self_0),
                        StringFilter::Contains(__arg1_0),
                    ) => __self_0 == __arg1_0,
                    (
                        StringFilter::StartsWith(__self_0),
                        StringFilter::StartsWith(__arg1_0),
                    ) => __self_0 == __arg1_0,
                    (
                        StringFilter::EndsWith(__self_0),
                        StringFilter::EndsWith(__arg1_0),
                    ) => __self_0 == __arg1_0,
                    (StringFilter::Mode(__self_0), StringFilter::Mode(__arg1_0)) => {
                        __self_0 == __arg1_0
                    }
                    (StringFilter::Not(__self_0), StringFilter::Not(__arg1_0)) => {
                        __self_0 == __arg1_0
                    }
                    _ => unsafe { ::core::intrinsics::unreachable() }
                }
        }
    }
    /// String write operations for use in set/update clauses (Caustics equivalent of Prisma's StringParam)
    pub enum StringParam {
        Set(String),
    }
    #[automatically_derived]
    impl ::core::fmt::Debug for StringParam {
        #[inline]
        fn fmt(&self, f: &mut ::core::fmt::Formatter) -> ::core::fmt::Result {
            match self {
                StringParam::Set(__self_0) => {
                    ::core::fmt::Formatter::debug_tuple_field1_finish(
                        f,
                        "Set",
                        &__self_0,
                    )
                }
            }
        }
    }
    #[automatically_derived]
    impl ::core::clone::Clone for StringParam {
        #[inline]
        fn clone(&self) -> StringParam {
            match self {
                StringParam::Set(__self_0) => {
                    StringParam::Set(::core::clone::Clone::clone(__self_0))
                }
            }
        }
    }
    #[automatically_derived]
    impl ::core::marker::StructuralPartialEq for StringParam {}
    #[automatically_derived]
    impl ::core::cmp::PartialEq for StringParam {
        #[inline]
        fn eq(&self, other: &StringParam) -> bool {
            match (self, other) {
                (StringParam::Set(__self_0), StringParam::Set(__arg1_0)) => {
                    __self_0 == __arg1_0
                }
            }
        }
    }
}
pub use query_builders::*;
pub use types::*;
pub use query_builders::DeferredLookup;
pub use types::{EntityRegistry, EntityFetcher};
