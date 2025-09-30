use sea_orm::sea_query::{Expr, Func, SimpleExpr};
use sea_orm::{ConnectionTrait, EntityTrait, IntoSimpleExpr, QueryFilter, QuerySelect, QueryTrait};

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
    pub aggregates: Vec<(SimpleExpr, &'static str, &'static str)>,
    pub _phantom: std::marker::PhantomData<Entity>,
}

#[derive(Debug, Default, Clone)]
pub struct AggregateTypedResult {
    pub count: Option<i64>,
    pub sum: std::collections::HashMap<String, String>,
    pub avg: std::collections::HashMap<String, String>,
    pub min: std::collections::HashMap<String, String>,
    pub max: std::collections::HashMap<String, String>,
}

impl<'a, C, Entity> AggregateQueryBuilder<'a, C, Entity>
where
    C: ConnectionTrait,
    Entity: EntityTrait,
{
    pub fn count(mut self) -> Self {
        self.selections.count = true;
        self
    }

    pub async fn exec(self) -> Result<AggregateTypedResult, sea_orm::DbErr> {
        let db_backend = self.conn.get_database_backend();
        let mut select = Entity::find().filter(self.condition).select_only();

        if self.selections.count {
            select.expr_as(Expr::cust("COUNT(*)"), "count");
        }

        if self.selections.min || self.selections.max || self.selections.sum || self.selections.avg
        {
            use sea_orm::Iterable;
            if let Some(first_col) = <Entity as EntityTrait>::Column::iter().next() {
                let expr = first_col.into_simple_expr();
                if self.selections.min {
                    select.expr_as(SimpleExpr::FunctionCall(Func::min(expr.clone())), "min");
                }
                if self.selections.max {
                    select.expr_as(SimpleExpr::FunctionCall(Func::max(expr.clone())), "max");
                }
                if self.selections.sum {
                    select.expr_as(SimpleExpr::FunctionCall(Func::sum(expr.clone())), "sum");
                }
                if self.selections.avg {
                    select.expr_as(SimpleExpr::FunctionCall(Func::avg(expr.clone())), "avg");
                }
            }
        }

        for (expr, alias, _) in &self.aggregates {
            select.expr_as(expr.clone(), *alias);
        }

        let stmt = select.build(db_backend);
        let row = self.conn.query_one(stmt).await?;

        let mut typed = AggregateTypedResult::default();
        if let Some(r) = row {
            if self.selections.count {
                if let Ok(v) = r.try_get::<i64>("", "count") {
                    typed.count = Some(v);
                }
            }
            if self.selections.min {
                if let Ok(v) = r.try_get::<String>("", "min") {
                    typed.min.insert("_first".to_string(), v);
                }
            }
            if self.selections.max {
                if let Ok(v) = r.try_get::<String>("", "max") {
                    typed.max.insert("_first".to_string(), v);
                }
            }
            if self.selections.sum {
                if let Ok(v) = r.try_get::<String>("", "sum") {
                    typed.sum.insert("_first".to_string(), v);
                }
            }
            if self.selections.avg {
                if let Ok(v) = r.try_get::<String>("", "avg") {
                    typed.avg.insert("_first".to_string(), v);
                }
            }
            for (_, alias, kind) in &self.aggregates {
                let mut as_string: Option<String> = None;
                if let Ok(v) = r.try_get::<i64>("", alias) {
                    as_string = Some(v.to_string());
                } else if let Ok(v) = r.try_get::<i32>("", alias) {
                    as_string = Some(v.to_string());
                } else if let Ok(v) = r.try_get::<f64>("", alias) {
                    as_string = Some(v.to_string());
                } else if let Ok(v) = r.try_get::<String>("", alias) {
                    as_string = Some(v);
                }

                if let Some(vs) = as_string {
                    match *kind {
                        "sum" => {
                            typed.sum.insert((*alias).to_string(), vs);
                        }
                        "avg" => {
                            typed.avg.insert((*alias).to_string(), vs);
                        }
                        "min" => {
                            typed.min.insert((*alias).to_string(), vs);
                        }
                        "max" => {
                            typed.max.insert((*alias).to_string(), vs);
                        }
                        _ => {}
                    }
                }
            }
        }
        Ok(typed)
    }
}
