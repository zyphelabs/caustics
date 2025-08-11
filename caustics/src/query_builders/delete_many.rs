use sea_orm::{ConnectionTrait, DatabaseTransaction, EntityTrait, QueryFilter};

/// Query builder for deleting multiple entity records matching a condition
pub struct DeleteManyQueryBuilder<'a, C: ConnectionTrait, Entity: EntityTrait> {
    pub condition: sea_orm::Condition,
    pub conn: &'a C,
    pub _phantom: std::marker::PhantomData<Entity>,
}

impl<'a, C, Entity> DeleteManyQueryBuilder<'a, C, Entity>
where
    C: ConnectionTrait,
    Entity: EntityTrait,
{
    /// Delete all matching records and return the number of rows affected
    pub async fn exec(self) -> Result<i64, sea_orm::DbErr> {
        let res = Entity::delete_many()
            .filter::<sea_orm::Condition>(self.condition)
            .exec(self.conn)
            .await?;
        Ok(res.rows_affected as i64)
    }

    /// Execute the query within a transaction
    pub async fn exec_in_txn(self, txn: &DatabaseTransaction) -> Result<i64, sea_orm::DbErr> {
        let res = Entity::delete_many()
            .filter::<sea_orm::Condition>(self.condition)
            .exec(txn)
            .await?;
        Ok(res.rows_affected as i64)
    }
}


