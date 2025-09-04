use super::deferred_lookup::DeferredLookup;
use crate::{FromModel, MergeInto, PostInsertOp};
use sea_orm::{ConnectionTrait, DatabaseConnection, DatabaseTransaction, EntityTrait, IntoActiveModel, QueryFilter};
use std::any::Any;

/// Query builder for upserting (insert or update) entity records
pub struct UpsertQueryBuilder<
    'a,
    C: ConnectionTrait,
    Entity: EntityTrait,
    ActiveModel: sea_orm::ActiveModelTrait<Entity = Entity> + sea_orm::ActiveModelBehavior + Send + 'static,
    ModelWithRelations,
    T: MergeInto<ActiveModel>,
> {
    pub condition: sea_orm::Condition,
    pub create: (
        ActiveModel,
        Vec<DeferredLookup>,
        Vec<PostInsertOp<'a>>,
        fn(&<Entity as EntityTrait>::Model) -> i32,
    ),
    pub update: Vec<T>,
    pub conn: &'a C,
    pub _phantom: std::marker::PhantomData<(Entity, ModelWithRelations)>,
}

impl<'a, C, Entity, ActiveModel, ModelWithRelations, T>
    UpsertQueryBuilder<'a, C, Entity, ActiveModel, ModelWithRelations, T>
where
    C: ConnectionTrait,
    Entity: EntityTrait,
    ActiveModel:
        sea_orm::ActiveModelTrait<Entity = Entity> + sea_orm::ActiveModelBehavior + Send + 'static,
    ModelWithRelations: FromModel<<Entity as EntityTrait>::Model>,
    T: MergeInto<ActiveModel>,
    <Entity as EntityTrait>::Model: sea_orm::IntoActiveModel<ActiveModel>,
{

    /// Execute the upsert within a transaction
    pub async fn exec_in_txn(
        self,
        txn: &DatabaseTransaction,
    ) -> Result<ModelWithRelations, sea_orm::DbErr>
    {
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
                active_model
                    .update(txn)
                    .await
                    .map(ModelWithRelations::from_model)
            }
            None => {
                let (mut active_model, deferred_lookups, post_ops, id_extractor) = self.create;
        for lookup in &deferred_lookups {
            let lookup_result = (lookup.resolve_on_txn)(txn, &*lookup.unique_param).await?;
                    (lookup.assign)(&mut active_model as &mut (dyn std::any::Any + 'static), lookup_result);
                }
                for change in self.update {
                    change.merge_into(&mut active_model);
                }
                let inserted = active_model.insert(txn).await?;
                let parent_id: i32 = (id_extractor)(&inserted);
                for op in post_ops {
                    (op.run_on_txn)(txn, parent_id).await?;
                }
                Ok(ModelWithRelations::from_model(inserted))
            }
        }
    }
}

impl<'a, Entity, ActiveModel, ModelWithRelations, T>
    UpsertQueryBuilder<'a, DatabaseConnection, Entity, ActiveModel, ModelWithRelations, T>
where
    Entity: EntityTrait,
    ActiveModel:
        sea_orm::ActiveModelTrait<Entity = Entity> + sea_orm::ActiveModelBehavior + Send + 'static,
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
                let (mut active_model, deferred_lookups, post_ops, id_extractor) = self.create;
                // Execute all deferred lookups in batch (if needed)
                for lookup in &deferred_lookups {
                    let lookup_result = (lookup.resolve_on_conn)(self.conn, &*lookup.unique_param).await?;
                    (lookup.assign)(&mut active_model as &mut (dyn Any + 'static), lookup_result);
                }
                for change in self.update {
                    change.merge_into(&mut active_model);
                }
                let inserted = active_model.insert(self.conn).await?;
                let parent_id: i32 = (id_extractor)(&inserted);
                for op in post_ops {
                    (op.run_on_conn)(self.conn, parent_id).await?;
                }
                Ok(ModelWithRelations::from_model(inserted))
            }
        }
    }
}

