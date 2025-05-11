# Caustics

A Prisma-like DSL for SeaORM that provides a type-safe and ergonomic way to build database queries.

> Caustics are the shimmering patterns of light that form when sunlight passes through water and reflects off the sea bed. Similarly, this crate bends and focuses SeaORM's query interface into a more ergonomic shape, offering an alternative to SeaORM's native DSL with a familiar Prisma-like syntax.

## Features

- Type-safe query building
- Prisma-like syntax
- Support for complex queries and relations
- Automatic SQL generation
- Transaction support
- Raw query support

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
use caustics::Caustics;
use sea_orm::entity::prelude::*;
use chrono::{DateTime, FixedOffset};
use serde_json::Value as Json;
use uuid::Uuid;

#[derive(Caustics)]
#[sea_orm(table_name = "users")]
#[sea_orm(primary_key = "id")]
#[sea_orm(has_many = "Post")]
struct User {
    #[sea_orm(primary_key)]
    id: i32,
    #[sea_orm(unique)]
    email: String,
    name: String,
    age: i32,
    #[sea_orm(created_at)]
    created_at: DateTime<FixedOffset>,
    #[sea_orm(updated_at)]
    updated_at: DateTime<FixedOffset>,
    #[sea_orm(nullable)]
    deleted_at: Option<DateTime<FixedOffset>>,
}

#[derive(Caustics)]
#[sea_orm(table_name = "posts")]
#[sea_orm(primary_key = "id")]
#[sea_orm(belongs_to = "User")]
struct Post {
    #[sea_orm(primary_key)]
    id: i32,
    title: String,
    content: String,
    #[sea_orm(created_at)]
    created_at: DateTime<FixedOffset>,
    #[sea_orm(updated_at)]
    updated_at: DateTime<FixedOffset>,
    #[sea_orm(column_name = "user_id")]
    user_id: i32,
}
```

### Query Operations

#### Find Operations

```rust
// Find a unique record
let user = client
    .user()
    .find_unique(user::id::equals(1))
    .await?;

// Find first record
let user = client
    .user()
    .find_first(vec![
        user::name::equals("John"),
        user::age::greater_than(18),
    ])
    .await?;

// Find many records
let users = client
    .user()
    .find_many(vec![
        user::age::greater_than(18),
        user::created_at::less_than(now),
    ])
    .await?;

// Find with relations
let user = client
    .user()
    .find_unique(user::id::equals(1))
    .with(user::posts::fetch(vec![]))
    .await?;

// Find posts with user
let posts = client
    .post()
    .find_many(vec![
        post::user_id::equals(1),
    ])
    .with(post::user::fetch(vec![]))
    .await?;
```

#### Create Operations

```rust
// Create a new record
let user = client
    .user()
    .create(
            user::name::set("John"),
            user::email::set("john@example.com"),
        vec![
            user::age::set(25),
        ],
    )
    .await?;

// Create a post with user relation
let post = client
    .post()
    .create(
            post::title::set("Hello World"),
            post::content::set("This is my first post"),
            post::user_id::set(1),
        vec![],
    )
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
            user::age::set(26),
        ],
    )
    .await?;

// Update a post
let post = client
    .post()
    .update(
        post::id::equals(1),
        vec![
            post::title::set("Updated Title"),
            post::content::set("Updated content"),
        ],
    )
    .await?;
```

#### Delete Operations

```rust
// Delete a record
client
    .user()
    .delete(1)
    .await?;

// Delete a post
client
    .post()
    .delete(1)
    .await?;
```

### Advanced Operations

#### Upsert

```rust
let user = client
    .user()
    .upsert(
        user::email::equals("john@example.com"),
        user::name::set("John"),
        user::age::set(25),
        vec![],
    )
    .await?;
```

#### Batch Operations

```rust
let results = client
    .user()
    ._batch(vec![
        |client| async move { client.user().find_unique(user::id::equals(1)).await },
        |client| async move { client.user().find_unique(user::id::equals(2)).await },
    ])
    .await?;
```

#### Transaction

```rust
let result = client
    ._transaction(|tx| async move {
        // Create user
        let user = tx
            .user()
            .create(
                user::name::set("John"),
                user::email::set("john@example.com"),
                vec![],
            )
            .await?;

        // Create post
        let post = tx
            .post()
            .create(
                post::title::set("Hello World"),
                post::content::set("This is my first post"),
                post::user_id::set(user.id),
                vec![],
            )
            .await?;

        Ok((user, post))
    })
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
user::age::greater_than(18)

// Greater Than or Equal
user::age::greater_than_or_equals(18)

// Less Than
user::age::less_than(18)

// Less Than or Equal
user::age::less_than_or_equals(18)

// In
user::id::in_vec(vec![1, 2, 3])

// Not In
user::id::not_in_vec(vec![1, 2, 3])
```

#### String Operators

```rust
// Contains
user::name::contains("John")

// Starts With
user::name::starts_with("John")

// Ends With
user::name::ends_with("Doe")
```

#### Null Operators

```rust
// Is Null
user::deleted_at::is_null()

// Is Not Null
user::deleted_at::is_not_null()
```

#### JSON Operators

```rust
// JSON Path
user::data::json_path(vec!["address", "city"], "New York", FieldType::String)

// JSON Contains
user::data::json_contains("address")
```

#### Logical Operators

```rust
// AND
Condition::and(vec![
    user::age::greater_than(18),
    user::name::equals("John"),
])

// OR
Condition::or(vec![
    user::age::greater_than(18),
    user::name::equals("John"),
])

// NOT
Condition::not(user::age::less_than(18))
```

#### Relation Operators

```rust
// Some
Condition::some(vec![
    user::posts::title::equals("Hello"),
])

// Every
Condition::every(vec![
    user::posts::title::equals("Hello"),
])

// None
Condition::none(vec![
    user::posts::title::equals("Hello"),
])
```

### Pagination and Sorting

```rust
let users = User::db(&db)
    .find_many(vec![user::age::greater_than(18)])
    .take(10)
    .skip(0)
    .order_by(user::created_at::order(SortOrder::Desc))
    .await?;

let posts = Post::db(&db)
    .find_many(vec![post::user_id::equals(1)])
    .take(10)
    .skip(0)
    .order_by(post::created_at::order(SortOrder::Desc))
    .await?;
```

### Raw Queries

```rust
let users = User::db(&db)
    .raw("SELECT * FROM users WHERE age > 18")
    .await?;

let posts = Post::db(&db)
    .raw("SELECT * FROM posts WHERE user_id = 1")
    .await?;
```

## License

This project is licensed under the Apache License 2.0 - see the [LICENSE](LICENSE) file for details. 