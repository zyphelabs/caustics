# Caustics Implementation Todo List

## High Priority Features (Core Functionality)

### 1. String Operators
- [x] `contains` operator for string fields
- [x] `starts_with` operator for string fields  
- [x] `ends_with` operator for string fields
- [ ] Case-insensitive search mode
- [x] Update macro to generate field-specific string operator variants
- [x] Add string operators to nullable string fields

### 2. Comparison Operators
- [ ] `lt` (less than) for all comparable types
- [ ] `lte` (less than or equal) for all comparable types
- [ ] `gt` (greater than) for all comparable types
- [ ] `gte` (greater than or equal) for all comparable types
- [ ] Ensure operators work with nullable fields
- [ ] Add operators for DateTime, Int, String types

### 3. Collection Operators
- [ ] `in_vec` operator for all types
- [ ] `not_in_vec` operator for all types
- [ ] Support for Vec<Int>, Vec<String>, Vec<DateTime>
- [ ] Handle nullable field collections properly

### 4. Null Operators
- [ ] `is_null` operator for nullable fields
- [ ] `is_not_null` operator for nullable fields
- [ ] Update macro to generate null-specific variants
- [ ] Ensure proper type safety for null operations

### 5. Logical Operators
- [ ] `AND` operator for combining conditions
- [ ] `OR` operator for combining conditions
- [ ] `NOT` operator for negating conditions
- [ ] Complex nested logical expressions
- [ ] Update WhereParam enum to support logical operators

## Medium Priority Features (Advanced Functionality)

### 6. JSON Field Support
- [ ] `JsonNullableFilter` enum with all operators
- [ ] `path` operator for JSON field access
- [ ] `string_contains`, `string_starts_with`, `string_ends_with` for JSON
- [ ] `array_contains`, `array_starts_with`, `array_ends_with` for JSON arrays
- [ ] `lt`, `lte`, `gt`, `gte`, `not` for JSON values
- [ ] JSON field type detection in macro

### 7. Atomic Operations
- [ ] `increment` operation for numeric fields
- [ ] `decrement` operation for numeric fields
- [ ] `multiply` operation for numeric fields
- [ ] `divide` operation for numeric fields
- [ ] Support for both nullable and non-nullable numeric fields
- [ ] Update SetParam enum to include atomic operations

### 8. Advanced Relation Operations
- [ ] `some` operator for relation filtering
- [ ] `every` operator for relation filtering
- [ ] `none` operator for relation filtering
- [ ] `connect` operation for relations
- [ ] `disconnect` operation for relations
- [ ] `set` operation for relations
- [ ] Nested relation queries

### 9. Raw SQL Capabilities
- [ ] `_query_raw` method for raw SQL queries
- [ ] `_execute_raw` method for raw SQL execution
- [ ] Type-safe raw query results
- [ ] Raw query builder interface

### 10. Enhanced Batch Operations
- [ ] Complete batch operation support
- [ ] Batch updates (currently missing)
- [ ] Batch deletes (currently missing)
- [ ] Batch upserts (currently missing)
- [ ] Optimized batch execution

## Low Priority Features (Nice to Have)

### 11. Query Modes and Ordering
- [ ] `QueryMode` enum with `Default` and `Insensitive`
- [ ] `NullsOrder` enum with `First` and `Last`
- [ ] `JsonNullValueFilter` with `DbNull`, `JsonNull`, `AnyNull`
- [ ] Advanced ordering options

### 12. Advanced Type System
- [ ] `ScalarFieldEnum` for each model
- [ ] `RecursiveSafeType` for preventing infinite recursion
- [ ] `PartialUnchecked` for partial updates
- [ ] Enhanced type safety features

### 13. Comprehensive Error Handling
- [ ] `RelationNotFetchedError` type
- [ ] `NewClientError` type
- [ ] Specific error types for different operations
- [ ] Better error messages and context

### 14. Advanced Query Building
- [ ] `ManyArgs` with complex parameter structures
- [ ] `UniqueArgs` with advanced options
- [ ] `OrderByWithRelationParam` for relation ordering
- [ ] `OrderByRelationAggregateParam` for aggregate ordering

### 15. Data Model Integration
- [ ] Schema introspection capabilities
- [ ] `DATAMODEL_STR` for schema awareness
- [ ] `DATABASE_STR` for database type detection
- [ ] Automatic schema validation

## Implementation Notes

### Macro Updates Required
- [ ] Update `caustics-macros/src/entity.rs` to generate field-specific operator variants
- [ ] Add support for new filter types in WhereParam enum
- [ ] Generate atomic operation variants in SetParam enum
- [ ] Add JSON field detection and handling
- [ ] Update relation generation to support advanced operations

### Type System Updates
- [ ] Add new filter types to `caustics/src/types.rs`
- [ ] Create JSON-specific filter types
- [ ] Add atomic operation types
- [ ] Enhance error type system

### Query Builder Updates
- [ ] Update `caustics/src/query_builders.rs` for new operators
- [ ] Add raw SQL query builders
- [ ] Enhance batch operation support
- [ ] Add relation-specific query builders

### Testing Requirements
- [ ] Unit tests for each new operator
- [ ] Integration tests for complex queries
- [ ] Performance tests for batch operations
- [ ] Error handling tests

## Current Status

### Already Implemented
- [x] Basic CRUD operations
- [x] Simple relation fetching
- [x] Basic transaction support
- [x] Simple batch operations (insert only)
- [x] Basic filtering with `equals`
- [x] Basic ordering
- [x] Pagination (take/skip)

### In Progress
- [ ] String operators (partially implemented)
- [ ] Comparison operators (partially implemented)

### Not Started
- [ ] JSON field support
- [ ] Atomic operations
- [ ] Advanced relation operations
- [ ] Raw SQL capabilities
- [ ] Most advanced features

## Estimated Timeline

- **High Priority**: 2-3 months
- **Medium Priority**: 3-4 months  
- **Low Priority**: 4-6 months

**Total estimated time**: 9-13 months for full feature parity with Prisma Client Rust 