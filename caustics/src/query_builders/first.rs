use crate::types::ApplyNestedIncludes;
use crate::types::EntityRegistry;
use crate::types::SelectionSpec;
use crate::types::{IntoOrderSpec, NullsOrder};
use crate::EntitySelection;
use crate::{FromModel, HasRelationMetadata, RelationFilter};
use sea_orm::sea_query::{Expr, SimpleExpr};
use sea_orm::{ConnectionTrait, DatabaseBackend, EntityTrait, QueryOrder, Select};

/// Query builder for finding the first entity record matching conditions
pub struct FirstQueryBuilder<'a, C: ConnectionTrait, Entity: EntityTrait, ModelWithRelations> {
    pub query: Select<Entity>,
    pub conn: &'a C,
    pub relations_to_fetch: Vec<RelationFilter>,
    pub registry: &'a (dyn EntityRegistry<C> + Sync),
    pub database_backend: DatabaseBackend,
    pub pending_order_bys: Vec<(SimpleExpr, sea_orm::Order)>,
    pub pending_nulls: Option<NullsOrder>,
    pub _phantom: std::marker::PhantomData<ModelWithRelations>,
}

impl<'a, C: ConnectionTrait, Entity: EntityTrait, ModelWithRelations>
    FirstQueryBuilder<'a, C, Entity, ModelWithRelations>
where
    ModelWithRelations: FromModel<Entity::Model>
        + HasRelationMetadata<ModelWithRelations>
        + crate::types::ApplyNestedIncludes<C>
        + Send
        + 'static,
{
    pub fn select<S>(
        self,
        spec: S,
    ) -> crate::query_builders::select_first::SelectFirstQueryBuilder<'a, C, Entity, S::Data>
    where
        S: SelectionSpec<Entity = Entity>,
        S::Data: EntitySelection + HasRelationMetadata<S::Data> + Send + 'static,
    {
        let mut builder = crate::query_builders::select_first::SelectFirstQueryBuilder {
            query: self.query,
            conn: self.conn,
            selected_fields: Vec::new(),
            requested_aliases: Vec::new(),
            relations_to_fetch: self.relations_to_fetch,
            registry: self.registry,
            database_backend: self.database_backend,
            pending_order_bys: self.pending_order_bys,
            pending_nulls: self.pending_nulls,
            _phantom: std::marker::PhantomData,
        };
        let aliases = spec.collect_aliases();
        for alias in aliases {
            if let Some(expr) = <S::Data as EntitySelection>::column_for_alias(alias.as_str()) {
                builder = builder.push_field(expr, alias.as_str());
                builder.requested_aliases.push(alias);
            }
        }
        builder
    }

    /// Order the result deterministically when multiple rows match
    pub fn order_by<T>(mut self, order_spec: T) -> Self
    where
        T: IntoOrderSpec,
    {
        let (expr, order, nulls) = order_spec.into_order_spec();
        self.pending_order_bys.push((expr, order));
        if nulls.is_some() {
            self.pending_nulls = nulls;
        }
        self
    }
    /// Execute the query and return a single result
    pub async fn exec(self) -> Result<Option<ModelWithRelations>, sea_orm::DbErr> {
        if self.relations_to_fetch.is_empty() {
            let mut query = self.query;
            // Apply NULLS ordering hint if provided, before actual order clauses
            if let Some(n) = self.pending_nulls {
                if let Some((first_expr, _)) = self.pending_order_bys.first() {
                    let nulls_expr = Expr::expr(first_expr.clone()).is_null();
                    match n {
                        NullsOrder::First => {
                            query = query.order_by(nulls_expr, sea_orm::Order::Desc);
                        }
                        NullsOrder::Last => {
                            query = query.order_by(nulls_expr, sea_orm::Order::Asc);
                        }
                    }
                }
            }
            for (expr, order) in &self.pending_order_bys {
                query = query.order_by(expr.clone(), order.clone());
            }
            query
                .one(self.conn)
                .await
                .map(|opt| opt.map(|model| ModelWithRelations::from_model(model)))
        } else {
            self.exec_with_relations().await
        }
    }

    /// Add a relation to fetch with the query
    pub fn with<T: Into<RelationFilter>>(mut self, relation: T) -> Self {
        self.relations_to_fetch.push(relation.into());
        self
    }

    /// Execute query with relations
    async fn exec_with_relations(self) -> Result<Option<ModelWithRelations>, sea_orm::DbErr>
    where
        ModelWithRelations: FromModel<Entity::Model>,
    {
        let Self {
            query,
            conn,
            relations_to_fetch,
            registry,
            pending_order_bys,
            pending_nulls,
            ..
        } = self;
        // Apply ordering to ensure deterministic first row
        let mut ordered = query;
        if let Some(n) = pending_nulls {
            if let Some((first_expr, _)) = pending_order_bys.first() {
                let nulls_expr = Expr::expr(first_expr.clone()).is_null();
                match n {
                    NullsOrder::First => {
                        ordered = ordered.order_by(nulls_expr, sea_orm::Order::Desc);
                    }
                    NullsOrder::Last => {
                        ordered = ordered.order_by(nulls_expr, sea_orm::Order::Asc);
                    }
                }
            }
        }
        for (expr, order) in &pending_order_bys {
            ordered = ordered.order_by(expr.clone(), order.clone());
        }
        let main_result = ordered.one(conn).await?;

        if let Some(main_model) = main_result {
            let mut model_with_relations = ModelWithRelations::from_model(main_model);

            // Fetch relations for the main model (nested-aware)
            for relation_filter in relations_to_fetch {
                ApplyNestedIncludes::apply_relation_filter(
                    &mut model_with_relations,
                    conn,
                    &relation_filter,
                    registry,
                )
                .await?;
            }

            Ok(Some(model_with_relations))
        } else {
            Ok(None)
        }
    }
}
