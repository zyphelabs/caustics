use sea_orm::sea_query::{Expr, Func, SimpleExpr};
use sea_orm::{ConnectionTrait, EntityTrait, QueryFilter, QuerySelect, QueryTrait};

pub struct GroupByQueryBuilder<'a, C: ConnectionTrait, Entity: EntityTrait> {
    pub condition: sea_orm::sea_query::Condition,
    pub conn: &'a C,
    pub group_by_exprs: Vec<SimpleExpr>,
    pub group_by_columns: Vec<String>,
    pub having: Vec<SimpleExpr>,
    pub having_condition: Option<sea_orm::sea_query::Condition>,
    pub order_by: Vec<(SimpleExpr, sea_orm::Order)>,
    pub take: Option<u64>,
    pub skip: Option<u64>,
    pub aggregates: Vec<(SimpleExpr, &'static str)>,
    pub _phantom: std::marker::PhantomData<Entity>,
}

#[derive(Debug, Default, Clone)]
pub struct GroupByTypedRow {
    pub keys: std::collections::HashMap<String, String>,
    pub aggregates: std::collections::HashMap<String, String>,
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

    /// Compose a HAVING condition tree (AND/OR) in addition to simple expressions
    pub fn having_condition(mut self, cond: sea_orm::sea_query::Condition) -> Self {
        self.having_condition = match self.having_condition.take() {
            Some(existing) => Some(existing.add(cond)),
            None => Some(cond),
        };
        self
    }

    pub fn order_by<Col: sea_orm::IntoSimpleExpr>(
        mut self,
        col: Col,
        order: sea_orm::Order,
    ) -> Self {
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
        self.having
            .push(Expr::cust_with_values("COUNT(*) > ?", [v]));
        self
    }
    pub fn having_count_lt(mut self, v: i64) -> Self {
        self.having
            .push(Expr::cust_with_values("COUNT(*) < ?", [v]));
        self
    }
    pub fn having_count_eq(mut self, v: i64) -> Self {
        self.having
            .push(Expr::cust_with_values("COUNT(*) = ?", [v]));
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

    pub fn count(mut self, alias: &'static str) -> Self {
        self.aggregates.push((Expr::cust("COUNT(*)"), alias));
        self
    }

    pub fn min<F: crate::FieldSelection<Entity>>(mut self, field: F, alias: &'static str) -> Self {
        self.aggregates.push((
            SimpleExpr::FunctionCall(Func::min(field.to_simple_expr())),
            alias,
        ));
        self
    }

    pub fn max<F: crate::FieldSelection<Entity>>(mut self, field: F, alias: &'static str) -> Self {
        self.aggregates.push((
            SimpleExpr::FunctionCall(Func::max(field.to_simple_expr())),
            alias,
        ));
        self
    }

    pub fn sum<F: crate::FieldSelection<Entity>>(mut self, field: F, alias: &'static str) -> Self {
        self.aggregates.push((
            SimpleExpr::FunctionCall(Func::sum(field.to_simple_expr())),
            alias,
        ));
        self
    }

    pub fn avg<F: crate::FieldSelection<Entity>>(mut self, field: F, alias: &'static str) -> Self {
        self.aggregates.push((
            SimpleExpr::FunctionCall(Func::avg(field.to_simple_expr())),
            alias,
        ));
        self
    }

    pub async fn exec(self) -> Result<Vec<GroupByTypedRow>, sea_orm::DbErr> {
        let db_backend = self.conn.get_database_backend();
        let mut select = Entity::find().filter(self.condition).select_only();

        if !self.group_by_exprs.is_empty() {
            for (idx, expr) in self.group_by_exprs.iter().enumerate() {
                sea_orm::QueryTrait::query(&mut select).add_group_by(std::iter::once(expr.clone()));
                if let Some(alias) = self.group_by_columns.get(idx) {
                    select.expr_as(expr.clone(), alias.as_str());
                }
            }
        }

        for (expr, alias) in &self.aggregates {
            select.expr_as(expr.clone(), *alias);
        }

        for having in &self.having {
            sea_orm::QueryTrait::query(&mut select).and_having(having.clone());
        }
        if let Some(cond) = &self.having_condition {
            sea_orm::QueryTrait::query(&mut select).cond_having(cond.clone());
        }

        for (expr, ord) in &self.order_by {
            sea_orm::QueryTrait::query(&mut select).order_by_expr(expr.clone(), ord.clone());
        }

        if let Some(n) = self.take {
            sea_orm::QueryTrait::query(&mut select).limit(n);
        }
        if let Some(n) = self.skip {
            sea_orm::QueryTrait::query(&mut select).offset(n);
        }

        let stmt = select.build(db_backend);
        let rows = self.conn.query_all(stmt).await?;

        let mut out: Vec<GroupByTypedRow> = Vec::with_capacity(rows.len());
        for r in rows {
            let mut keys = std::collections::HashMap::new();
            for k in &self.group_by_columns {
                if let Ok(v) = r.try_get::<i64>("", k) {
                    keys.insert(k.clone(), v.to_string());
                    continue;
                }
                // Try to get the value as the expected type based on the field
                if let Ok(v) = r.try_get::<i32>("", k) {
                    keys.insert(k.clone(), v.to_string());
                    continue;
                }
                if let Ok(v) = r.try_get::<f64>("", k) {
                    keys.insert(k.clone(), v.to_string());
                    continue;
                }
                if let Ok(v) = r.try_get::<String>("", k) {
                    keys.insert(k.clone(), v);
                    continue;
                }
            }
            let mut aggs = std::collections::HashMap::new();
            for (_, alias) in &self.aggregates {
                if let Some(v) = crate::extract_db_value_as_string(&r, alias) {
                    aggs.insert((*alias).to_string(), v);
                    continue;
                }
            }
            out.push(GroupByTypedRow {
                keys,
                aggregates: aggs,
            });
        }
        Ok(out)
    }
}
