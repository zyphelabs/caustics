use crate::MergeInto;
use sea_orm::{ConnectionTrait, EntityTrait, IntoActiveModel, QueryFilter};

/// Query builder for updating many records; returns affected row count
pub struct UpdateManyQueryBuilder<'a, C: ConnectionTrait, Entity: EntityTrait, ActiveModel, T>
where
    ActiveModel: sea_orm::ActiveModelTrait<Entity = Entity> + sea_orm::ActiveModelBehavior + Send,
    T: MergeInto<ActiveModel>,
{
    pub condition: sea_orm::Condition,
    pub changes: Vec<T>,
    pub conn: &'a C,
    pub _phantom: std::marker::PhantomData<(Entity, ActiveModel)>,
}

impl<'a, C, Entity, ActiveModel, T> UpdateManyQueryBuilder<'a, C, Entity, ActiveModel, T>
where
    C: ConnectionTrait,
    Entity: EntityTrait,
    ActiveModel: sea_orm::ActiveModelTrait<Entity = Entity> + sea_orm::ActiveModelBehavior + Send,
    T: MergeInto<ActiveModel>,
{
    /// Update all matching records and return number of rows affected
    pub async fn exec(self) -> Result<i64, sea_orm::DbErr>
    where
        <Entity as EntityTrait>::Model: IntoActiveModel<ActiveModel>,
    {
        // Select all matching rows, update individually for portability
        let rows = <Entity as EntityTrait>::find()
            .filter::<sea_orm::Condition>(self.condition)
            .all(self.conn)
            .await?;
        let mut affected: i64 = 0;
        for row in rows {
            let mut am: ActiveModel = row.into_active_model();
            for change in &self.changes {
                change.merge_into(&mut am);
            }
            let _ = am.update(self.conn).await?;
            affected += 1;
        }
        Ok(affected)
    }
}
