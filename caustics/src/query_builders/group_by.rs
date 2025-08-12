use sea_orm::{ConnectionTrait, EntityTrait, QueryFilter, QuerySelect, QueryTrait};
use sea_orm::sea_query::{Expr, Func, SimpleExpr};

pub struct GroupByQueryBuilder<'a, C: ConnectionTrait, Entity: EntityTrait> {
    pub condition: sea_orm::sea_query::Condition,
    pub conn: &'a C,
    pub group_by_exprs: Vec<SimpleExpr>,
    pub having: Vec<SimpleExpr>,
    pub order_by: Vec<(SimpleExpr, sea_orm::Order)>,
    pub take: Option<u64>,
    pub skip: Option<u64>,
    pub aggregates: Vec<(SimpleExpr, &'static str)>,
    pub _phantom: std::marker::PhantomData<Entity>,
}

impl<'a, C, Entity> GroupByQueryBuilder<'a, C, Entity>
where
    C: ConnectionTrait,
    Entity: EntityTrait,
{
    /// Add multiple group-by expressions at once
    pub fn by_params(mut self, exprs: Vec<SimpleExpr>) -> Self {
        self.group_by_exprs.extend(exprs);
        self
    }
    pub fn by<Col: sea_orm::IntoSimpleExpr>(mut self, col: Col) -> Self {
        self.group_by_exprs.push(col.into_simple_expr());
        self
    }

    pub fn having(mut self, cond: SimpleExpr) -> Self {
        self.having.push(cond);
        self
    }

    pub fn order_by<Col: sea_orm::IntoSimpleExpr>(mut self, col: Col, order: sea_orm::Order) -> Self {
        self.order_by.push((col.into_simple_expr(), order));
        self
    }

    /// Add multiple order by pairs at once
    pub fn order_by_pairs(mut self, pairs: Vec<(SimpleExpr, sea_orm::Order)>) -> Self {
        self.order_by.extend(pairs);
        self
    }

    // Minimal typed helpers for having on count
    pub fn having_count_gt(mut self, v: i64) -> Self {
        self.having.push(Expr::cust_with_values("COUNT(*) > ?", [v]));
        self
    }
    pub fn having_count_lt(mut self, v: i64) -> Self {
        self.having.push(Expr::cust_with_values("COUNT(*) < ?", [v]));
        self
    }
    pub fn having_count_eq(mut self, v: i64) -> Self {
        self.having.push(Expr::cust_with_values("COUNT(*) = ?", [v]));
        self
    }

    pub fn take(mut self, n: i64) -> Self {
        let n = if n < 0 { 0 } else { n as u64 };
        self.take = Some(n);
        self
    }
    pub fn skip(mut self, n: i64) -> Self {
        let n = if n < 0 { 0 } else { n as u64 };
        self.skip = Some(n);
        self
    }

    pub fn select_count(mut self, alias: &'static str) -> Self {
        self.aggregates.push((Expr::cust("COUNT(*)"), alias));
        self
    }

    pub fn select_min<Col: sea_orm::IntoSimpleExpr>(mut self, col: Col, alias: &'static str) -> Self {
        self.aggregates.push((SimpleExpr::FunctionCall(Func::min(col.into_simple_expr())), alias));
        self
    }

    pub fn select_max<Col: sea_orm::IntoSimpleExpr>(mut self, col: Col, alias: &'static str) -> Self {
        self.aggregates.push((SimpleExpr::FunctionCall(Func::max(col.into_simple_expr())), alias));
        self
    }

    pub fn select_sum<Col: sea_orm::IntoSimpleExpr>(mut self, col: Col, alias: &'static str) -> Self {
        self.aggregates.push((SimpleExpr::FunctionCall(Func::sum(col.into_simple_expr())), alias));
        self
    }

    pub fn select_avg<Col: sea_orm::IntoSimpleExpr>(mut self, col: Col, alias: &'static str) -> Self {
        self.aggregates.push((SimpleExpr::FunctionCall(Func::avg(col.into_simple_expr())), alias));
        self
    }

    pub async fn exec(self) -> Result<Vec<sea_orm::QueryResult>, sea_orm::DbErr> {
        let db_backend = self.conn.get_database_backend();
        let mut select = Entity::find().filter(self.condition).select_only();

        // group by
        // project group-by columns once via raw SQL to avoid moving select in loop
        if !self.group_by_exprs.is_empty() {
            for expr in &self.group_by_exprs {
                sea_orm::QueryTrait::query(&mut select).add_group_by(std::iter::once(expr.clone()));
            }
        }

        // aggregates
        for (expr, alias) in &self.aggregates {
            select.expr_as(expr.clone(), *alias);
        }

        // having
        for having in &self.having {
            sea_orm::QueryTrait::query(&mut select).and_having(having.clone());
        }

        // order
        for (expr, ord) in &self.order_by {
            sea_orm::QueryTrait::query(&mut select).order_by_expr(expr.clone(), ord.clone());
        }

        // pagination
        if let Some(n) = self.take { sea_orm::QueryTrait::query(&mut select).limit(n); }
        if let Some(n) = self.skip { sea_orm::QueryTrait::query(&mut select).offset(n); }

        let stmt = select.build(db_backend);
        let rows = self.conn.query_all(stmt).await?;
        Ok(rows)
    }
}


