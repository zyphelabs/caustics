use sea_orm::{ConnectionTrait, EntityTrait, QueryFilter, QuerySelect, QueryTrait, IntoSimpleExpr};
use sea_orm::sea_query::{Expr, Func, SimpleExpr};

#[derive(Default, Debug, Clone, Copy)]
pub struct AggregateSelections {
    pub count: bool,
    pub min: bool,
    pub max: bool,
    pub sum: bool,
    pub avg: bool,
}

/// Simple aggregate query builder supporting count/min/max/sum/avg across all columns
pub struct AggregateQueryBuilder<'a, C: ConnectionTrait, Entity: EntityTrait> {
    pub condition: sea_orm::sea_query::Condition,
    pub conn: &'a C,
    pub selections: AggregateSelections,
    pub aggregates: Vec<(SimpleExpr, &'static str)>,
    pub _phantom: std::marker::PhantomData<Entity>,
}

#[derive(Debug, Default, Clone)]
pub struct AggregateResult {
    pub count: Option<i64>,
    pub values: std::collections::HashMap<String, String>,
}

impl<'a, C, Entity> AggregateQueryBuilder<'a, C, Entity>
where
    C: ConnectionTrait,
    Entity: EntityTrait,
{
    pub fn select_count(mut self) -> Self { self.selections.count = true; self }
    // Legacy, non-typed toggles preserved under *_any to avoid API conflicts with typed variants
    pub fn select_min_any(mut self) -> Self { self.selections.min = true; self }
    pub fn select_max_any(mut self) -> Self { self.selections.max = true; self }
    pub fn select_sum_any(mut self) -> Self { self.selections.sum = true; self }
    pub fn select_avg_any(mut self) -> Self { self.selections.avg = true; self }

    pub async fn exec(self) -> Result<AggregateResult, sea_orm::DbErr> {
        let db_backend = self.conn.get_database_backend();
        let mut select = Entity::find().filter(self.condition).select_only();

        // Always safe to ask COUNT(*) if requested
        if self.selections.count {
            select.expr_as(Expr::cust("COUNT(*)"), "count");
        }

        // Legacy global min/max/sum/avg: apply to the first column
        if self.selections.min || self.selections.max || self.selections.sum || self.selections.avg {
            use sea_orm::Iterable;
            if let Some(first_col) = <Entity as EntityTrait>::Column::iter().next() {
                let expr = first_col.into_simple_expr();
                if self.selections.min { select.expr_as(SimpleExpr::FunctionCall(Func::min(expr.clone())), "min"); }
                if self.selections.max { select.expr_as(SimpleExpr::FunctionCall(Func::max(expr.clone())), "max"); }
                if self.selections.sum { select.expr_as(SimpleExpr::FunctionCall(Func::sum(expr.clone())), "sum"); }
                if self.selections.avg { select.expr_as(SimpleExpr::FunctionCall(Func::avg(expr.clone())), "avg"); }
            }
        }

        // Typed per-field aggregate expressions
        for (expr, alias) in &self.aggregates {
            select.expr_as(expr.clone(), *alias);
        }

        let stmt = select.build(db_backend);
        let row = self.conn.query_one(stmt).await?;
        let mut result = AggregateResult::default();
        if let Some(r) = row {
            let mut map = std::collections::HashMap::new();
            if self.selections.count {
                if let Ok(v) = r.try_get::<i64>("", "count") { result.count = Some(v); }
            }
            if self.selections.min { if let Ok(v) = r.try_get::<String>("", "min") { map.insert("min".to_string(), v); } }
            if self.selections.max { if let Ok(v) = r.try_get::<String>("", "max") { map.insert("max".to_string(), v); } }
            if self.selections.sum { if let Ok(v) = r.try_get::<String>("", "sum") { map.insert("sum".to_string(), v); } }
            if self.selections.avg { if let Ok(v) = r.try_get::<String>("", "avg") { map.insert("avg".to_string(), v); } }
            // Capture typed aggregates by their alias, coercing to String
            for (_, alias) in &self.aggregates {
                if let Ok(v) = r.try_get::<i64>("", alias) { map.insert((*alias).to_string(), v.to_string()); continue; }
                if let Ok(v) = r.try_get::<i32>("", alias) { map.insert((*alias).to_string(), v.to_string()); continue; }
                if let Ok(v) = r.try_get::<f64>("", alias) { map.insert((*alias).to_string(), v.to_string()); continue; }
                if let Ok(v) = r.try_get::<String>("", alias) { map.insert((*alias).to_string(), v); continue; }
            }
            result.values = map;
        }
        Ok(result)
    }
}


