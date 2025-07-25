# Caustics

A Prisma-like DSL for SeaORM that provides a type-safe and ergonomic way to build database queries.

> Caustics are the shimmering patterns of light that form when sunlight passes through water and reflects off the sea bed. Similarly, this crate bends and focuses SeaORM's query interface into a more ergonomic shape, offering an alternative to SeaORM's native DSL with a familiar Prisma-like syntax deduced by reflection on SeaORM's datamodel.

## Features

- Type-safe query building
- Prisma-like syntax
- Support for complex queries and relations
- Advanced filtering operators (comparison, string, collection, logical, and null operators)
- Automatic SQL generation
- Transaction support
- Batch operations

## Installation

Add this to your `Cargo.toml`:

```toml
[dependencies]
caustics = { path = "../caustics" }
```

## Usage

### Client Initialization

```rust
use caustics::{Caustics, CausticsClient};
use sea_orm::DatabaseConnection;

// Initialize the client
let client = CausticsClient::new(db);

// Or use the extension trait
use caustics::CausticsExt;
let client = db.caustics();
```

### Basic Entity Definition

```rust
use caustics_macros::caustics;

#[caustics]
pub mod user {
    use caustics_macros::Caustics;
    use chrono::{DateTime, FixedOffset};
    use sea_orm::entity::prelude::*;

    #[derive(Caustics, Clone, Debug, PartialEq, DeriveEntityModel)]
    #[sea_orm(table_name = "users")]
    pub struct Model {
        #[sea_orm(primary_key)]
        pub id: i32,
        #[sea_orm(unique)]
        pub email: String,
        pub name: String,
        #[sea_orm(nullable)]
        pub age: Option<i32>,
        pub created_at: DateTime<FixedOffset>,
        pub updated_at: DateTime<FixedOffset>,
        #[sea_orm(nullable)]
        pub deleted_at: Option<DateTime<FixedOffset>>,
    }

    #[derive(Caustics, Copy, Clone, Debug, EnumIter, DeriveRelation)]
    pub enum Relation {
        #[sea_orm(
            has_many = "super::post::Entity",
            from = "Column::Id",
            to = "super::post::Column::UserId"
        )]
        Posts,
    }

    impl sea_orm::ActiveModelBehavior for ActiveModel {}
}

#[caustics]
pub mod post {
    use caustics_macros::Caustics;
    use chrono::{DateTime, FixedOffset};
    use sea_orm::entity::prelude::*;

    #[derive(Caustics, Clone, Debug, PartialEq, DeriveEntityModel)]
    #[sea_orm(table_name = "posts")]
    pub struct Model {
        #[sea_orm(primary_key)]
        pub id: i32,
        pub title: String,
        #[sea_orm(nullable)]
        pub content: Option<String>,
        #[sea_orm(created_at)]
        pub created_at: DateTime<FixedOffset>,
        #[sea_orm(updated_at)]
        pub updated_at: DateTime<FixedOffset>,
        #[sea_orm(column_name = "user_id")]
        pub user_id: i32,
        #[sea_orm(column_name = "reviewer_user_id", nullable)]
        pub reviewer_user_id: Option<i32>,
    }

    #[derive(Caustics, Copy, Clone, Debug, EnumIter, DeriveRelation)]
    pub enum Relation {
        #[sea_orm(
            belongs_to = "super::user::Entity",
            from = "Column::UserId",
            to = "super::user::Column::Id",
            on_update = "NoAction",
            on_delete = "NoAction"
        )]
        User,
        #[sea_orm(
            belongs_to = "super::user::Entity",
            from = "Column::ReviewerUserId",
            to = "super::user::Column::Id",
            on_update = "NoAction",
            on_delete = "NoAction"
        )]
        Reviewer,
    }

    impl ActiveModelBehavior for ActiveModel {}
}
```

### Query Operations

#### Find Operations

```rust
// Find a unique record
let user = client
    .user()
    .find_unique(user::id::equals(1))
    .exec()
    .await?;

// Find first record
let user = client
    .user()
    .find_first(vec![
        user::name::equals("John"),
        user::age::gt(18),
    ])
    .exec()
    .await?;

// Find many records
let users = client
    .user()
    .find_many(vec![
        user::age::gt(18),
    ])
    .exec()
    .await?;

// Find with relations
let user = client
    .user()
    .find_unique(user::id::equals(1))
    .with(user::posts::fetch(vec![]))
    .exec()
    .await?;

// Find posts with user
let posts = client
    .post()
    .find_many(vec![
        post::user_id::equals(1),
    ])
    .with(post::user::fetch(vec![]))
    .exec()
    .await?;
```

#### Create Operations

```rust
// Create a new record
let user = client
    .user()
    .create(
        "john@example.com".to_string(),
        "John".to_string(),
        DateTime::<FixedOffset>::from_str("2021-01-01T00:00:00Z").unwrap(),
        DateTime::<FixedOffset>::from_str("2021-01-01T00:00:00Z").unwrap(),
        vec![user::age::set(Some(25))],
    )
    .exec()
    .await?;

// Create a post with user relation
let post = client
    .post()
    .create(
        "Hello World".to_string(),
        DateTime::<FixedOffset>::from_str("2021-01-01T00:00:00Z").unwrap(),
        DateTime::<FixedOffset>::from_str("2021-01-01T00:00:00Z").unwrap(),
        user::id::equals(1),
        vec![post::content::set(Some("This is my first post".to_string()))],
    )
    .exec()
    .await?;
```

#### Update Operations

```rust
// Update a record
let user = client
    .user()
    .update(
        user::id::equals(1),
        vec![
            user::name::set("John Doe"),
            user::age::set(Some(26)),
        ],
    )
    .exec()
    .await?;

// Update a post
let post = client
    .post()
    .update(
        post::id::equals(1),
        vec![
            post::title::set("Updated Title"),
            post::content::set(Some("Updated content".to_string())),
        ],
    )
    .exec()
    .await?;
```

#### Delete Operations

```rust
// Delete a record
client
    .user()
    .delete(user::id::equals(1))
    .exec()
    .await?;

// Delete a post
client
    .post()
    .delete(post::id::equals(1))
    .exec()
    .await?;
```

### Advanced Operations

#### Upsert

```rust
let user = client
    .user()
    .upsert(
        user::email::equals("john@example.com"),
        user::Create {
            name: "John".to_string(),
            email: "john@example.com".to_string(),
            created_at: DateTime::<FixedOffset>::from_str("2021-01-01T00:00:00Z").unwrap(),
            updated_at: DateTime::<FixedOffset>::from_str("2021-01-01T00:00:00Z").unwrap(),
            _params: vec![],
        },
        vec![user::name::set("John"), user::age::set(25)],
    )
    .exec()
    .await?;
```

#### Batch Operations

```rust
let (user1, user2) = client
    ._batch((
        client.user().create(
            "john@example.com".to_string(),
            "John".to_string(),
            DateTime::<FixedOffset>::from_str("2021-01-01T00:00:00Z").unwrap(),
            DateTime::<FixedOffset>::from_str("2021-01-01T00:00:00Z").unwrap(),
            vec![user::age::set(Some(25))],
        ),
        client.user().create(
            "jane@example.com".to_string(),
            "Jane".to_string(),
            DateTime::<FixedOffset>::from_str("2021-01-01T00:00:00Z").unwrap(),
            DateTime::<FixedOffset>::from_str("2021-01-01T00:00:00Z").unwrap(),
            vec![user::age::set(Some(30))],
        ),
    ))
    .await?;
```

#### Transaction

```rust
let result = client
    ._transaction()
    .run(|tx| {
        Box::pin(async move {
            // Create user
            let user = tx
                .user()
                .create(
                    "john@example.com".to_string(),
                    "John".to_string(),
                    DateTime::<FixedOffset>::from_str("2021-01-01T00:00:00Z").unwrap(),
                    DateTime::<FixedOffset>::from_str("2021-01-01T00:00:00Z").unwrap(),
                    vec![],
                )
                .exec()
                .await?;

            // Create post
            let post = tx
                .post()
                .create(
                    "Hello World".to_string(),
                    DateTime::<FixedOffset>::from_str("2021-01-01T00:00:00Z").unwrap(),
                    DateTime::<FixedOffset>::from_str("2021-01-01T00:00:00Z").unwrap(),
                    user::id::equals(user.id),
                    vec![post::content::set(Some("This is my first post".to_string()))],
                )
                .exec()
                .await?;

            Ok((user, post))
        })
    })
    .await?;
```

### Advanced Filtering

#### String Search with Case-Insensitive Mode

```rust
// Case-sensitive search (default)
let users = client
    .user()
    .find_many(vec![
        user::name::contains("john"),
    ])
    .exec()
    .await?;

// Case-insensitive search
let users = client
    .user()
    .find_many(vec![
        user::name::contains("john"),
        user::name::mode(caustics::QueryMode::Insensitive),
    ])
    .exec()
    .await?;
```

#### Complex Logical Queries

```rust
// Find users who are either young or old, but not middle-aged
let users = client
    .user()
    .find_many(vec![
        user::or(vec![
            user::age::lt(Some(25)),
            user::age::gt(Some(55))
        ])
    ])
    .exec()
    .await?;

// Find users with specific criteria using AND logic
let users = client
    .user()
    .find_many(vec![
        user::and(vec![
            user::age::gte(Some(18)),
            user::name::starts_with("J"),
            user::email::contains("example.com")
        ])
    ])
    .exec()
    .await?;

// Exclude users with certain characteristics
let users = client
    .user()
    .find_many(vec![
        user::not(vec![
            user::age::lt(Some(18))
        ])
    ])
    .exec()
    .await?;
```

#### Collection Queries

```rust
// Find users with specific IDs
let users = client
    .user()
    .find_many(vec![
        user::id::in_vec(vec![1, 2, 3, 5, 8])
    ])
    .exec()
    .await?;

// Find users excluding certain ages
let users = client
    .user()
    .find_many(vec![
        user::age::not_in_vec(vec![Some(13), Some(14), Some(15)])
    ])
    .exec()
    .await?;
```

#### Null Value Filtering

```rust
// Find users who haven't set their age
let users_without_age = client
    .user()
    .find_many(vec![
        user::age::is_null()
    ])
    .exec()
    .await?;

// Find active users (not deleted)
let active_users = client
    .user()
    .find_many(vec![
        user::deleted_at::is_null()
    ])
    .exec()
    .await?;

// Find posts with content
let posts_with_content = client
    .post()
    .find_many(vec![
        post::content::is_not_null()
    ])
    .exec()
    .await?;

// Complex query: Find adult users who are not deleted
let active_adults = client
    .user()
    .find_many(vec![
        user::and(vec![
            user::age::is_not_null(),
            user::age::gte(Some(18)),
            user::deleted_at::is_null()
        ])
    ])
    .exec()
    .await?;
```


### Available Operators

#### Comparison Operators

```rust
// Equals
user::id::equals(1)

// Not Equals
user::id::not_equals(1)

// Greater Than
user::age::gt(18)

// Greater Than or Equal
user::age::gte(18)

// Less Than
user::age::lt(18)

// Less Than or Equal
user::age::lte(18)
```

#### String Operators

```rust
// Contains
user::name::contains("John")

// Starts With
user::name::starts_with("John")

// Ends With
user::name::ends_with("Doe")

// Case-insensitive mode
user::name::mode(caustics::QueryMode::Insensitive)
```

#### Collection Operators

```rust
// In
user::id::in_vec(vec![1, 2, 3])

// Not In
user::id::not_in_vec(vec![1, 2, 3])
```

#### Logical Operators

```rust
// AND - combine multiple conditions (all must be true)
user::and(vec![
    user::age::gt(18),
    user::name::contains("John")
])

// OR - combine multiple conditions (any can be true)
user::or(vec![
    user::age::lt(18),
    user::age::gt(65)
])

// NOT - negate conditions
user::not(vec![
    user::age::lt(18)
])
```

#### Null Operators

```rust
// Is Null - check if a nullable field is null
user::age::is_null()
user::deleted_at::is_null()
post::content::is_null()

// Is Not Null - check if a nullable field has a value
user::age::is_not_null()
user::deleted_at::is_not_null()
post::content::is_not_null()

// Combining null operators with logical operators
user::and(vec![
    user::age::is_not_null(),
    user::deleted_at::is_null()
])

user::or(vec![
    user::age::is_null(),
    user::deleted_at::is_not_null()
])
```

> **Note**: Null operators are only available for nullable fields (marked with `#[sea_orm(nullable)]` or `Option<T>` types). Attempting to use null operators on non-nullable fields will result in a compile-time error.

### Pagination and Sorting

```rust
let users = client
    .user()
    .find_many(vec![user::age::gt(18)])
    .take(10)
    .skip(0)
    .order_by(user::created_at::order(SortOrder::Desc))
    .exec()
    .await?;

let posts = client
    .post()
    .find_many(vec![post::user_id::equals(1)])
    .take(10)
    .skip(0)
    .order_by(post::created_at::order(SortOrder::Desc))
    .exec()
    .await?;
```

## Recent Updates

**String Operators** - Full support for string search operations:
- `contains()`, `starts_with()`, `ends_with()` for all string fields
- Case-insensitive search with `QueryMode::Insensitive`
- Works with both regular and nullable string fields

**Comparison Operators** - Complete set of comparison operations:
- `gt()`, `gte()`, `lt()`, `lte()`, `not_equals()` for all comparable types
- Support for integers, floats, strings, dates, and their nullable variants

**Collection Operators** - Query with lists of values:
- `in_vec()` and `not_in_vec()` for efficient multi-value filtering
- Proper handling of nullable fields in collections

**Logical Operators** - Complex query composition:
- `and()`, `or()`, `not()` functions for combining multiple conditions
- Nested logical expressions with proper precedence
- Type-safe condition building

All features include comprehensive test coverage and are ready for production use.

## TODO: Planned Features

The following features are planned but not yet implemented:

### JSON Operators
- `user::data::json_path(vec!["address", "city"], "New York", FieldType::String)`
- `user::data::json_contains("address")`

### Relation Operators
- `Condition::some(vec![user::posts::title::equals("Hello")])`
- `Condition::every(vec![user::posts::title::equals("Hello")])`
- `Condition::none(vec![user::posts::title::equals("Hello")])`

### Atomic Operations
- Increment/decrement operations for numeric fields
- Multiply/divide operations for numeric fields

### Additional Features
- Create many operations
- Update many operations
- Delete many operations
- Aggregation functions (count, sum, avg, etc.)
- Raw SQL queries
- Database migrations
- Connection pooling
- Query optimization

## Acknowledgments

This project is inspired by the excellent work done on [Prisma Client Rust](https://github.com/Brendonovich/prisma-client-rust), which provides a type-safe database client for Rust. While Caustics is not derived from Prisma Client Rust, it shares similar design goals of providing an ergonomic, type-safe database interface and is intended to be a drop-in replacement for most of the features.

## License

This project is licensed under the Apache License 2.0 - see the [LICENSE](LICENSE) file for details. 

