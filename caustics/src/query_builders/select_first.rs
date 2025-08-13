use crate::{EntitySelection, HasRelationMetadata, RelationFilter};
use crate::types::{EntityRegistry};
use sea_orm::{ConnectionTrait, DatabaseBackend, EntityTrait, QuerySelect, QueryTrait, Select};
use sea_orm::sea_query::SimpleExpr;

/// Query builder for selected scalar fields on first
pub struct SelectFirstQueryBuilder<'a, C: ConnectionTrait, Entity: EntityTrait, Selected>
where
    Selected: EntitySelection + HasRelationMetadata<Selected> + Send + 'static,
{
    pub query: Select<Entity>,
    pub conn: &'a C,
    pub selected_fields: Vec<(SimpleExpr, String)>,
    pub requested_aliases: Vec<String>,
    pub relations_to_fetch: Vec<RelationFilter>,
    pub registry: &'a dyn EntityRegistry<C>,
    pub database_backend: DatabaseBackend,
    pub _phantom: std::marker::PhantomData<Selected>,
}

impl<'a, C, Entity, Selected> SelectFirstQueryBuilder<'a, C, Entity, Selected>
where
    C: ConnectionTrait,
    Entity: EntityTrait,
    Selected: EntitySelection + HasRelationMetadata<Selected> + Send + 'static,
{
    pub fn push_field(mut self, expr: SimpleExpr, alias: &str) -> Self {
        self.selected_fields.push((expr, alias.to_string()));
        self
    }

    pub async fn exec(self) -> Result<Option<Selected>, sea_orm::DbErr> {
        // Ensure required key columns for any requested relations are added implicitly via Selected::column_for_alias
        let query = self.query.clone();
        let mut selected = self.selected_fields.clone();
        if !self.relations_to_fetch.is_empty() {
            for rf in &self.relations_to_fetch {
                if let Some(desc) = Selected::get_relation_descriptor(rf.relation) {
                    let needed_alias = if desc.is_has_many { desc.current_primary_key_field_name } else { desc.foreign_key_field_name };
                    if !self.requested_aliases.iter().any(|a| a == needed_alias) {
                        if let Some(expr) = Selected::column_for_alias(needed_alias) {
                            selected.push((expr, needed_alias.to_string()));
                        }
                    }
                }
            }
        }
        let mut select = query.select_only();
        for (expr, alias) in &selected {
            select.expr_as(expr.clone(), alias.as_str());
        }
        let stmt = select.build(self.database_backend);
        if let Some(row) = self.conn.query_one(stmt).await? {
            let field_names: Vec<&str> = self.requested_aliases.iter().map(|a| a.as_str()).collect();
            let mut s = Selected::fill_from_row(&row, &field_names);

            for rf in &self.relations_to_fetch {
                if let Some(desc) = Selected::get_relation_descriptor(rf.relation) {
                    let fk_val = if desc.is_has_many { s.get_i32(desc.current_primary_key_field_name) } else { s.get_i32(desc.foreign_key_field_name) };
                    if let Some(fk) = fk_val {
                        let fetcher = self
                            .registry
                            .get_fetcher(desc.target_entity)
                            .ok_or_else(|| sea_orm::DbErr::Custom(format!(
                                "Missing fetcher for {}",
                                desc.target_entity
                            )))?;
                        let res = fetcher
                            .fetch_by_foreign_key(self.conn, Some(fk), desc.foreign_key_column, desc.target_entity, rf.relation)
                            .await?;
                        s.set_relation(rf.relation, res);
                    }
                }
            }

            s.clear_unselected(&field_names);
            Ok(Some(s))
        } else {
            Ok(None)
        }
    }

    pub fn with<T: Into<RelationFilter>>(mut self, relation: T) -> Self {
        self.relations_to_fetch.push(relation.into());
        self
    }
}


