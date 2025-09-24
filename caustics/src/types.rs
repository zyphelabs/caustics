#![allow(non_camel_case_types)]

use sea_orm::sea_query;
use sea_orm::{DatabaseConnection, DatabaseTransaction};
use std::any::Any;

pub type QueryError = sea_orm::DbErr;
// Crate-wide result alias for ergonomics (non-conflicting)
pub type CausticsResult<T> = std::result::Result<T, sea_orm::DbErr>;

/// Typed Caustics errors that can be converted into `sea_orm::DbErr`
#[derive(Debug, Clone)]
pub enum CausticsError {
    // Include/Relation errors
    RelationNotFound { relation: String },
    InvalidIncludePath { relation: String },
    RelationNotFetched { relation: String, reason: String },
    EntityFetcherMissing { entity: String },
    DeferredLookupFailed { target: String, detail: String },
    NotFoundForCondition { entity: String, condition: String },
    QueryValidation { message: String },
}

impl core::fmt::Display for CausticsError {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        match self {
            CausticsError::RelationNotFound { relation } => {
                write!(
                    f,
                    "CausticsError::RelationNotFound: relation='{}'",
                    relation
                )
            }
            CausticsError::InvalidIncludePath { relation } => {
                write!(
                    f,
                    "CausticsError::InvalidIncludePath: relation='{}'",
                    relation
                )
            }
            CausticsError::RelationNotFetched { relation, reason } => {
                write!(
                    f,
                    "CausticsError::RelationNotFetched: relation='{}' reason='{}'",
                    relation, reason
                )
            }
            CausticsError::EntityFetcherMissing { entity } => {
                write!(
                    f,
                    "CausticsError::EntityFetcherMissing: entity='{}'",
                    entity
                )
            }
            CausticsError::DeferredLookupFailed { target, detail } => {
                write!(
                    f,
                    "CausticsError::DeferredLookupFailed: target='{}' detail='{}'",
                    target, detail
                )
            }
            CausticsError::NotFoundForCondition { entity, condition } => {
                write!(
                    f,
                    "CausticsError::NotFoundForCondition: entity='{}' condition='{}'",
                    entity, condition
                )
            }
            CausticsError::QueryValidation { message } => {
                write!(f, "CausticsError::QueryValidation: {}", message)
            }
        }
    }
}

impl From<CausticsError> for sea_orm::DbErr {
    fn from(err: CausticsError) -> Self {
        sea_orm::DbErr::Custom(err.to_string())
    }
}

/// Operation to run after a parent insert completes (used by nested writes)
pub struct PostInsertOp<'a> {
    pub run_on_conn: Box<
        dyn for<'b> Fn(
                &'b DatabaseConnection,
                i32,
            ) -> std::pin::Pin<
                Box<dyn std::future::Future<Output = Result<(), sea_orm::DbErr>> + Send + 'b>,
            > + Send
            + 'a,
    >,
    pub run_on_txn: Box<
        dyn for<'b> Fn(
                &'b DatabaseTransaction,
                i32,
            ) -> std::pin::Pin<
                Box<dyn std::future::Future<Output = Result<(), sea_orm::DbErr>> + Send + 'b>,
            > + Send
            + 'a,
    >,
}

// Import query builder types for batch operations
use crate::query_builders::{BatchQuery, BatchResult};

#[derive(Copy, Clone, Debug)]
pub enum SortOrder {
    Asc,
    Desc,
}

#[derive(Copy, Clone, Debug)]
pub enum NullsOrder {
    First,
    Last,
}

#[derive(Copy, Clone, Debug, PartialEq, Eq)]
pub enum QueryMode {
    Default,
    Insensitive,
}

/// Generic field operations for filtering
#[derive(Debug, Clone)]
pub enum FieldOp<T> {
    Equals(T),
    NotEquals(T),
    Gt(T),
    Lt(T),
    Gte(T),
    Lte(T),
    InVec(Vec<T>),
    NotInVec(Vec<T>),
    Contains(String),
    StartsWith(String),
    EndsWith(String),
    IsNull,
    IsNotNull,
    // JSON-specific operations
    JsonPath(Vec<String>),
    JsonStringContains(String),
    JsonStringStartsWith(String),
    JsonStringEndsWith(String),
    JsonArrayContains(serde_json::Value),
    JsonArrayStartsWith(serde_json::Value),
    JsonArrayEndsWith(serde_json::Value),
    JsonObjectContains(String),
    // JSON null handling flags
    JsonNull(JsonNullValueFilter),
    // Relation operations
    Some(()),
    Every(()),
    None(()),
}

// Keeping type for future, but not used by FieldOp right now
#[derive(Copy, Clone, Debug, PartialEq, Eq)]
pub enum JsonNullValueFilter {
    DbNull,
    JsonNull,
    AnyNull,
}

/// Trait for converting a model to a model with relations
pub trait FromModel<M> {
    fn from_model(model: M) -> Self;
}

/// Trait for merging values into an ActiveModel
pub trait MergeInto<AM> {
    fn merge_into(&self, model: &mut AM);
}

// Default implementation for unit type
impl<AM> MergeInto<AM> for () {
    fn merge_into(&self, _model: &mut AM) {
        // Unit type does nothing when merged
    }
}

/// Trait for relation filters that can be used with .with()
pub trait RelationFilterTrait: Clone {
    fn relation_name(&self) -> &'static str;
    fn filters(&self) -> &[Filter];
}

/// Generic filter structure that matches the generated Filter type
#[derive(Debug, Clone)]
pub struct Filter {
    pub field: String,
    pub operation: FieldOp<String>, // Type-safe operation instead of string value
}

/// Generic relation filter structure that matches the generated RelationFilter type
#[derive(Debug, Clone)]
pub struct RelationFilter {
    pub relation: &'static str,
    pub filters: Vec<Filter>,
    pub nested_select_aliases: Option<Vec<String>>,
    pub nested_includes: Vec<RelationFilter>,
    pub take: Option<i64>,
    pub skip: Option<i64>,
    pub order_by: Vec<(String, SortOrder)>,
    pub cursor_id: Option<i32>,
    pub include_count: bool,
    pub distinct: bool,
}

// IncludeArgs was an internal transitional type. It is now deprecated.
#[deprecated(since = "0.1.0", note = "Use relation::include(|rel| ...) instead")]
#[derive(Debug, Clone, Default)]
pub struct IncludeArgs {
    pub filters: Vec<Filter>,
    pub nested_select_aliases: Option<Vec<String>>,
    pub nested_includes: Vec<RelationFilter>,
    pub take: Option<i64>,
    pub skip: Option<i64>,
    pub order_by: Vec<(String, SortOrder)>,
    pub cursor_id: Option<i32>,
    pub include_count: bool,
}

#[allow(deprecated)]
impl IncludeArgs {
    pub fn with_filters(mut self, filters: Vec<Filter>) -> Self {
        self.filters = filters;
        self
    }
    pub fn push_filters(mut self, mut filters: Vec<Filter>) -> Self {
        self.filters.append(&mut filters);
        self
    }
    pub fn with_nested_includes(mut self, nested: Vec<RelationFilter>) -> Self {
        self.nested_includes = nested;
        self
    }
    pub fn take(mut self, n: i64) -> Self {
        self.take = Some(n);
        self
    }
    pub fn skip(mut self, n: i64) -> Self {
        self.skip = Some(n);
        self
    }
}

/// Central PCR-like include builder that accumulates generic include state
#[derive(Debug, Clone, Default)]
pub struct IncludeBuilderCore {
    pub filters: Vec<Filter>,
    pub nested_select_aliases: Option<Vec<String>>,
    pub nested_includes: Vec<RelationFilter>,
    pub take: Option<i64>,
    pub skip: Option<i64>,
    pub order_by: Vec<(String, SortOrder)>,
    pub cursor_id: Option<i32>,
    pub include_count: bool,
    pub distinct: bool,
}

impl IncludeBuilderCore {
    pub fn new() -> Self {
        Self::default()
    }
    pub fn push_filters(&mut self, filters: Vec<Filter>) {
        self.filters.extend(filters);
    }
    pub fn push_order_pairs(&mut self, pairs: Vec<(String, SortOrder)>) {
        self.order_by.extend(pairs);
    }
    pub fn set_select_aliases(&mut self, aliases: Vec<String>) {
        self.nested_select_aliases = Some(aliases);
    }
    pub fn with_nested(&mut self, include: RelationFilter) {
        self.nested_includes.push(include);
    }
    pub fn set_take(&mut self, n: i64) {
        self.take = Some(n);
    }
    pub fn set_skip(&mut self, n: i64) {
        self.skip = Some(n);
    }
    pub fn set_cursor_id(&mut self, id: i32) {
        self.cursor_id = Some(id);
    }
    pub fn enable_count(&mut self) {
        self.include_count = true;
    }
    pub fn enable_distinct(&mut self) {
        self.distinct = true;
    }
    pub fn build(self, relation: &'static str) -> RelationFilter {
        RelationFilter {
            relation,
            filters: self.filters,
            nested_select_aliases: self.nested_select_aliases,
            nested_includes: self.nested_includes,
            take: self.take,
            skip: self.skip,
            order_by: self.order_by,
            cursor_id: self.cursor_id,
            include_count: self.include_count,
            distinct: self.distinct,
        }
    }
}

impl RelationFilterTrait for RelationFilter {
    fn relation_name(&self) -> &'static str {
        self.relation
    }

    fn filters(&self) -> &[Filter] {
        &self.filters
    }
}

/// Advanced relation operations for filtering on relations
/// These follow the Prisma Client Rust pattern for relation filtering
#[derive(Debug, Clone)]
pub struct RelationCondition {
    pub relation_name: &'static str,
    pub operation: FieldOp<()>,
    pub filters: Vec<Filter>,
    pub foreign_key_column: Option<String>,
    pub current_table: Option<String>,
    pub relation_table: Option<String>,
}

impl RelationCondition {
    pub fn some(relation_name: &'static str, filters: Vec<Filter>) -> Self {
        Self {
            relation_name,
            operation: FieldOp::Some(()),
            filters,
            foreign_key_column: None,
            current_table: None,
            relation_table: None,
        }
    }

    pub fn every(relation_name: &'static str, filters: Vec<Filter>) -> Self {
        Self {
            relation_name,
            operation: FieldOp::Every(()),
            filters,
            foreign_key_column: None,
            current_table: None,
            relation_table: None,
        }
    }

    pub fn none(relation_name: &'static str, filters: Vec<Filter>) -> Self {
        Self {
            relation_name,
            operation: FieldOp::None(()),
            filters,
            foreign_key_column: None,
            current_table: None,
            relation_table: None,
        }
    }
}

/// Trait for dynamic relation fetching
pub trait RelationFetcher<C: sea_orm::ConnectionTrait, Selected> {
    fn fetch_relation_for_model<'a>(
        &'a self,
        conn: &'a C,
        selected: &'a mut Selected,
        relation_name: &'a str,
        filters: &'a [Filter],
    ) -> std::pin::Pin<Box<dyn std::future::Future<Output = Result<(), sea_orm::DbErr>> + Send + 'a>>;
}

// Provide a default no-op implementation for all types
impl<C: sea_orm::ConnectionTrait, Selected> RelationFetcher<C, Selected> for () {
    fn fetch_relation_for_model<'a>(
        &'a self,
        _conn: &'a C,
        _selected: &'a mut Selected,
        _relation_name: &'a str,
        _filters: &'a [Filter],
    ) -> std::pin::Pin<Box<dyn std::future::Future<Output = Result<(), sea_orm::DbErr>> + Send + 'a>>
    {
        Box::pin(async { Ok(()) })
    }
}

/// Descriptor for a relation, used for dynamic lookup
pub struct RelationDescriptor<Selected> {
    pub name: &'static str,
    // Function to set the relation field on the model
    pub set_field: fn(&mut Selected, Box<dyn Any + Send>),
    // Function to get the foreign key value from the model
    pub get_foreign_key: fn(&Selected) -> Option<i32>,
    // The target entity name for the relation
    pub target_entity: &'static str,
    // The foreign key column name
    pub foreign_key_column: &'static str,
    // The foreign key field name on the current model (rust field name)
    pub foreign_key_field_name: &'static str,
    // The target table name (for has_many set operations)
    pub target_table_name: &'static str,
    // The current entity's primary key column name
    pub current_primary_key_column: &'static str,
    // The current entity's primary key field name (rust field name)
    pub current_primary_key_field_name: &'static str,
    // The target entity's primary key column name
    pub target_primary_key_column: &'static str,
    // The target entity name extracted from "to" attribute (for runtime primary key resolution)
    pub target_entity_name: Option<&'static str>,
    // Whether the foreign key is nullable
    pub is_foreign_key_nullable: bool,
    // Whether this relation is has_many
    pub is_has_many: bool,
}

/// Trait implemented by per-entity Selected output structs generated by macros
pub trait EntitySelection: Sized {
    /// Fill a selection from a query row using aliased field names
    fn fill_from_row(row: &sea_orm::QueryResult, fields: &[&str]) -> Self;
    /// Set relation field value by relation name
    fn set_relation(&mut self, relation_name: &str, value: Box<dyn Any + Send>);
    /// Extract i32 scalar by rust field name if present (legacy helper)
    fn get_i32(&self, field_name: &str) -> Option<i32>;
    /// Extract field value as a database Value by rust field name, if present
    fn get_value_as_db_value(&self, _field_name: &str) -> Option<sea_orm::Value> {
        None
    }
    /// Map an alias/rust field name to a column expression for implicit selection
    fn column_for_alias(alias: &str) -> Option<sea_query::SimpleExpr>
    where
        Self: Sized,
    {
        let _ = alias;
        None
    }
}

/// Helper trait to extract primary key value generically from ModelWithRelations
pub trait ExtractPkValue {
    fn extract_pk_value(&self, pk_field_name: &str) -> Option<sea_orm::Value>;
}
// Default no-op; entity macro will implement on ModelWithRelations if needed
impl<T> ExtractPkValue for T {
    fn extract_pk_value(&self, _pk_field_name: &str) -> Option<sea_orm::Value> {
        None
    }
}

/// Trait implemented by per-entity Selected holder to construct from full model
pub trait BuildSelectedFromModel<Model>: Sized {
    fn from_model_selected(model: Model, allowed_fields: &[&str]) -> Self;
}

/// Trait for types that provide relation metadata
pub trait HasRelationMetadata<Selected> {
    fn relation_descriptors() -> &'static [RelationDescriptor<Selected>];
    fn get_relation_descriptor(name: &str) -> Option<&'static RelationDescriptor<Selected>> {
        Self::relation_descriptors()
            .iter()
            .find(|desc| desc.name == name)
    }
}

/// Trait for defensive field fetching - allows entities to specify which fields
/// should be automatically included for relation fetching
pub trait DefensiveFieldFetcher {
    /// Returns a list of field names that should be defensively fetched
    /// for the given relation. These fields will be automatically included
    /// in queries even if not explicitly selected.
    fn defensive_fields_for_relation(relation_name: &str) -> Vec<&'static str>;

    /// Returns all fields that should be defensively fetched for any relation
    fn all_defensive_fields() -> Vec<&'static str>;

    /// Checks if a field should be defensively fetched for a specific relation
    fn should_defensively_fetch(field_name: &str, relation_name: &str) -> bool {
        Self::defensive_fields_for_relation(relation_name).contains(&field_name)
    }
}

/// Compile-time selection spec produced by per-entity macros
pub trait SelectionSpec {
    /// The entity type this selection targets
    type Entity: sea_orm::EntityTrait;
    /// The output data type materialized by the selection
    type Data: EntitySelection + HasRelationMetadata<Self::Data> + Send + 'static;
    /// Return the list of scalar aliases (snake_case rust field names) to fetch
    fn collect_aliases(self) -> Vec<String>;
}

/// Concrete typed selection marker carrying aliases and output type info
pub struct TypedSelection<E: sea_orm::EntityTrait, D> {
    pub aliases: Vec<String>,
    pub _phantom: std::marker::PhantomData<(E, D)>,
}

impl<E, D> SelectionSpec for TypedSelection<E, D>
where
    E: sea_orm::EntityTrait,
    D: EntitySelection + HasRelationMetadata<D> + Send + 'static,
{
    type Entity = E;
    type Data = D;
    fn collect_aliases(self) -> Vec<String> {
        self.aliases
    }
}

/// Helper to construct a typed selection marker
pub fn typed_selection<E, D>(aliases: Vec<String>) -> TypedSelection<E, D>
where
    E: sea_orm::EntityTrait,
    D: EntitySelection + HasRelationMetadata<D> + Send + 'static,
{
    TypedSelection {
        aliases,
        _phantom: std::marker::PhantomData,
    }
}

/// Helper to construct a typed selection marker without placing types in generic args at callsite
pub fn typed_selection_from_values<E, D>(
    _e: fn() -> E,
    _d: fn() -> D,
    aliases: Vec<String>,
) -> TypedSelection<E, D>
where
    E: sea_orm::EntityTrait,
    D: EntitySelection + HasRelationMetadata<D> + Send + 'static,
{
    TypedSelection {
        aliases,
        _phantom: std::marker::PhantomData,
    }
}

// Macro helper to construct TypedSelection for a module path without exposing type paths in the callsite macro body
// legacy helper removed

/// Trait for dynamic entity fetching without hardcoding
pub trait EntityFetcher<C: sea_orm::ConnectionTrait> {
    /// Fetch entities by foreign key value (legacy - returns Box<dyn Any + Send>)
    fn fetch_by_foreign_key<'a>(
        &'a self,
        conn: &'a C,
        foreign_key_value: Option<i32>,
        foreign_key_column: &'a str,
        target_entity: &'a str,
        relation_name: &'a str,
        filter: &'a RelationFilter,
    ) -> std::pin::Pin<
        Box<
            dyn std::future::Future<Output = Result<Box<dyn Any + Send>, sea_orm::DbErr>>
                + Send
                + 'a,
        >,
    >;

    /// Fetch entities by foreign key value with selection (returns Selected types directly)
    fn fetch_by_foreign_key_with_selection<'a>(
        &'a self,
        conn: &'a C,
        foreign_key_value: Option<i32>,
        foreign_key_column: &'a str,
        target_entity: &'a str,
        relation_name: &'a str,
        filter: &'a RelationFilter,
    ) -> std::pin::Pin<
        Box<
            dyn std::future::Future<Output = Result<Box<dyn Any + Send>, sea_orm::DbErr>>
                + Send
                + 'a,
        >,
    >;
}

/// Registry for mapping entity names to their fetchers
pub trait EntityRegistry<C: sea_orm::ConnectionTrait> {
    /// Get the fetcher for a given entity name
    fn get_fetcher(&self, entity_name: &str) -> Option<&dyn EntityFetcher<C>>;
}

/// Trait for converting various typed order specifications into a (expr, order) pair
pub trait IntoOrderByExpr {
    fn into_order_by_expr(self) -> (sea_query::SimpleExpr, sea_orm::Order);
}

impl<Col> IntoOrderByExpr for (Col, sea_orm::Order)
where
    Col: sea_orm::IntoSimpleExpr,
{
    fn into_order_by_expr(self) -> (sea_query::SimpleExpr, sea_orm::Order) {
        (self.0.into_simple_expr(), self.1)
    }
}

/// Combined order spec that can optionally carry a NullsOrder hint
pub trait IntoOrderSpec {
    fn into_order_spec(self) -> (sea_query::SimpleExpr, sea_orm::Order, Option<NullsOrder>);
}

impl<T> IntoOrderSpec for T
where
    T: IntoOrderByExpr,
{
    fn into_order_spec(self) -> (sea_query::SimpleExpr, sea_orm::Order, Option<NullsOrder>) {
        let (expr, ord) = self.into_order_by_expr();
        (expr, ord, None)
    }
}

impl<L> IntoOrderSpec for (L, NullsOrder)
where
    L: IntoOrderByExpr,
{
    fn into_order_spec(self) -> (sea_query::SimpleExpr, sea_orm::Order, Option<NullsOrder>) {
        let (expr, ord) = self.0.into_order_by_expr();
        (expr, ord, Some(self.1))
    }
}

/// Trait for models capable of applying nested relation filters/includes
pub trait ApplyNestedIncludes<C: sea_orm::ConnectionTrait> {
    fn apply_relation_filter<'a>(
        &'a mut self,
        conn: &'a C,
        filter: &'a RelationFilter,
        registry: &'a (dyn EntityRegistry<C> + Sync),
    ) -> std::pin::Pin<Box<dyn std::future::Future<Output = Result<(), sea_orm::DbErr>> + Send + 'a>>;
}

/// Helper type for dynamic entity resolution
pub struct EntityResolver<C: sea_orm::ConnectionTrait> {
    pub registry: Box<dyn EntityRegistry<C> + Send + Sync>,
}

impl<C: sea_orm::ConnectionTrait> EntityResolver<C> {
    pub fn new(registry: Box<dyn EntityRegistry<C> + Send + Sync>) -> Self {
        Self { registry }
    }

    pub async fn resolve_entity(
        &self,
        conn: &C,
        foreign_key_value: Option<i32>,
        foreign_key_column: &str,
        target_entity: &str,
        relation_name: &str,
    ) -> Result<Box<dyn Any + Send>, sea_orm::DbErr> {
        if let Some(fetcher) = self.registry.get_fetcher(target_entity) {
            let dummy = RelationFilter {
                relation: "",
                filters: vec![],
                nested_select_aliases: None,
                nested_includes: vec![],
                take: None,
                skip: None,
                order_by: vec![],
                cursor_id: None,
                include_count: false,
                distinct: false,
            };
            fetcher
                .fetch_by_foreign_key_with_selection(
                    conn,
                    foreign_key_value,
                    foreign_key_column,
                    target_entity,
                    relation_name,
                    &dummy,
                )
                .await
        } else {
            Err(sea_orm::DbErr::Custom(format!(
                "No fetcher found for entity: {}",
                target_entity
            )))
        }
    }
}

/// Trait for batch containers that can hold multiple queries (like Prisma Client Rust)
pub trait BatchContainer<'a, C, Entity, ActiveModel, ModelWithRelations, T>
where
    C: sea_orm::ConnectionTrait,
    Entity: sea_orm::EntityTrait,
    ActiveModel:
        sea_orm::ActiveModelTrait<Entity = Entity> + sea_orm::ActiveModelBehavior + Send + 'static,
    ModelWithRelations: FromModel<<Entity as sea_orm::EntityTrait>::Model>,
    T: MergeInto<ActiveModel>,
{
    type ReturnType;
    fn into_queries(self) -> Vec<BatchQuery<'a, C, Entity, ActiveModel, ModelWithRelations, T>>;
    fn from_results(results: Vec<BatchResult<ModelWithRelations>>) -> Self::ReturnType;
}

/// Helper function to create batch queries
pub async fn batch<'a, C, Entity, ActiveModel, ModelWithRelations, T, Container>(
    queries: Container,
    conn: &'a C,
) -> Result<Container::ReturnType, sea_orm::DbErr>
where
    C: sea_orm::ConnectionTrait + sea_orm::TransactionTrait,
    Entity: sea_orm::EntityTrait,
    ActiveModel:
        sea_orm::ActiveModelTrait<Entity = Entity> + sea_orm::ActiveModelBehavior + Send + 'static,
    ModelWithRelations: FromModel<<Entity as sea_orm::EntityTrait>::Model>,
    T: MergeInto<ActiveModel>,
    Container: BatchContainer<'a, C, Entity, ActiveModel, ModelWithRelations, T>,
    <Entity as sea_orm::EntityTrait>::Model: sea_orm::IntoActiveModel<ActiveModel>,
{
    let txn = conn.begin().await?;
    let batch_queries = Container::into_queries(queries);
    let mut results = Vec::with_capacity(batch_queries.len());

    for query in batch_queries {
        let res = match query {
            BatchQuery::Insert(q) => {
                let model = q.model;
                let result = model.insert(&txn).await.map(FromModel::from_model)?;
                BatchResult::Insert(result)
            }
            BatchQuery::Update(q) => {
                let result = q.exec_in_txn(&txn).await?;
                BatchResult::Update(result)
            }
            BatchQuery::Delete(q) => {
                let result = q.exec_in_txn(&txn).await?;
                BatchResult::Delete(result)
            }
            BatchQuery::Upsert(q) => {
                let result = q.exec_in_txn(&txn).await?;
                BatchResult::Upsert(result)
            }
        };
        results.push(res);
    }

    txn.commit().await?;
    Ok(Container::from_results(results))
}

// Implementation for Vec
impl<'a, C, Entity, ActiveModel, ModelWithRelations, T>
    BatchContainer<'a, C, Entity, ActiveModel, ModelWithRelations, T>
    for Vec<BatchQuery<'a, C, Entity, ActiveModel, ModelWithRelations, T>>
where
    C: sea_orm::ConnectionTrait,
    Entity: sea_orm::EntityTrait,
    ActiveModel:
        sea_orm::ActiveModelTrait<Entity = Entity> + sea_orm::ActiveModelBehavior + Send + 'static,
    ModelWithRelations: FromModel<<Entity as sea_orm::EntityTrait>::Model>,
    T: MergeInto<ActiveModel>,
{
    type ReturnType = Vec<BatchResult<ModelWithRelations>>;

    fn into_queries(self) -> Vec<BatchQuery<'a, C, Entity, ActiveModel, ModelWithRelations, T>> {
        self
    }

    fn from_results(results: Vec<BatchResult<ModelWithRelations>>) -> Self::ReturnType {
        results
    }
}

// Generic element trait to unify tuple impls up to arity 16
pub trait BatchElement<'a, C, Entity, ActiveModel, ModelWithRelations, T>
where
    C: sea_orm::ConnectionTrait,
    Entity: sea_orm::EntityTrait,
    ActiveModel:
        sea_orm::ActiveModelTrait<Entity = Entity> + sea_orm::ActiveModelBehavior + Send + 'static,
    ModelWithRelations: FromModel<<Entity as sea_orm::EntityTrait>::Model>,
    T: MergeInto<ActiveModel>,
{
    type Output;
    fn into_query(self) -> BatchQuery<'a, C, Entity, ActiveModel, ModelWithRelations, T>;
    fn extract_output(result: BatchResult<ModelWithRelations>) -> Self::Output;
}

impl<'a, C, Entity, ActiveModel, ModelWithRelations, T>
    BatchElement<'a, C, Entity, ActiveModel, ModelWithRelations, T>
    for crate::query_builders::HasManySetUpdateQueryBuilder<
        'a,
        C,
        Entity,
        ActiveModel,
        ModelWithRelations,
        T,
    >
where
    C: sea_orm::ConnectionTrait + sea_orm::TransactionTrait,
    Entity: sea_orm::EntityTrait,
    ActiveModel:
        sea_orm::ActiveModelTrait<Entity = Entity> + sea_orm::ActiveModelBehavior + Send + 'static,
    ModelWithRelations: FromModel<<Entity as sea_orm::EntityTrait>::Model>,
    T: MergeInto<ActiveModel> + std::fmt::Debug + crate::types::SetParamInfo,
    <Entity as sea_orm::EntityTrait>::Model: sea_orm::IntoActiveModel<ActiveModel>,
{
    type Output = ModelWithRelations;
    fn into_query(self) -> BatchQuery<'a, C, Entity, ActiveModel, ModelWithRelations, T> {
        unreachable!()
    }
    fn extract_output(result: BatchResult<ModelWithRelations>) -> Self::Output {
        match result {
            BatchResult::Update(m) => m,
            // HasManySet ultimately returns Update result
            _ => panic!("Expected Update"),
        }
    }
}

impl<'a, C, Entity, ActiveModel, ModelWithRelations, T>
    BatchElement<'a, C, Entity, ActiveModel, ModelWithRelations, T>
    for crate::query_builders::UpsertQueryBuilder<'a, C, Entity, ActiveModel, ModelWithRelations, T>
where
    C: sea_orm::ConnectionTrait,
    Entity: sea_orm::EntityTrait,
    ActiveModel:
        sea_orm::ActiveModelTrait<Entity = Entity> + sea_orm::ActiveModelBehavior + Send + 'static,
    ModelWithRelations: FromModel<<Entity as sea_orm::EntityTrait>::Model>,
    T: MergeInto<ActiveModel>,
    <Entity as sea_orm::EntityTrait>::Model: sea_orm::IntoActiveModel<ActiveModel>,
{
    type Output = ModelWithRelations;
    fn into_query(self) -> BatchQuery<'a, C, Entity, ActiveModel, ModelWithRelations, T> {
        BatchQuery::Upsert(self)
    }
    fn extract_output(result: BatchResult<ModelWithRelations>) -> Self::Output {
        match result {
            BatchResult::Upsert(m) => m,
            _ => panic!("Expected Upsert"),
        }
    }
}

impl<'a, C, Entity, ActiveModel, ModelWithRelations>
    BatchElement<'a, C, Entity, ActiveModel, ModelWithRelations, ()>
    for crate::query_builders::CreateQueryBuilder<'a, C, Entity, ActiveModel, ModelWithRelations>
where
    C: sea_orm::ConnectionTrait,
    Entity: sea_orm::EntityTrait,
    ActiveModel:
        sea_orm::ActiveModelTrait<Entity = Entity> + sea_orm::ActiveModelBehavior + Send + 'static,
    ModelWithRelations: FromModel<<Entity as sea_orm::EntityTrait>::Model>,
{
    type Output = ModelWithRelations;
    fn into_query(self) -> BatchQuery<'a, C, Entity, ActiveModel, ModelWithRelations, ()> {
        BatchQuery::Insert(self)
    }
    fn extract_output(result: BatchResult<ModelWithRelations>) -> Self::Output {
        match result {
            BatchResult::Insert(m) => m,
            _ => panic!("Expected Insert"),
        }
    }
}

impl<'a, C, Entity, ActiveModel, ModelWithRelations>
    BatchElement<'a, C, Entity, ActiveModel, ModelWithRelations, ()>
    for crate::query_builders::DeleteQueryBuilder<'a, C, Entity, ModelWithRelations>
where
    C: sea_orm::ConnectionTrait,
    Entity: sea_orm::EntityTrait,
    ActiveModel:
        sea_orm::ActiveModelTrait<Entity = Entity> + sea_orm::ActiveModelBehavior + Send + 'static,
    ModelWithRelations: FromModel<<Entity as sea_orm::EntityTrait>::Model>,
{
    type Output = ModelWithRelations;
    fn into_query(self) -> BatchQuery<'a, C, Entity, ActiveModel, ModelWithRelations, ()> {
        BatchQuery::Delete(self)
    }
    fn extract_output(result: BatchResult<ModelWithRelations>) -> Self::Output {
        match result {
            BatchResult::Delete(m) => m,
            _ => panic!("Expected Delete"),
        }
    }
}

macro_rules! impl_tuple_batch_container {
    ( $( $name:ident ),+ ) => {
        impl<'a, Conn, Entity, ActiveModel, ModelWithRelations, T, $( $name ),+>
            BatchContainer<'a, Conn, Entity, ActiveModel, ModelWithRelations, T> for ( $( $name ),+ , )
        where
            Conn: sea_orm::ConnectionTrait + sea_orm::TransactionTrait,
            Entity: sea_orm::EntityTrait,
            ActiveModel: sea_orm::ActiveModelTrait<Entity = Entity>
                + sea_orm::ActiveModelBehavior
                + Send
                + 'static,
            ModelWithRelations: FromModel<<Entity as sea_orm::EntityTrait>::Model>,
            T: MergeInto<ActiveModel>,
            $( $name: BatchElement<'a, Conn, Entity, ActiveModel, ModelWithRelations, T> ),+
        {
            type ReturnType = ( $( <$name as BatchElement<'a, Conn, Entity, ActiveModel, ModelWithRelations, T>>::Output ),+ , );
            fn into_queries(self) -> Vec<BatchQuery<'a, Conn, Entity, ActiveModel, ModelWithRelations, T>> {
                let ( $( $name ),+ , ) = self;
                vec![ $( $name.into_query() ),+ ]
            }
            fn from_results(mut results: Vec<BatchResult<ModelWithRelations>>) -> Self::ReturnType {
                (
                    $(
                        {
                            let tmp = results.remove(0);
                            <$name as BatchElement<'a, Conn, Entity, ActiveModel, ModelWithRelations, T>>::extract_output(tmp)
                        }
                    ),+ ,
                )
            }
        }
    };
}

/// Trait for SetParam types to enable proper pattern matching without string parsing
pub trait SetParamInfo {
    /// Check if this is a has_many set operation
    fn is_has_many_set_operation(&self) -> bool;

    /// Extract the relation name from a has_many set operation
    fn extract_relation_name(&self) -> Option<&'static str>;

    /// Extract target IDs from a has_many set operation
    fn extract_target_ids(&self) -> Vec<sea_orm::Value>;

    /// Check if this is a has_many nested create operation (create/createMany)
    fn is_has_many_create_operation(&self) -> bool;

    /// Execute nested create items on a connection for a given parent id
    fn exec_has_many_create_on_conn<'a>(
        &'a self,
        conn: &'a DatabaseConnection,
        parent_id: i32,
    ) -> std::pin::Pin<Box<dyn std::future::Future<Output = Result<(), sea_orm::DbErr>> + Send + 'a>>;

    /// Execute nested create items in a transaction for a given parent id
    fn exec_has_many_create_on_txn<'a>(
        &'a self,
        txn: &'a DatabaseTransaction,
        parent_id: i32,
    ) -> std::pin::Pin<Box<dyn std::future::Future<Output = Result<(), sea_orm::DbErr>> + Send + 'a>>;
}

/// Trait for condition types to enable proper ID extraction without string parsing
pub trait ConditionInfo {
    /// Extract the entity ID from a condition
    fn extract_entity_id(&self) -> Option<sea_orm::Value>;
}

impl ConditionInfo for sea_orm::Condition {
    fn extract_entity_id(&self) -> Option<sea_orm::Value> {
        // Best-effort parsing of debug string until a typed API is available
        let condition_str = format!("{:?}", self);

        fn try_extract_id_pattern(
            condition_str: &str,
            pattern: &str,
            pattern_len: usize,
        ) -> Option<i32> {
            condition_str.find(pattern).and_then(|id_start| {
                let after_pattern = &condition_str[id_start + pattern_len..];
                let id_end = after_pattern
                    .find(')')
                    .or_else(|| after_pattern.find(' '))?;
                let id_str = &after_pattern[..id_end];
                id_str.parse::<i32>().ok()
            })
        }

        let extracted_id = try_extract_id_pattern(&condition_str, "Id = ", 5)
            .or_else(|| try_extract_id_pattern(&condition_str, "Value(Int(Some(", 15))
            .or_else(|| try_extract_id_pattern(&condition_str, "IdEquals(", 9))
            .or_else(|| try_extract_id_pattern(&condition_str, "Equal, Value(Int(Some(", 22))
            .or_else(|| try_extract_id_pattern(&condition_str, "Value(Int(Some(", 15))
            .or_else(|| try_extract_id_pattern(&condition_str, " = ", 3));

        extracted_id.map(|id| sea_orm::Value::Int(Some(id)))
    }
}

// (Old per-operation tuple macros were removed; unified macro below is used instead.)

// Generate tuple impls up to arity 16

impl_tuple_batch_container!(a);

impl_tuple_batch_container!(a, b);

impl_tuple_batch_container!(a, b, c);

impl_tuple_batch_container!(a, b, c, d);

impl_tuple_batch_container!(a, b, c, d, e);

impl_tuple_batch_container!(a, b, c, d, e, f);

impl_tuple_batch_container!(a, b, c, d, e, f, g);

impl_tuple_batch_container!(a, b, c, d, e, f, g, h);

impl_tuple_batch_container!(a, b, c, d, e, f, g, h, i);

impl_tuple_batch_container!(a, b, c, d, e, f, g, h, i, j);

impl_tuple_batch_container!(a, b, c, d, e, f, g, h, i, j, k);

impl_tuple_batch_container!(a, b, c, d, e, f, g, h, i, j, k, l);

impl_tuple_batch_container!(a, b, c, d, e, f, g, h, i, j, k, l, m);

impl_tuple_batch_container!(a, b, c, d, e, f, g, h, i, j, k, l, m, n);

impl_tuple_batch_container!(a, b, c, d, e, f, g, h, i, j, k, l, m, n, o);

impl_tuple_batch_container!(a, b, c, d, e, f, g, h, i, j, k, l, m, n, o, p);
