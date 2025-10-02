use super::deferred_lookup::{DeferredLookup, DeferredResolveFor};
use crate::PostInsertOp;
use sea_orm::{
    ConnectionTrait, DatabaseConnection, DatabaseTransaction, EntityTrait, IntoActiveModel,
};
use std::any::Any;

/// Query builder for creating many records; returns affected row count
pub struct CreateManyQueryBuilder<'a, C: ConnectionTrait, Entity: EntityTrait, ActiveModel>
where
    ActiveModel:
        sea_orm::ActiveModelTrait<Entity = Entity> + sea_orm::ActiveModelBehavior + Send + 'static,
{
    #[allow(clippy::type_complexity)]
    pub items: Vec<(
        ActiveModel,
        Vec<DeferredLookup>,
        Vec<PostInsertOp<'a>>,
        fn(&<Entity as EntityTrait>::Model) -> i32,
    )>,
    pub conn: &'a C,
    pub _phantom: std::marker::PhantomData<(Entity, ActiveModel)>,
}

impl<'a, Entity, ActiveModel> CreateManyQueryBuilder<'a, DatabaseConnection, Entity, ActiveModel>
where
    Entity: EntityTrait,
    ActiveModel:
        sea_orm::ActiveModelTrait<Entity = Entity> + sea_orm::ActiveModelBehavior + Send + 'static,
{
    /// Execute all inserts and return number of rows inserted
    pub async fn exec(self) -> Result<i64, sea_orm::DbErr>
    where
        <Entity as EntityTrait>::Model: IntoActiveModel<ActiveModel>,
        DeferredLookup: DeferredResolveFor<DatabaseConnection>,
    {
        let mut affected: i64 = 0;
        for (mut model, lookups, post_ops, id_extractor) in self.items {
            for lookup in &lookups {
                let value = lookup.resolve_for(self.conn).await?;
                (lookup.assign)(&mut model as &mut (dyn Any + 'static), value);
            }
            let inserted = model.insert(self.conn).await?;
            let parent_id = (id_extractor)(&inserted);
            for op in post_ops {
                (op.run_on_conn)(self.conn, parent_id).await?;
            }
            affected += 1;
        }
        Ok(affected)
    }
}

impl<'a, Entity, ActiveModel> CreateManyQueryBuilder<'a, DatabaseTransaction, Entity, ActiveModel>
where
    Entity: EntityTrait,
    ActiveModel:
        sea_orm::ActiveModelTrait<Entity = Entity> + sea_orm::ActiveModelBehavior + Send + 'static,
{
    /// Execute all inserts in a transaction and return number of rows inserted
    pub async fn exec(self) -> Result<i64, sea_orm::DbErr>
    where
        <Entity as EntityTrait>::Model: IntoActiveModel<ActiveModel>,
        DeferredLookup: DeferredResolveFor<DatabaseTransaction>,
    {
        let mut affected: i64 = 0;
        for (mut model, lookups, post_ops, id_extractor) in self.items {
            for lookup in &lookups {
                let value = lookup.resolve_for(self.conn).await?;
                (lookup.assign)(&mut model as &mut (dyn Any + 'static), value);
            }
            let inserted = model.insert(self.conn).await?;
            let parent_id = (id_extractor)(&inserted);
            for op in post_ops {
                (op.run_on_txn)(self.conn, parent_id).await?;
            }
            affected += 1;
        }
        Ok(affected)
    }
}
