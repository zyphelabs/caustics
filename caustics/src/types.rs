#![allow(non_camel_case_types)]

use std::any::Any;

pub type QueryError = sea_orm::DbErr;
// Crate-wide result alias for ergonomics (non-conflicting)
pub type CausticsResult<T> = std::result::Result<T, sea_orm::DbErr>;

/// Typed Caustics errors that can be converted into `sea_orm::DbErr`
#[derive(Debug, Clone)]
pub enum CausticsError {
    RelationNotFound { relation: String },
    EntityFetcherMissing { entity: String },
    DeferredLookupFailed { target: String, detail: String },
}

impl core::fmt::Display for CausticsError {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        match self {
            CausticsError::RelationNotFound { relation } => {
                write!(f, "CausticsError::RelationNotFound: relation='{}'", relation)
            }
            CausticsError::EntityFetcherMissing { entity } => {
                write!(f, "CausticsError::EntityFetcherMissing: entity='{}'", entity)
            }
            CausticsError::DeferredLookupFailed { target, detail } => {
                write!(f, "CausticsError::DeferredLookupFailed: target='{}' detail='{}'", target, detail)
            }
        }
    }
}

impl From<CausticsError> for sea_orm::DbErr {
    fn from(err: CausticsError) -> Self {
        sea_orm::DbErr::Custom(err.to_string())
    }
}

// Import query builder types for batch operations
use crate::query_builders::{BatchQuery, BatchResult};

#[derive(Copy, Clone, Debug)]
pub enum SortOrder {
    Asc,
    Desc,
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
    // Relation operations
    Some(()),
    Every(()),
    None(()),
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
pub trait RelationFetcher<C: sea_orm::ConnectionTrait, ModelWithRelations> {
    fn fetch_relation_for_model<'a>(
        &'a self,
        conn: &'a C,
        model_with_relations: &'a mut ModelWithRelations,
        relation_name: &'a str,
        filters: &'a [Filter],
    ) -> std::pin::Pin<Box<dyn std::future::Future<Output = Result<(), sea_orm::DbErr>> + Send + 'a>>;
}

// Provide a default no-op implementation for all types
impl<C: sea_orm::ConnectionTrait, ModelWithRelations> RelationFetcher<C, ModelWithRelations>
    for ()
{
    fn fetch_relation_for_model<'a>(
        &'a self,
        _conn: &'a C,
        _model_with_relations: &'a mut ModelWithRelations,
        _relation_name: &'a str,
        _filters: &'a [Filter],
    ) -> std::pin::Pin<Box<dyn std::future::Future<Output = Result<(), sea_orm::DbErr>> + Send + 'a>> {
        Box::pin(async { Ok(()) })
    }
}

/// Descriptor for a relation, used for dynamic lookup
pub struct RelationDescriptor<ModelWithRelations> {
    pub name: &'static str,
    // Function to set the relation field on the model
    pub set_field: fn(&mut ModelWithRelations, Box<dyn Any + Send>),
    // Function to get the foreign key value from the model
    pub get_foreign_key: fn(&ModelWithRelations) -> Option<i32>,
    // The target entity name for the relation
    pub target_entity: &'static str,
    // The foreign key column name
    pub foreign_key_column: &'static str,
    // The target table name (for has_many set operations)
    pub target_table_name: &'static str,
    // The current entity's primary key column name
    pub current_primary_key_column: &'static str,
    // The target entity's primary key column name
    pub target_primary_key_column: &'static str,
    // Whether the foreign key is nullable
    pub is_foreign_key_nullable: bool,
}

/// Trait for types that provide relation metadata
pub trait HasRelationMetadata<ModelWithRelations> {
    fn relation_descriptors() -> &'static [RelationDescriptor<ModelWithRelations>];
    fn get_relation_descriptor(
        name: &str,
    ) -> Option<&'static RelationDescriptor<ModelWithRelations>> {
        Self::relation_descriptors()
            .iter()
            .find(|desc| desc.name == name)
    }
}

/// Trait for dynamic entity fetching without hardcoding
pub trait EntityFetcher<C: sea_orm::ConnectionTrait> {
    /// Fetch entities by foreign key value
    fn fetch_by_foreign_key<'a>(
        &'a self,
        conn: &'a C,
        foreign_key_value: Option<i32>,
        foreign_key_column: &'a str,
        target_entity: &'a str,
        relation_name: &'a str,
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
            fetcher
                .fetch_by_foreign_key(
                    conn,
                    foreign_key_value,
                    foreign_key_column,
                    target_entity,
                    relation_name,
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
    ActiveModel: sea_orm::ActiveModelTrait<Entity = Entity> + sea_orm::ActiveModelBehavior + Send + 'static,
    ModelWithRelations: FromModel<<Entity as sea_orm::EntityTrait>::Model>,
    T: MergeInto<ActiveModel>,
{
    type Output;
    fn into_query(self) -> BatchQuery<'a, C, Entity, ActiveModel, ModelWithRelations, T>;
    fn extract_output(result: BatchResult<ModelWithRelations>) -> Self::Output;
}

impl<'a, C, Entity, ActiveModel, ModelWithRelations, T>
    BatchElement<'a, C, Entity, ActiveModel, ModelWithRelations, T>
    for crate::query_builders::UpdateQueryBuilder<'a, C, Entity, ActiveModel, ModelWithRelations, T>
where
    C: sea_orm::ConnectionTrait,
    Entity: sea_orm::EntityTrait,
    ActiveModel: sea_orm::ActiveModelTrait<Entity = Entity> + sea_orm::ActiveModelBehavior + Send + 'static,
    ModelWithRelations: FromModel<<Entity as sea_orm::EntityTrait>::Model>,
    T: MergeInto<ActiveModel>,
    <Entity as sea_orm::EntityTrait>::Model: sea_orm::IntoActiveModel<ActiveModel>,
{
    type Output = ModelWithRelations;
    fn into_query(self) -> BatchQuery<'a, C, Entity, ActiveModel, ModelWithRelations, T> {
        BatchQuery::Update(self)
    }
    fn extract_output(result: BatchResult<ModelWithRelations>) -> Self::Output {
        match result {
            BatchResult::Update(m) => m,
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
    ActiveModel: sea_orm::ActiveModelTrait<Entity = Entity> + sea_orm::ActiveModelBehavior + Send + 'static,
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
    ActiveModel: sea_orm::ActiveModelTrait<Entity = Entity> + sea_orm::ActiveModelBehavior + Send + 'static,
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
    ActiveModel: sea_orm::ActiveModelTrait<Entity = Entity> + sea_orm::ActiveModelBehavior + Send + 'static,
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

        fn try_extract_id_pattern(condition_str: &str, pattern: &str, pattern_len: usize) -> Option<i32> {
            condition_str.find(pattern).and_then(|id_start| {
                let after_pattern = &condition_str[id_start + pattern_len..];
                let id_end = after_pattern.find(')').or_else(|| after_pattern.find(' '))?;
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
