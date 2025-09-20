# Caustics

A Prisma-like DSL for SeaORM that provides a type-safe and ergonomic way to build database queries.

> Caustics are the shimmering patterns of light that form when sunlight passes through water and reflects off the sea bed. Similarly, this crate bends and focuses SeaORM's query interface into a more ergonomic shape, offering an alternative to SeaORM's native DSL with a familiar Prisma-like syntax deduced by reflection on SeaORM's datamodel.

## Installation

Add this to your `Cargo.toml`:

```toml
[dependencies]
caustics = { path = "../caustics" }
```

Toolchain requirement: this project requires a fixed nightly toolchain for per-entity macro syntax (`entity::select!(...)`). The workspace pins nightly via `rust-toolchain.toml` (nightly-2025-08-31).

## Usage

### Quick start

```rust
use caustics_client::CausticsClient;

// Given an existing SeaORM DatabaseConnection `db`
let client = CausticsClient::new(db.clone());
```

### Raw SQL APIs (typed)

```rust
use sea_orm::FromQueryResult;

#[derive(FromQueryResult)]
struct Row { value: i32 }

// Typed query (returns Vec<Row>)
let rows: Vec<Row> = client
    ._query_raw::<Row>(raw!("SELECT {} as value", 1))
    .exec()
    .await?;

// Execute statement (no result set)
let res = client
    ._execute_raw(raw!("CREATE TEMP TABLE {} (id int)", ident!("__raw_tmp")))
    .exec()
    .await?;

// Binding helpers
use caustics::{ident, in_params};

let (ph, args) = in_params!(&[1,2,3]);
// Injection-safe: values (strings, numbers, json, etc.) are always bound as params
// Use ident!(..) for identifiers (table/column names); never inline user input as identifiers
#[derive(FromQueryResult)]
struct UserRow { id: i32, name: String }
let users: Vec<UserRow> = client
    ._query_raw::<UserRow>(raw!(
        "SELECT id, name FROM {} WHERE id IN ({})",
        ident!("users"),
        caustics::raw::Inline(ph)
    ).with_params(args))
    .exec()
    .await?;

// Mixed bound types
#[derive(FromQueryResult)]
struct Mixed { s: String, n: i64 }
let rows: Vec<Mixed> = client
    ._query_raw::<Mixed>(raw!("SELECT {} as s, {} as n", "hello", 42))
    .exec()
    .await?;
```

### Define entities

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
    .find_many(vec![
        user::age::gt(18),
    ])
    .exec()
    .await?;

// Include relations
let user_with_posts = client
    .user()
    .find_unique(user::id::equals(1))
    .with(user::posts::fetch())
    .exec()
    .await?;
```

#### Include and Select

```rust
// Include relations (full model)
let users_with_posts = client
    .user()
    .find_many(vec![])
    .with(user::posts::fetch())
    .exec()
    .await?;

// Select scalar fields (returns a Selected holder with only requested fields)
let users_basic = client
    .user()
    .find_many(vec![])
    .select(user::select!(id, name))
    .exec()
    .await?;

// Include on selections – implicit keys are fetched automatically when needed
let students = client
    .student()
    .find_many(vec![])
    .select(student::select!(first_name))
    .with(student::enrollments::include(|rel| rel
        .filter(vec![enrollment::status::contains("en".to_string())])
        .order_by(vec![enrollment::id::order(SortOrder::Desc)])
        .take(10)
        .skip(0)
        .cursor(0)
        .distinct()
        .count()
    ))
    .exec()
    .await?;

// Order parents by child relation counts (e.g., students by enrollments count desc)
let students_by_enrollments = client
    .student()
    .find_many(vec[])
    .order_by(student::enrollments::order_by(enrollment::id::count(SortOrder::Desc)))
    .take(10)
    .exec()
    .await?;

// Deep nested select/include using closure API only
let selected = client
    .student()
    .find_unique(student::id::equals(1))
    .select(student::select!(first_name, last_name))
    .with(student::enrollments::include(|rel| rel
        .with(enrollment::course::include(|rel2| rel2
            .select(course::select!(name))
            .with(course::teacher::include(|rel3| rel3
                .select(teacher::select!(first_name))
            ))
        ))
    ))
    .exec()
    .await?;
```

#### Relation include closure API

```rust
// Include builder per relation node
let s = client
  .student()
  .find_unique(student::id::equals(1))
  .with(student::enrollments::include(|rel| rel
    .filter(vec![enrollment::status::equals("enrolled".to_string())])
    .order_by(vec![enrollment::status::order(SortOrder::Asc)])
    .cursor(0)
    .take(5)
    .skip(0)
    .distinct()
    .select(enrollment::select!(status))
    .count()
  ))
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

// Create a post with user relation
let post = client
    .post()
    .create(
        "Hello World".to_string(),
        DateTime::<FixedOffset>::parse_from_rfc3339("2021-01-01T00:00:00Z").unwrap(),
        DateTime::<FixedOffset>::parse_from_rfc3339("2021-01-01T00:00:00Z").unwrap(),
        user::id::equals(1),
        vec![post::content::set(Some("This is my first post".to_string()))],
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

### Nested writes (has_many) on update

Nested has_many create and createMany can be performed inside an update. Caustics executes the nested creates, relation set operations (if any), and scalar field updates in a single transaction atomically.

```rust
// Atomically set a scalar field and create new enrollments for a student
let _updated = client
    .student()
    .update(
        student::id::equals(student_id),
        vec![
            student::first_name::set("UpdatedName".to_string()),
            // Create a single enrollment
            student::enrollments::create(vec![enrollment::Create {
                enrollment_date: fixed_now(),
                status: "enrolled".to_string(),
                created_at: fixed_now(),
                updated_at: fixed_now(),
                student: student::id::equals(student_id),
                course: course::id::equals(course_id_a),
                _params: vec![],
            }]),
            // Create multiple enrollments
            student::enrollments::create_many(vec![
                enrollment::Create {
                    enrollment_date: fixed_now(),
                    status: "enrolled".to_string(),
                    created_at: fixed_now(),
                    updated_at: fixed_now(),
                    student: student::id::equals(student_id),
                    course: course::id::equals(course_id_b),
                    _params: vec![],
                },
                enrollment::Create {
                    enrollment_date: fixed_now(),
                    status: "completed".to_string(),
                    created_at: fixed_now(),
                    updated_at: fixed_now(),
                    student: student::id::equals(student_id),
                    course: course::id::equals(course_id_b),
                    _params: vec![],
                },
            ]),
        ],
    )
    .exec()
    .await?;
```

### Atomic

Caustics supports atomic numeric operations that are performed at the database level for safe concurrent updates:

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

// Decrement a product's stock count
let product = client
    .product()
    .update(
        product::id::equals(123),
        vec![product::stock::decrement(1)]
    )
    .exec()
    .await?;

// Double a user's score
let user = client
    .user()
    .update(
        user::id::equals(1),
        vec![user::score::multiply(2)]
    )
    .exec()
    .await?;

// Split a value in half
let user = client
    .user()
    .update(
        user::id::equals(1),
        vec![user::balance::divide(2)]
    )
    .exec()
    .await?;

// Atomic operations handle null values appropriately:
// - increment/decrement on null: sets to the operation value
// - multiply/divide on null: remains null
let user = client
    .user()
    .update(
        user::id::equals(1),
        vec![user::optional_score::increment(10)] // null becomes 10
    )
    .exec()
    .await?;
```

### Transactions and atomicity

- Create: parent insert and all post-insert nested writes run atomically. In transaction contexts, everything uses the provided transaction.
- Update (with has_many set and/or nested create): nested creates, set operations, and scalar update execute in a single transaction.
- Upsert: create or update branches run atomically; post-insert operations for the create branch are executed in the same transaction.

### Delete

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

### Upsert

```rust
let user = client
    .user()
    .upsert(
        user::email::equals("john@example.com"),
        user::Create {
            name: "John".to_string(),
            email: "john@example.com".to_string(),
            created_at: DateTime::<FixedOffset>::parse_from_rfc3339("2021-01-01T00:00:00Z").unwrap(),
            updated_at: DateTime::<FixedOffset>::parse_from_rfc3339("2021-01-01T00:00:00Z").unwrap(),
            _params: vec![],
        },
        vec![user::name::set("John"), user::age::set(25)],
    )
    .exec()
    .await?;
```

### createMany / updateMany

```rust
// createMany users
let inserted = client
    .user()
    .create_many(vec![
        user::Create {
            email: "cm1@example.com".to_string(),
            name: "CM1".to_string(),
            created_at: DateTime::<FixedOffset>::parse_from_rfc3339("2021-01-01T00:00:00Z").unwrap(),
            updated_at: DateTime::<FixedOffset>::parse_from_rfc3339("2021-01-01T00:00:00Z").unwrap(),
            _params: vec![user::age::set(Some(21))],
        },
        user::Create {
            email: "cm2@example.com".to_string(),
            name: "CM2".to_string(),
            created_at: DateTime::<FixedOffset>::parse_from_rfc3339("2021-01-01T00:00:00Z").unwrap(),
            updated_at: DateTime::<FixedOffset>::parse_from_rfc3339("2021-01-01T00:00:00Z").unwrap(),
            _params: vec![user::age::set(Some(22))],
        },
    ])
    .exec()
    .await?;

// updateMany matching users
let affected = client
    .user()
    .update_many(
        vec![user::age::gte(Some(30))],
        vec![user::deleted_at::set(Some(DateTime::<FixedOffset>::parse_from_rfc3339("2021-12-31T00:00:00Z").unwrap()))],
    )
    .exec()
    .await?;
```

### Batch

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

### Transaction

```rust
let result = client
    ._transaction()
    .run(|tx| async move {
        // Create user
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

        // Create post
        let post = tx
            .post()
            .create(
                "Hello World".to_string(),
                DateTime::<FixedOffset>::parse_from_rfc3339("2021-01-01T00:00:00Z").unwrap(),
                DateTime::<FixedOffset>::parse_from_rfc3339("2021-01-01T00:00:00Z").unwrap(),
                user::id::equals(user.id),
                vec![post::content::set(Some("This is my first post".to_string()))],
            )
            .exec()
            .await?;

        Ok((user, post))
    })
    .await?;
```

### Aggregates and Group By

Caustics provides Prisma-like aggregate and group-by APIs with typed field selectors.

```rust
// Aggregates with typed selectors
let agg = client
    .user()
    .aggregate(vec![user::age::is_not_null()])
    .select_count()
    .select_avg_typed(user::AvgSelect::Age, "age_avg")
    .select_min_typed(user::MinSelect::Age, "age_min")
    .select_max_typed(user::MaxSelect::Age, "age_max")
    .exec()
    .await?;

// Access aggregate values by alias
let avg = agg.values.get("age_avg");

// Group by with typed fields, aggregates and HAVING using plain Rust types
let rows = client
    .user()
    .group_by(vec![user::GroupByFieldParam::Age], vec![])
    .select_count("cnt")
    .select_sum(user::SumSelect::Age, "age_sum")
    .having_sum_gte(user::SumSelect::Age, 20)
    .exec()
    .await?;
```

Notes:
- The typed selectors are enums generated per-entity: `SumSelect`, `AvgSelect`, `MinSelect`, `MaxSelect`.
- Group-by HAVING helpers accept plain Rust numeric types via `Into<sea_orm::Value>`.

### Filtering

### String search (case-insensitive)

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

### Logical

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

### Collections

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

### Nulls

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


### Operators: Comparison

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

### Operators: String

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

### Operators: Collection

```rust
// In
user::id::in_vec(vec![1, 2, 3])

// Not In
user::id::not_in_vec(vec![1, 2, 3])
```

### Operators: Logical

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

### Operators: Null

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

### Nulls Order

Caustics supports controlling how NULL values are ordered in query results, similar to Prisma Client Rust:

```rust
// Order by age ascending with NULLs first
let users = client
    .user()
    .find_many(vec![])
    .order_by((user::age::order(SortOrder::Asc), caustics::NullsOrder::First))
    .exec()
    .await?;

// Order by age ascending with NULLs last
let users = client
    .user()
    .find_many(vec![])
    .order_by((user::age::order(SortOrder::Asc), caustics::NullsOrder::Last))
    .exec()
    .await?;

// Order by age descending with NULLs first
let users = client
    .user()
    .find_many(vec![])
    .order_by((user::age::order(SortOrder::Desc), caustics::NullsOrder::First))
    .exec()
    .await?;

// Order by age descending with NULLs last
let users = client
    .user()
    .find_many(vec![])
    .order_by((user::age::order(SortOrder::Desc), caustics::NullsOrder::Last))
    .exec()
    .await?;
```

The `NullsOrder` enum provides two options:
- `NullsOrder::First` - NULL values appear first in the result set
- `NullsOrder::Last` - NULL values appear last in the result set

This feature is particularly useful when working with nullable fields where you want to control the positioning of NULL values in your sorted results.

### JSON

Caustics provides comprehensive support for JSON field operations.

### JSON basics

```rust
// Check if JSON field exists
let posts = client.post()
    .find_many(vec![post::custom_data::is_not_null()])
    .exec().await?;

// Exact JSON value matching
let posts = client.post()
    .find_many(vec![post::custom_data::equals(Some(serde_json::json!({
        "category": "technology",
        "priority": "high"
    })))])
    .exec().await?;
```

### JSON path

```rust
// Check if nested JSON path exists
let posts = client.post()
    .find_many(vec![post::custom_data::path(vec![
        "metadata".to_string(),
        "author_notes".to_string()
    ])])
    .exec().await?;
```

### JSON string

```rust
// Search within JSON string values
let posts = client.post()
    .find_many(vec![post::custom_data::json_string_contains("rust".to_string())])
    .exec().await?;

// Pattern matching on JSON strings
let posts = client.post()
    .find_many(vec![post::custom_data::json_string_starts_with("Hello".to_string())])
    .exec().await?;
```

### JSON array

```rust
// Check if array contains a specific value
let posts = client.post()
    .find_many(vec![post::custom_data::json_array_contains(
        serde_json::json!("rust")
    )])
    .exec().await?;

// Check array start/end elements
let posts = client.post()
    .find_many(vec![post::custom_data::json_array_starts_with(
        serde_json::json!("first_item")
    )])
    .exec().await?;
```

### JSON object

```rust
// Check if object contains a specific key
let posts = client.post()
    .find_many(vec![post::custom_data::json_object_contains("category".to_string())])
    .exec().await?;

// Complex JSON queries with logical operators
let posts = client.post()
    .find_many(vec![post::and(vec![
        post::custom_data::is_not_null(),
        post::custom_data::json_object_contains("category".to_string()),
        post::or(vec![
            post::custom_data::json_string_contains("rust".to_string()),
            post::custom_data::json_string_contains("database".to_string()),
        ]),
    ])])
    .exec().await?;

// JSON null helpers
// - db_null(): column IS NULL (DB NULL)
// - json_null(): column = JSON null
// - any_null(): DB NULL OR JSON null
let posts_db_null = client.post()
    .find_many(vec![post::custom_data::db_null()])
    .exec().await?;
let posts_json_null = client.post()
    .find_many(vec![post::custom_data::json_null()])
    .exec().await?;
let posts_any_null = client.post()
    .find_many(vec![post::custom_data::any_null()])
    .exec().await?;
```

## Acknowledgments

This project is inspired by the excellent work done on [Prisma Client Rust](https://github.com/Brendonovich/prisma-client-rust), which provides a type-safe database client for Rust. While Caustics is not derived from Prisma Client Rust, it shares similar design goals of providing an ergonomic, type-safe database interface and is intended to be a drop-in replacement for most of its features.

## License

This project is licensed under the Apache License 2.0 - see the [LICENSE](LICENSE) file for details. 

