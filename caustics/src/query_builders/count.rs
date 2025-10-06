use sea_orm::sea_query::Condition as SeaQueryCondition;
use sea_orm::sea_query::Expr;
use sea_orm::{ConnectionTrait, EntityTrait, QueryFilter, QuerySelect, QueryTrait};

/// Query builder for counting entity records matching conditions
pub struct CountQueryBuilder<'a, C: ConnectionTrait, Entity: EntityTrait> {
    pub condition: SeaQueryCondition,
    pub conn: &'a C,
    pub _phantom: std::marker::PhantomData<Entity>,
}

impl<'a, C, Entity> CountQueryBuilder<'a, C, Entity>
where
    C: ConnectionTrait,
    Entity: EntityTrait,
{
    pub async fn exec(self) -> Result<i64, sea_orm::DbErr> {
        let db_backend = self.conn.get_database_backend();
        let select = Entity::find().filter(self.condition).select_only();
        let select = select.expr_as(Expr::cust("COUNT(*)"), "count");
        let stmt = select.build(db_backend);
        let row = self.conn.query_one(stmt).await?;
        let count = match row {
            Some(r) => r.try_get::<i64>("", "count").unwrap_or(0),
            None => 0,
        };
        Ok(count)
    }
}
