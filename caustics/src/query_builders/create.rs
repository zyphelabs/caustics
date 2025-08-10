use super::deferred_lookup::DeferredLookup;
use crate::FromModel;
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

        model.insert(txn).await.map(ModelWithRelations::from_model)
    }
}

impl<'a, Entity, ActiveModel, ModelWithRelations>
    CreateQueryBuilder<'a, DatabaseConnection, Entity, ActiveModel, ModelWithRelations>
where
    Entity: EntityTrait,
    ActiveModel:
        sea_orm::ActiveModelTrait<Entity = Entity> + sea_orm::ActiveModelBehavior + Send + 'static,
    ModelWithRelations: FromModel<<Entity as EntityTrait>::Model>,
{
    pub async fn exec(self) -> Result<ModelWithRelations, sea_orm::DbErr>
    where
        <Entity as EntityTrait>::Model: sea_orm::IntoActiveModel<ActiveModel>,
    {
        let mut model = self.model;

        // Execute all deferred lookups in batch
        for lookup in &self.deferred_lookups {
            let lookup_result = (lookup.resolve_on_conn)(self.conn, &*lookup.unique_param).await?;
            (lookup.assign)(&mut model as &mut (dyn Any + 'static), lookup_result);
        }

        model
            .insert(self.conn)
            .await
            .map(ModelWithRelations::from_model)
    }
}

impl<'a, Entity, ActiveModel, ModelWithRelations>
    CreateQueryBuilder<'a, DatabaseTransaction, Entity, ActiveModel, ModelWithRelations>
where
    Entity: EntityTrait,
    ActiveModel:
        sea_orm::ActiveModelTrait<Entity = Entity> + sea_orm::ActiveModelBehavior + Send + 'static,
    ModelWithRelations: FromModel<<Entity as EntityTrait>::Model>,
{
    pub async fn exec(self) -> Result<ModelWithRelations, sea_orm::DbErr>
    where
        <Entity as EntityTrait>::Model: sea_orm::IntoActiveModel<ActiveModel>,
    {
        let mut model = self.model;

        for lookup in &self.deferred_lookups {
            let lookup_result = (lookup.resolve_on_txn)(self.conn, &*lookup.unique_param).await?;
            (lookup.assign)(&mut model as &mut (dyn Any + 'static), lookup_result);
        }

        model
            .insert(self.conn)
            .await
            .map(ModelWithRelations::from_model)
    }
}

