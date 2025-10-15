use super::deferred_lookup::DeferredLookup;
use crate::{FromModel, PostInsertOp, RelationFilter, ApplyNestedIncludes, HasRelationMetadata, EntityRegistry};
use sea_orm::{ConnectionTrait, DatabaseConnection, DatabaseTransaction, EntityTrait};
use std::any::Any;

/// Query builder for creating a new entity record
pub struct CreateQueryBuilder<
    'a,
    C: ConnectionTrait,
    Entity: EntityTrait,
    ActiveModel: sea_orm::ActiveModelTrait<Entity = Entity> + sea_orm::ActiveModelBehavior + Send + 'static,
    ModelWithRelations,
> {
    pub model: ActiveModel,
    pub conn: &'a C,
    pub deferred_lookups: Vec<DeferredLookup>,
    pub post_insert_ops: Vec<PostInsertOp<'a>>,
    pub id_extractor: fn(&<Entity as EntityTrait>::Model) -> crate::CausticsKey,
    pub relations_to_fetch: Vec<RelationFilter>,
    pub registry: &'a (dyn EntityRegistry<C> + Sync),
    pub _phantom: std::marker::PhantomData<(Entity, ModelWithRelations)>,
}

impl<'a, C, Entity, ActiveModel, ModelWithRelations>
    CreateQueryBuilder<'a, C, Entity, ActiveModel, ModelWithRelations>
where
    C: ConnectionTrait,
    Entity: EntityTrait,
    ActiveModel:
        sea_orm::ActiveModelTrait<Entity = Entity> + sea_orm::ActiveModelBehavior + Send + 'static,
    ModelWithRelations: FromModel<<Entity as EntityTrait>::Model>,
{
    /// Add a relation to fetch with the query
    pub fn with<T: Into<RelationFilter>>(mut self, relation: T) -> Self {
        self.relations_to_fetch.push(relation.into());
        self
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

        // Execute all deferred lookups in batch using the transaction
        for lookup in &self.deferred_lookups {
            let lookup_result = (lookup.resolve_on_txn)(txn, &*lookup.unique_param).await?;
            (lookup.assign)(&mut model as &mut (dyn Any + 'static), lookup_result);
        }

        let inserted = model.insert(txn).await?;
        let parent_id = (self.id_extractor)(&inserted);
        for op in self.post_insert_ops {
            (op.run_on_txn)(txn, parent_id.clone()).await?;
        }

        let model_with_relations = ModelWithRelations::from_model(inserted);

        // Note: Relation fetching in exec_in_txn is not yet supported
        // due to type constraints between EntityRegistry<C> and DatabaseTransaction
        // Relations will need to be fetched separately after the transaction

        Ok(model_with_relations)
    }
}

impl<'a, Entity, ActiveModel, ModelWithRelations>
    CreateQueryBuilder<'a, DatabaseConnection, Entity, ActiveModel, ModelWithRelations>
where
    Entity: EntityTrait,
    ActiveModel:
        sea_orm::ActiveModelTrait<Entity = Entity> + sea_orm::ActiveModelBehavior + Send + 'static,
    ModelWithRelations: FromModel<<Entity as EntityTrait>::Model>
        + HasRelationMetadata<ModelWithRelations>
        + ApplyNestedIncludes<DatabaseConnection>
        + Send
        + 'static,
{
    pub async fn exec(self) -> Result<ModelWithRelations, sea_orm::DbErr>
    where
        <Entity as EntityTrait>::Model: sea_orm::IntoActiveModel<ActiveModel>,
    {
        if self.relations_to_fetch.is_empty() {
            let mut model = self.model;

            // Execute all deferred lookups in batch
            for lookup in &self.deferred_lookups {
                let lookup_result = (lookup.resolve_on_conn)(self.conn, &*lookup.unique_param).await?;
                (lookup.assign)(&mut model as &mut (dyn Any + 'static), lookup_result);
            }

            let inserted = model.insert(self.conn).await?;
            let parent_id = (self.id_extractor)(&inserted);
            for op in self.post_insert_ops {
                (op.run_on_conn)(self.conn, parent_id.clone()).await?;
            }

            let model_with_relations = ModelWithRelations::from_model(inserted);
            Ok(model_with_relations)
        } else {
            self.exec_with_relations().await
        }
    }

    async fn exec_with_relations(self) -> Result<ModelWithRelations, sea_orm::DbErr>
    where
        <Entity as EntityTrait>::Model: sea_orm::IntoActiveModel<ActiveModel>,
        ModelWithRelations: FromModel<<Entity as EntityTrait>::Model>
            + HasRelationMetadata<ModelWithRelations>
            + ApplyNestedIncludes<DatabaseConnection>,
    {
        let Self {
            mut model,
            conn,
            deferred_lookups,
            post_insert_ops,
            id_extractor,
            relations_to_fetch,
            registry,
            ..
        } = self;

        for lookup in &deferred_lookups {
            let lookup_result = (lookup.resolve_on_conn)(conn, &*lookup.unique_param).await?;
            (lookup.assign)(&mut model as &mut (dyn Any + 'static), lookup_result);
        }

        let inserted = model.insert(conn).await?;
        let parent_id = (id_extractor)(&inserted);
        for op in post_insert_ops {
            (op.run_on_conn)(conn, parent_id.clone()).await?;
        }

        let mut model_with_relations = ModelWithRelations::from_model(inserted);

        for relation_filter in relations_to_fetch {
            ApplyNestedIncludes::apply_relation_filter(
                &mut model_with_relations,
                conn,
                &relation_filter,
                registry,
            )
            .await?;
        }

        Ok(model_with_relations)
    }
}

impl<'a, Entity, ActiveModel, ModelWithRelations>
    CreateQueryBuilder<'a, DatabaseTransaction, Entity, ActiveModel, ModelWithRelations>
where
    Entity: EntityTrait,
    ActiveModel:
        sea_orm::ActiveModelTrait<Entity = Entity> + sea_orm::ActiveModelBehavior + Send + 'static,
    ModelWithRelations: FromModel<<Entity as EntityTrait>::Model>
        + HasRelationMetadata<ModelWithRelations>
        + ApplyNestedIncludes<DatabaseTransaction>
        + Send
        + 'static,
{
    pub async fn exec(self) -> Result<ModelWithRelations, sea_orm::DbErr>
    where
        <Entity as EntityTrait>::Model: sea_orm::IntoActiveModel<ActiveModel>,
    {
        if self.relations_to_fetch.is_empty() {
            let mut model = self.model;

            for lookup in &self.deferred_lookups {
                let lookup_result = (lookup.resolve_on_txn)(self.conn, &*lookup.unique_param).await?;
                (lookup.assign)(&mut model as &mut (dyn Any + 'static), lookup_result);
            }

            let inserted = model.insert(self.conn).await?;
            let parent_id = (self.id_extractor)(&inserted);
            for op in self.post_insert_ops {
                (op.run_on_txn)(self.conn, parent_id.clone()).await?;
            }

            let model_with_relations = ModelWithRelations::from_model(inserted);
            Ok(model_with_relations)
        } else {
            self.exec_with_relations().await
        }
    }

    async fn exec_with_relations(self) -> Result<ModelWithRelations, sea_orm::DbErr>
    where
        <Entity as EntityTrait>::Model: sea_orm::IntoActiveModel<ActiveModel>,
        ModelWithRelations: FromModel<<Entity as EntityTrait>::Model>
            + HasRelationMetadata<ModelWithRelations>
            + ApplyNestedIncludes<DatabaseTransaction>,
    {
        let Self {
            mut model,
            conn,
            deferred_lookups,
            post_insert_ops,
            id_extractor,
            relations_to_fetch,
            registry,
            ..
        } = self;

        for lookup in &deferred_lookups {
            let lookup_result = (lookup.resolve_on_txn)(conn, &*lookup.unique_param).await?;
            (lookup.assign)(&mut model as &mut (dyn Any + 'static), lookup_result);
        }

        let inserted = model.insert(conn).await?;
        let parent_id = (id_extractor)(&inserted);
        for op in post_insert_ops {
            (op.run_on_txn)(conn, parent_id.clone()).await?;
        }

        let mut model_with_relations = ModelWithRelations::from_model(inserted);

        for relation_filter in relations_to_fetch {
            ApplyNestedIncludes::apply_relation_filter(
                &mut model_with_relations,
                conn,
                &relation_filter,
                registry,
            )
            .await?;
        }

        Ok(model_with_relations)
    }
}
