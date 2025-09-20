# Caustics Implementation Todo List

## High Priority

### 1. String Operators
- [x] `contains` operator for string fields
- [x] `starts_with` operator for string fields  
- [x] `ends_with` operator for string fields
- [x] Case-insensitive search mode (see QueryMode below)
- [x] Update macro to generate field-specific string operator variants
- [x] Add string operators to nullable string fields

### 2. Comparison Operators
- [x] `lt` (less than) for all comparable types
- [x] `lte` (less than or equal) for all comparable types
- [x] `gt` (greater than) for all comparable types
- [x] `gte` (greater than or equal) for all comparable types
- [x] Ensure operators work with nullable fields
- [x] Add operators for DateTime, Int, String types

### 3. Collection Operators
- [x] `in_vec` operator for all types
- [x] `not_in_vec` operator for all types
- [x] Support for Vec<Int>, Vec<String>, Vec<DateTime>
- [x] Handle nullable field collections properly

### 4. Null Operators
- [x] `is_null` operator for nullable fields
- [x] `is_not_null` operator for nullable fields
- [x] Update macro to generate null-specific variants
- [x] Ensure proper type safety for null operations

### 5. Logical Operators
- [x] `AND` operator for combining conditions
- [x] `OR` operator for combining conditions
- [x] `NOT` operator for negating conditions
- [x] Complex nested logical expressions
- [x] Update WhereParam enum to support logical operators

## Medium Priority

### 6. JSON Field Support
- [x] `JsonNullableFilter` enum with all operators
- [x] `path` operator for JSON field access
- [x] `string_contains`, `string_starts_with`, `string_ends_with` for JSON
- [x] `array_contains`, `array_starts_with`, `array_ends_with` for JSON arrays
- [x] `object_contains` for JSON objects
- [x] `lt`, `lte`, `gt`, `gte`, `not` for JSON values
- [x] JSON field type detection in macro

### 7. Database Compatibility
- [x] PostgreSQL support with native JSON operators (`@>`, `#>`, `?`, `ILIKE`)
- [x] MySQL support with JSON functions (`JSON_EXTRACT()`, `JSON_CONTAINS()`)
- [x] MariaDB support with JSON functions (`JSON_VALUE()`, `JSON_CONTAINS()`)
- [x] SQLite support with JSON1 extension (`json_extract()`, `json_each()`)
- [x] Automatic database detection
- [x] Database-agnostic string operations with case sensitivity
- [x] Comprehensive test coverage across all database types
- [x] Migration support between databases without code changes

### 8. Atomic Operations
- [x] `increment` operation for numeric fields
- [x] `decrement` operation for numeric fields
- [x] `multiply` operation for numeric fields
- [x] `divide` operation for numeric fields
- [x] Support for both nullable and non-nullable numeric fields
- [x] Update SetParam enum to include atomic operations

### 9. Advanced Relation Operations (Reads)
- [x] `some` operator for relation filtering
- [x] `every` operator for relation filtering
- [x] `none` operator for relation filtering
- [x] `connect` operation for relations
- [x] `disconnect` operation for relations
- [x] `set` operation for relations
- [x] Nested has_many create/createMany on create
- [x] Nested has_many create/createMany on update (atomic with scalar update)
- [x] Nested relation queries: per-relation filtering, orderBy (any column), take/skip, cursor, distinct
- [x] Nested relation queries: deep nested trees (multi-level)
- [x] Relation-aggregate orderBy (_count) with nested sugar
- [x] Upsert create-branch id extractor (macro-supplied id_extractor, reliable post-insert ops)

### 10. Raw SQL Capabilities
- [ ] `_query_raw` method for raw SQL queries
- [ ] `_execute_raw` method for raw SQL execution
- [ ] Type-safe raw query results
- [ ] Raw query builder interface

### 11. Enhanced Batch Operations
- [x] createMany (multi-insert via client.user().create_many([...]).exec())
- [x] updateMany (multi-update via client.user().update_many(where, changes).exec())
- [x] Batch deletes (DeleteQueryBuilder in batch)
- [x] Batch upserts (UpsertQueryBuilder in batch)
- [x] Optimized batch execution (single transaction)

## Low Priority

### 12. Query Modes and Ordering
- [x] `QueryMode` enum with `Default` and `Insensitive`
- [x] `NullsOrder` enum with `First` and `Last`
- [x] `JsonNullValueFilter` with `DbNull`, `JsonNull`, `AnyNull` (PCR parity)
- [ ] Advanced ordering options

### 13. Advanced Type System
- [x] `ScalarFieldEnum` for each model
- [ ] `RecursiveSafeType` for preventing infinite recursion
- [ ] `PartialUnchecked` for partial updates
- [ ] Enhanced type safety features

### 14. Comprehensive Error Handling
- [ ] `RelationNotFetchedError` type
- [ ] `NewClientError` type
- [ ] Specific error types for different operations
- [ ] Better error messages and context

### 15. Advanced Query Building
- [ ] `ManyArgs` with complex parameter structures
- [ ] `UniqueArgs` with advanced options
- [ ] `OrderByWithRelationParam` for relation ordering
- [ ] `OrderByRelationAggregateParam` for aggregate ordering

### 16. Data Model Integration
- [ ] Schema introspection capabilities
- [ ] `DATAMODEL_STR` for schema awareness
- [ ] `DATABASE_STR` for database type detection
- [ ] Automatic schema validation

## Implementation Notes

### Macro Updates Required
- [x] Update `caustics-macros/src/entity.rs` to generate field-specific operator variants
- [x] Add support for new filter types in WhereParam enum
- [x] Generate atomic operation variants in SetParam enum
- [x] Add JSON field detection and handling
- [x] Update relation generation to support advanced operations

### Type System Updates
- [x] Add new filter types to `caustics/src/types.rs` (generic `FieldOp`, `RelationCondition`)
- [x] Create JSON-specific filter types (operators via existing enums)
- [x] Add atomic operation types (via macro-generated SetParam variants)
- [ ] Enhance error type system

### Query Builder Updates
- [x] Operator support integrated in builders (macros + per-builder handling)
- [ ] Add raw SQL query builders
- [ ] Enhance batch operation support
- [ ] Add relation-specific query builders

### Testing Requirements
- [x] Unit/integration test for case-insensitive string search (see school_test.rs)
- [x] Unit tests for string operators (contains, starts_with, ends_with)
- [x] Unit tests for comparison operators (gt, lt, gte, lte)
- [x] Unit tests for collection operators (in_vec, not_in_vec) - includes README examples in test_collection_operators_readme_examples
- [x] Unit tests for logical operators (and, or, not) - see test_logical_operators in blog_test.rs
- [x] Unit tests for null operators
- [x] Integration test: update with nested has_many create + scalar set (atomic)
- [ ] Performance tests for batch operations
- [ ] Error handling tests

## Current Status

### Already Implemented
- [x] Basic CRUD operations
- [x] Simple relation fetching
- [x] **Advanced relation filtering** (`some`, `every`, `none` with EXISTS/NOT EXISTS subqueries)
- [x] Basic transaction support
- [x] Simple batch operations (insert only)
- [x] Basic filtering with `equals`
- [x] Basic ordering
- [x] Pagination (take/skip)
- [x] String operators (including case-insensitive mode)
- [x] Comparison operators (lt, lte, gt, gte, not_equals)
- [x] Collection operators (in_vec, not_in_vec)
- [x] Logical operators (and, or, not)

### PCR Parity: What’s Done vs Remaining

Completed parity
- [x] Select on find_many/find_unique/find_first returns per-entity `Selected` holder (only requested scalars populated)
- [x] Include on select-builders (Many/First/Unique) with implicit key auto-append
- [x] Include on full-model builders (Many/First/Unique)
- [x] Cursor pagination, orderBy, take/skip semantics (negative take flips order)
- [x] Aggregate and groupBy typed enums and helpers
 - [x] Per-entity `select!` macro on nightly (entity::select!(field_a, field_b)) with typed selection end-to-end
- [x] Field selection optimization wired to SQL (only requested columns fetched)
- [x] Relation-level orderBy/take/skip/cursor/distinct inside include closures
- [x] Distinct emulation via GROUP BY for specific fields (cross-backend)

Remaining gaps
- [x] Nested select/include trees (selecting scalars on included relations; nested includes)
- [x] Relation-level orderBy/take/skip inside include args (e.g., include: { posts: { take: 5, orderBy: ... } })
- [x] Native DISTINCT ON for Postgres (wired via SeaQuery distinct_on with typed columns)
- [ ] Error surface parity (typed errors like RelationNotFetched, QueryValidation) and error messages
- [ ] Raw SQL APIs (`_queryRaw`, `_executeRaw`) and result typing
- [x] Full batch API parity (batch update/delete/upsert builders consolidated)
- [ ] Middlewares/hooks and transaction API parity with PCR
 - [x] JSON null handling flags (DbNull/JsonNull/AnyNull) with `FieldOp::JsonNull`
- [ ] Schema introspection exposure (DATAMODEL_STR/DATABASE_STR) and validation helpers
- [x] Advanced ordering options (NullsOrder First/Last, relation aggregates in orderBy)

## Toolchain

- [x] Pin nightly toolchain in `rust-toolchain.toml` (nightly-2025-08-31)
- [x] Enable `#![feature(decl_macro)]` where needed for per-entity `pub macro` support

## API changes

- [x] Remove legacy global `Select!` macro and `Vec<ScalarField>` selects
- [x] Remove global `select_typed!` macro from public API; use per-entity `entity::select!(...)`
