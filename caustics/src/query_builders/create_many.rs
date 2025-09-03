use super::deferred_lookup::{DeferredLookup, DeferredResolveFor};
use sea_orm::{ConnectionTrait, EntityTrait, IntoActiveModel};
use std::any::Any;

/// Query builder for creating many records; returns affected row count
pub struct CreateManyQueryBuilder<'a, C: ConnectionTrait, Entity: EntityTrait, ActiveModel>
where
    ActiveModel: sea_orm::ActiveModelTrait<Entity = Entity> + sea_orm::ActiveModelBehavior + Send + 'static,
{
    pub items: Vec<(ActiveModel, Vec<DeferredLookup>)>,
    pub conn: &'a C,
    pub _phantom: std::marker::PhantomData<(Entity, ActiveModel)>,
}

impl<'a, C, Entity, ActiveModel> CreateManyQueryBuilder<'a, C, Entity, ActiveModel>
where
    C: ConnectionTrait,
    Entity: EntityTrait,
    ActiveModel: sea_orm::ActiveModelTrait<Entity = Entity> + sea_orm::ActiveModelBehavior + Send + 'static,
{
    /// Execute all inserts and return number of rows inserted
    pub async fn exec(self) -> Result<i64, sea_orm::DbErr>
    where
        <Entity as EntityTrait>::Model: IntoActiveModel<ActiveModel>,
        DeferredLookup: DeferredResolveFor<C>,
    {
        let mut affected: i64 = 0;
        for (mut model, lookups) in self.items {
            // Resolve deferred lookups against a connection
            for lookup in &lookups {
                let value = lookup.resolve_for(self.conn).await?;
                (lookup.assign)(&mut model as &mut (dyn Any + 'static), value);
            }
            let res = model.insert(self.conn).await?;
            let _ = res; // silence unused warning on some backends
            affected += 1;
        }
        Ok(affected)
    }
}

