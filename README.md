# Caustics

A Prisma-like DSL for SeaORM that provides a type-safe and ergonomic way to build database queries.

> Caustics are the shimmering patterns of light that form when sunlight passes through water and reflects off the sea bed. Similarly, this crate bends and focuses SeaORM's query interface into a more ergonomic shape, offering an alternative to SeaORM's native DSL with a familiar Prisma-like syntax deduced by reflection on SeaORM's datamodel.

## Table of Contents

- [Prerequisites](#prerequisites)
- [Getting Started](#getting-started)
- [Installation](#installation)
- [Feature Usage](#feature-usage)
- [Quick Start](#quick-start)
- [Define Entities](#define-entities)
- [Basic Operations](#basic-operations)
- [Relations and Includes](#relations-and-includes)
- [Filtering](#filtering)
- [Pagination and Sorting](#pagination-and-sorting)
- [Advanced Features](#advanced-features)
- [Testing](#testing)
- [Acknowledgments](#acknowledgments)
- [License](#license)

## Prerequisites

- Rust 1.70+ (stable or nightly)
- SeaORM-compatible database (PostgreSQL, MySQL, SQLite)
- Basic familiarity with SeaORM and Rust async/await

## Getting Started

### Building the Project

To build and test the Caustics project:

```bash
# Build the project
cargo build

# Run tests (stable Rust)
cargo test

# Run tests with nightly features
cargo +nightly test --workspace --exclude library --all-features


# Build examples
cargo build --examples
```

## Installation

Add this to your `Cargo.toml`:

```toml
[dependencies]
caustics = { path = "../caustics" }
caustics-macros = { path = "../caustics-macros" }
sea-orm = { version = "1.1", features = ["sqlx-sqlite", "runtime-tokio-rustls", "macros"] }
sea-query = "0.32"
chrono = { version = "0.4", features = ["serde"] }
uuid = { version = "1", features = ["v4", "serde"] }
serde = { version = "1", features = ["derive"] }
serde_json = "1"

[build-dependencies]
caustics-build = { path = "../caustics-build" }
```

### Configuring the Build Script

Create a `build.rs` file in your project root to generate the Caustics client:

```rust
use caustics_build::generate_caustics_client;

fn main() {
    if let Err(e) = generate_caustics_client(&["src"], "caustics_client.rs") {
        eprintln!("Error generating client: {}", e);
        std::process::exit(1);
    }
}
```

This build script will automatically generate a `caustics_client.rs` file in your `OUT_DIR` that contains the client code for all entities marked with the `#[caustics]` macro. Include the generated client in your main library file:

```rust
// In your src/lib.rs or src/main.rs
include!(concat!(env!("OUT_DIR"), "/caustics_client.rs"));
```

Toolchain support: this project supports both stable and nightly Rust toolchains. The `select!` macro requires nightly Rust and is gated behind the "select" feature. Use stable Rust for basic functionality, or enable the "select" feature with nightly Rust for enhanced field selection syntax.



## Feature Usage

### Stable Rust (Default)
```toml
[dependencies]
caustics = { path = "../caustics" }
caustics-macros = { path = "../caustics-macros" }
```

### Nightly Rust with Enhanced Selection
```toml
[dependencies]
caustics = { path = "../caustics" }
caustics-macros = { path = "../caustics-macros", features = ["select"] }
```

* When the "select" feature is enabled, you can use the convenient `entity::select!(field1, field2)`

## Quick Start

```rust
use caustics_client::CausticsClient;
use caustics_macros::{caustics, select_struct};
use sea_orm::{Database, DatabaseConnection};

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    // Connect to your database
    let db: DatabaseConnection = Database::connect("your_database_url").await?;
    
    // Create a Caustics client
    let client = CausticsClient::new(db);
    
    // Now you can use the client for type-safe database operations
    // See the examples below for more details
    
    Ok(())
}
```

## Define Entities

```rust
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

    // Add Related trait implementation for relations
    impl Related<super::post::Entity> for Entity {
        fn to() -> RelationDef {
            Relation::Posts.def()
        }
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
    }

    #[derive(Caustics, Copy, Clone, Debug, EnumIter, DeriveRelation)]
    pub enum Relation {
        #[sea_orm(
            belongs_to = "super::user::Entity",
            from = "Column::UserId",
            to = "super::user::Column::Id"
        )]
        User,
    }

    // Add Related trait implementation for relations
    impl Related<super::user::Entity> for Entity {
        fn to() -> RelationDef {
            Relation::User.def()
        }
    }

    impl ActiveModelBehavior for ActiveModel {}
}
```

## Basic Operations

### Find

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
    .find_many(vec![user::age::gt(18)])
    .exec()
    .await?;
```

### Create

```rust
// Create a new record
let user = client
    .user()
    .create(
        "john@example.com".to_string(),
        "John".to_string(),
        DateTime::<FixedOffset>::parse_from_rfc3339("2021-01-01T00:00:00Z").unwrap(),
        DateTime::<FixedOffset>::parse_from_rfc3339("2021-01-01T00:00:00Z").unwrap(),
        vec![user::age::set(Some(25))],
    )
    .exec()
    .await?;
```

### Update

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

// Partial updates with type safety
let updated_user = client
    .user()
    .update(
        user::id::equals(1),
        vec![
            user::name::set("New Name".to_string()),
            // Only update specific fields, others remain unchanged
        ]
    )
    .exec()
    .await?;

// Set field to null
let user_with_null_age = client
    .user()
    .update(
        user::id::equals(1),
        vec![user::age::set(None)]
    )
    .exec()
    .await?;
```

### Delete

```rust
// Delete a record
client
    .user()
    .delete(user::id::equals(1))
    .exec()
    .await?;
```

## Relations and Includes

### Basic Relations

```rust
// Include relations (full model)
let users_with_posts = client
    .user()
    .find_many(vec![])
    .with(user::posts::fetch())
    .exec()
    .await?;

// Select specific fields
let users_basic = client
    .user()
    .find_many(vec![])
    .select(user::select!(id, name))
    .exec()
    .await?;
```

### Nested Relations with Custom Structs

```rust
// First, define custom structs for type-safe nested data
select_struct! {
    UserWithPosts from user::Selected {
        id: i32,
        name: String,
        posts: Vec<PostSummary from post::Selected {
            id: i32,
            title: String,
            created_at: DateTime<FixedOffset>
        }>
    }
}

// Simple nested relation
let user_with_posts: UserWithPosts = client
    .user()
    .find_unique(user::id::equals(1))
    .select(user::select!(id, name))
    .with(user::posts::include(|posts| {
        posts.select(post::select!(id, title, created_at))
    }))
    .exec()
    .await?
    .unwrap();

// Access nested data with full type safety
for post in user_with_posts.posts {
}
```

### Deep Nested Relations

```rust
// Complex nested structs for deep relations
select_struct! {
    StudentWithEnrollments from student::Selected {
        first_name: String,
        last_name: String,
        enrollments: Vec<EnrollmentWithCourse from enrollment::Selected {
            id: i32,
            enrollment_date: DateTime<FixedOffset>,
            status: String,
            course: CourseWithTeacher from course::Selected {
                name: String,
                teacher: TeacherData from teacher::Selected {
                    first_name: String,
                    last_name: String
                }
            }
        }>
    }
}

// Deep nested query with custom structs
let student: StudentWithEnrollments = client
    .student()
    .find_unique(student::id::equals(1))
    .select(student::select!(first_name, last_name))
    .with(student::enrollments::include(|enrollments| {
        enrollments
            .select(enrollment::select!(id, enrollment_date, status))
            .with(enrollment::course::include(|course| {
                course
                    .select(course::select!(name))
                    .with(course::teacher::include(|teacher| {
                        teacher.select(teacher::select!(first_name, last_name))
                    }))
            }))
    }))
    .exec()
    .await?
    .unwrap();

// Access deeply nested data
for enrollment in student.enrollments {
}
```

## Filtering

### Basic Filters

```rust
// String search
let users = client
    .user()
    .find_many(vec![
        user::name::contains("john"),
        user::age::gt(18),
    ])
    .exec()
    .await?;

// Collections
let users = client
    .user()
    .find_many(vec![
        user::id::in_vec(vec![1, 2, 3]),
    ])
    .exec()
    .await?;

// Null checks
let users = client
    .user()
    .find_many(vec![
        user::age::is_not_null(),
    ])
    .exec()
    .await?;
```

### Logical Operators

```rust
// AND/OR/NOT
let users = client
    .user()
    .find_many(vec![
        user::and(vec![
            user::age::gte(Some(18)),
            user::name::starts_with("J"),
        ]),
        user::or(vec![
            user::age::lt(Some(25)),
            user::age::gt(Some(55))
        ]),
    ])
    .exec()
    .await?;
```

## Pagination and Sorting

```rust
let users = client
    .user()
    .find_many(vec![user::age::gt(18)])
    .take(10)
    .skip(0)
    .order_by(user::created_at::order(SortOrder::Desc))
    .exec()
    .await?;
```

### Relation Ordering

Caustics supports powerful relation ordering capabilities, allowing you to sort by related data:

```rust
// Order users by their post count (ascending)
let users_by_post_count = client
    .user()
    .find_many(vec![])
    .with(user::posts::fetch())
    .order_by(user::posts::count(SortOrder::Asc))
    .exec()
    .await?;

// Order users by their post count (descending) 
let users_most_active = client
    .user()
    .find_many(vec![])
    .with(user::posts::fetch())
    .order_by(user::posts::count(SortOrder::Desc))
    .exec()
    .await?;

// Complex ordering - by post count, then by name
let users_complex_order = client
    .user()
    .find_many(vec![])
    .with(user::posts::fetch())
    .order_by(user::posts::count(SortOrder::Asc))
    .order_by(user::name::order(SortOrder::Asc))
    .exec()
    .await?;
```

### Advanced Ordering with Nulls Handling

```rust
// Order by age with nulls first
let users_nulls_first = client
    .user()
    .find_many(vec![])
    .order_by((
        user::age::order(SortOrder::Asc),
        NullsOrder::First,
    ))
    .exec()
    .await?;

// Order by age with nulls last
let users_nulls_last = client
    .user()
    .find_many(vec![])
    .order_by((
        user::age::order(SortOrder::Desc),
        NullsOrder::Last,
    ))
    .exec()
    .await?;
```

## Advanced Features

### Batch Operations

```rust
// createMany
let inserted = client
    .user()
    .create_many(vec![
        user::Create {
            email: "user1@example.com".to_string(),
            name: "User 1".to_string(),
            created_at: DateTime::<FixedOffset>::parse_from_rfc3339("2021-01-01T00:00:00Z").unwrap(),
            updated_at: DateTime::<FixedOffset>::parse_from_rfc3339("2021-01-01T00:00:00Z").unwrap(),
            _params: vec![],
        },
        // ... more users
    ])
    .exec()
    .await?;

// updateMany
let affected = client
    .user()
    .update_many(
        vec![user::age::gte(Some(30))],
        vec![user::name::set("Updated")],
    )
    .exec()
    .await?;
```

### Transactions

```rust
let result = client
    ._transaction()
    .run(|tx| async move {
        let user = tx
            .user()
            .create(
                "john@example.com".to_string(),
                "John".to_string(),
                DateTime::<FixedOffset>::parse_from_rfc3339("2021-01-01T00:00:00Z").unwrap(),
                DateTime::<FixedOffset>::parse_from_rfc3339("2021-01-01T00:00:00Z").unwrap(),
                vec![],
            )
            .exec()
            .await?;

        let post = tx
            .post()
            .create(
                "Hello World".to_string(),
                DateTime::<FixedOffset>::parse_from_rfc3339("2021-01-01T00:00:00Z").unwrap(),
                DateTime::<FixedOffset>::parse_from_rfc3339("2021-01-01T00:00:00Z").unwrap(),
                user::id::equals(user.id),
                vec![],
            )
            .exec()
            .await?;

        Ok((user, post))
    })
    .await?;
```

### Aggregates

```rust
let agg = client
    .user()
    .aggregate(vec![user::age::is_not_null()])
    .count()
    .avg(user::select!(age), "age_avg")
    .min(user::select!(age), "age_min")
    .max(user::select!(age), "age_max")
    .exec()
    .await?;

// Access aggregate values by alias
let avg = agg.values.get("age_avg");
```

### Atomic Operations

```rust
// Increment a user's age by 5
let user = client
    .user()
    .update(
        user::id::equals(1),
        vec![user::age::increment(5)]
    )
    .exec()
    .await?;
```

### Raw SQL

```rust
use sea_orm::FromQueryResult;

#[derive(FromQueryResult)]
struct Row { value: i32 }

// Typed query
let rows: Vec<Row> = client
    ._query_raw::<Row>(raw!("SELECT {} as value", 1))
    .exec()
    .await?;
```



## Acknowledgments

This project is inspired by the excellent work done on [Prisma Client Rust](https://github.com/Brendonovich/prisma-client-rust), which provides a type-safe database client for Rust. While Caustics is not derived from Prisma Client Rust, it shares similar design goals of providing an ergonomic, type-safe database interface and is intended to be a drop-in replacement for most of its features.

## License

This project is licensed under the Apache License 2.0 - see the [LICENSE](LICENSE) file for details.