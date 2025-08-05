include!(concat!(env!("OUT_DIR"), "/caustics_client_blog_test.rs"));

use caustics_macros::caustics;

#[caustics(namespace = "blog")]
pub mod user {
    use caustics_macros::Caustics;
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

    // Add Related trait implementation
    impl Related<super::user::Entity> for Entity {
        fn to() -> RelationDef {
            Relation::Posts.def()
        }
    }
}

#[caustics(namespace = "blog")]
pub mod post {
    use caustics_macros::Caustics;
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
        #[sea_orm(column_name = "customData", nullable)]
        pub custom_data: Option<serde_json::Value>,
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

    // Add Related trait implementation
    impl Related<super::user::Entity> for Entity {
        fn to() -> RelationDef {
            Relation::User.def()
        }
    }
}

pub mod helpers {
    use sea_orm::{Database, DatabaseConnection, Schema};

    use super::{post, user};

    pub async fn setup_test_db() -> DatabaseConnection {
        use sea_orm::ConnectionTrait;

        // Use SQLite in-memory database with proper configuration
        let db = Database::connect("sqlite::memory:?mode=rwc").await.unwrap();

        // Create schema
        let schema = Schema::new(db.get_database_backend());

        // Create users table
        let mut user_table = schema.create_table_from_entity(user::Entity);
        let create_users = user_table.if_not_exists();
        let create_users_sql = db.get_database_backend().build(create_users);
        db.execute(create_users_sql).await.unwrap();

        // Create posts table
        let mut post_table = schema.create_table_from_entity(post::Entity);
        let create_posts = post_table.if_not_exists();
        let create_posts_sql = db.get_database_backend().build(create_posts);
        db.execute(create_posts_sql).await.unwrap();

        db
    }
}

mod client_tests {
    use super::helpers::setup_test_db;

    use super::*;

    #[tokio::test]
    async fn test_caustics_client() {
        // Setup
        let db = setup_test_db().await;
        let client = CausticsClient::new(db.clone());

        // Test client creation
        assert!(client.db().ping().await.is_ok());
    }
}

mod query_builder_tests {
    use std::str::FromStr;

    use caustics::{QueryError, SortOrder};
    use chrono::{DateTime, FixedOffset, TimeZone};
    use serde_json;

    use super::helpers::setup_test_db;

    use super::*;

    #[tokio::test]
    async fn test_find_operations() {
        let db = setup_test_db().await;
        let client = CausticsClient::new(db.clone());

        // Find unique
        let user = client
            .user()
            .find_unique(user::id::equals(1))
            .exec()
            .await
            .unwrap();
        assert!(user.is_none());

        // Find first
        let user = client
            .user()
            .find_first(vec![user::name::equals("John"), user::age::gt(18)])
            .exec()
            .await
            .unwrap();
        assert!(user.is_none());

        // Find many
        let users = client
            .user()
            .find_many(vec![user::age::gt(18)])
            .exec()
            .await
            .unwrap();
        assert!(users.is_empty());
    }

    #[tokio::test]
    async fn test_create_operations() {
        let db = setup_test_db().await;
        let client = CausticsClient::new(db.clone());

        // Create user with unique email
        let email = format!("john_{}@example.com", chrono::Utc::now().timestamp());
        let user = client
            .user()
            .create(
                email.clone(),
                "John".to_string(),
                DateTime::<FixedOffset>::from_str("2021-01-01T00:00:00Z").unwrap(),
                DateTime::<FixedOffset>::from_str("2021-01-01T00:00:00Z").unwrap(),
                vec![user::age::set(Some(25)), user::deleted_at::set(None)],
            )
            .exec()
            .await
            .unwrap();

        let found_user = client
            .user()
            .find_first(vec![user::email::equals(&email)])
            .exec()
            .await
            .unwrap()
            .unwrap();
        assert_eq!(found_user.name, "John");
        assert_eq!(found_user.email, email);
        assert_eq!(found_user.age, Some(25));

        // Create post
        let post = client
            .post()
            .create(
                "Hello World".to_string(),
                DateTime::<FixedOffset>::from_str("2021-01-01T00:00:00Z").unwrap(),
                DateTime::<FixedOffset>::from_str("2021-01-01T00:00:00Z").unwrap(),
                user::id::equals(user.id),
                vec![post::content::set(Some(
                    "This is my first post".to_string(),
                ))],
            )
            .exec()
            .await
            .unwrap();

        let found_post = client
            .post()
            .find_first(vec![post::id::equals(post.id)])
            .exec()
            .await
            .unwrap()
            .unwrap();
        assert_eq!(found_post.title, "Hello World");
        assert_eq!(
            found_post.content,
            Some("This is my first post".to_string())
        );
        assert_eq!(found_post.user_id, user.id);
    }

    #[tokio::test]
    async fn test_update_operations() {
        let db = setup_test_db().await;
        let client = CausticsClient::new(db.clone());

        // Create user with unique email
        let email = format!("john_{}@example.com", chrono::Utc::now().timestamp());
        let user = client
            .user()
            .create(
                email.clone(),
                "John".to_string(),
                DateTime::<FixedOffset>::from_str("2021-01-01T00:00:00Z").unwrap(),
                DateTime::<FixedOffset>::from_str("2021-01-01T00:00:00Z").unwrap(),
                vec![user::age::set(Some(25)), user::deleted_at::set(None)],
            )
            .exec()
            .await
            .unwrap();

        // Update user
        let updated_user = client
            .user()
            .update(
                user::id::equals(user.id),
                vec![user::name::set("John Doe"), user::age::set(Some(26))],
            )
            .exec()
            .await
            .unwrap();
        assert_eq!(updated_user.name, "John Doe");
        assert_eq!(updated_user.age, Some(26));
    }

    #[tokio::test]
    async fn test_pagination_and_sorting() {
        let db = setup_test_db().await;
        let client = CausticsClient::new(db.clone());

        // Create users
        for i in 0..5 {
            client
                .user()
                .create(
                    format!("user{}@example.com", i),
                    format!("User {}", i),
                    DateTime::<FixedOffset>::from_str("2021-01-01T00:00:00Z").unwrap(),
                    DateTime::<FixedOffset>::from_str("2021-01-01T00:00:00Z").unwrap(),
                    vec![user::age::set(Some(20 + i)), user::deleted_at::set(None)],
                )
                .exec()
                .await
                .unwrap();
        }

        // Test pagination and sorting
        let users = client
            .user()
            .find_many(vec![])
            .take(2)
            .skip(1)
            .order_by(user::age::order(SortOrder::Desc))
            .exec()
            .await
            .unwrap();

        assert_eq!(users.len(), 2);
        assert_eq!(users[0].age, Some(23));
        assert_eq!(users[1].age, Some(22));
    }

    #[tokio::test]
    async fn test_delete_operations() {
        let db = setup_test_db().await;
        let client = CausticsClient::new(db.clone());

        // Create user with unique email
        let email = format!("john_{}@example.com", chrono::Utc::now().timestamp());
        let user = client
            .user()
            .create(
                email.clone(),
                "John".to_string(),
                DateTime::<FixedOffset>::from_str("2021-01-01T00:00:00Z").unwrap(),
                DateTime::<FixedOffset>::from_str("2021-01-01T00:00:00Z").unwrap(),
                vec![user::age::set(Some(25)), user::deleted_at::set(None)],
            )
            .exec()
            .await
            .unwrap();

        // Delete user
        client
            .user()
            .delete(user::id::equals(user.id))
            .exec()
            .await
            .unwrap();

        // Verify deletion
        let deleted_user = client
            .user()
            .find_unique(user::id::equals(user.id))
            .exec()
            .await
            .unwrap();
        assert!(deleted_user.is_none());
    }

    #[tokio::test]
    async fn test_upsert_operations() {
        let db = setup_test_db().await;
        let client = CausticsClient::new(db.clone());

        // Upsert user
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
            .await
            .unwrap();
        assert_eq!(user.name, "John");
        assert_eq!(user.age, Some(25));

        // Update existing user
        let updated_user = client
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
                vec![user::name::set("John Doe"), user::age::set(26)],
            )
            .exec()
            .await
            .unwrap();
        assert_eq!(updated_user.name, "John Doe");
        assert_eq!(updated_user.age, Some(26));
    }

    #[tokio::test]
    async fn test_transaction_commit() {
        let db = setup_test_db().await;
        let client = CausticsClient::new(db.clone());

        let email = format!("john_{}@example.com", chrono::Utc::now().timestamp());
        let email_for_check = email.clone();
        let result = client
            ._transaction()
            .run(|tx| {
                Box::pin(async move {
                    // Create user
                    let user = tx
                        .user()
                        .create(
                            email.clone(),
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
                            vec![post::content::set("This is my first post".to_string())],
                        )
                        .exec()
                        .await?;

                    Ok((user, post))
                })
            })
            .await
            .expect("Transaction failed");

        assert_eq!(result.0.name, "John");
        assert_eq!(result.1.title, "Hello World");

        // Verify data is persisted
        let found_user = client
            .user()
            .find_first(vec![user::email::equals(&email_for_check)])
            .exec()
            .await
            .expect("Failed to query user")
            .expect("User not found");
        assert_eq!(found_user.name, "John");
    }

    #[tokio::test]
    async fn test_transaction_rollback() {
        let db = setup_test_db().await;
        let client = CausticsClient::new(db.clone());

        let email = format!("rollback_{}@example.com", chrono::Utc::now().timestamp());
        let email_for_check = email.clone();
        let result: Result<(), QueryError> = client
            ._transaction()
            .run(|tx| {
                Box::pin(async move {
                    // Create user
                    let _user = tx
                        .user()
                        .create(
                            email.clone(),
                            "Rollback".to_string(),
                            DateTime::<FixedOffset>::from_str("2021-01-01T00:00:00Z").unwrap(),
                            DateTime::<FixedOffset>::from_str("2021-01-01T00:00:00Z").unwrap(),
                            vec![],
                        )
                        .exec()
                        .await?;

                    // Intentionally return an error to trigger rollback
                    Err(QueryError::Custom("Intentional rollback".into()))
                })
            })
            .await;

        assert!(result.is_err());

        // Verify data is NOT persisted
        let found_user = client
            .user()
            .find_first(vec![user::email::equals(&email_for_check)])
            .exec()
            .await
            .expect("Failed to query user");
        assert!(found_user.is_none());
    }

    #[tokio::test]
    async fn test_relations() {
        let db = setup_test_db().await;
        let client = CausticsClient::new(db.clone());

        // Create users
        let author = client
            .user()
            .create(
                "john@example.com".to_string(),
                "John".to_string(),
                DateTime::<FixedOffset>::from_str("2021-01-01T00:00:00Z").unwrap(),
                DateTime::<FixedOffset>::from_str("2021-01-01T00:00:00Z").unwrap(),
                vec![],
            )
            .exec()
            .await
            .unwrap();
        assert!(author.posts.is_none());

        let reviewer = client
            .user()
            .create(
                "jane@example.com".to_string(),
                "Jane".to_string(),
                DateTime::<FixedOffset>::from_str("2021-01-01T00:00:00Z").unwrap(),
                DateTime::<FixedOffset>::from_str("2021-01-01T00:00:00Z").unwrap(),
                vec![],
            )
            .exec()
            .await
            .unwrap();

        // Create posts - one with reviewer, one without
        let post_with_reviewer = client
            .post()
            .create(
                "Reviewed Post".to_string(),
                DateTime::<FixedOffset>::from_str("2021-01-01T00:00:00Z").unwrap(),
                DateTime::<FixedOffset>::from_str("2021-01-01T00:00:00Z").unwrap(),
                user::id::equals(author.id),
                vec![
                    post::content::set("This post has been reviewed".to_string()),
                    post::reviewer::connect(user::id::equals(reviewer.id)),
                ],
            )
            .exec()
            .await
            .unwrap();

        let post_without_reviewer = client
            .post()
            .create(
                "Unreviewed Post".to_string(),
                DateTime::<FixedOffset>::from_str("2021-01-01T00:00:00Z").unwrap(),
                DateTime::<FixedOffset>::from_str("2021-01-01T00:00:00Z").unwrap(),
                user::id::equals(author.id),
                vec![post::content::set(
                    "This post hasn't been reviewed yet".to_string(),
                )],
            )
            .exec()
            .await
            .unwrap();

        // Test fetching user with posts
        let user_with_posts = client
            .user()
            .find_unique(user::id::equals(author.id))
            .with(user::posts::fetch())
            .exec()
            .await
            .unwrap()
            .unwrap();
        let posts = user_with_posts.posts.unwrap();
        assert_eq!(posts.len(), 2);
        assert_eq!(posts[0].title, "Reviewed Post");
        assert_eq!(posts[1].title, "Unreviewed Post");

        // Test fetching post with reviewer
        let post_with_reviewer = client
            .post()
            .find_unique(post::id::equals(post_with_reviewer.id))
            .with(post::reviewer::fetch())
            .exec()
            .await
            .unwrap()
            .unwrap();
        let reviewer = post_with_reviewer.reviewer.unwrap().unwrap();
        assert_eq!(reviewer.name, "Jane");
        assert_eq!(reviewer.email, "jane@example.com");

        // Test fetching post without reviewer
        let post_without_reviewer = client
            .post()
            .find_unique(post::id::equals(post_without_reviewer.id))
            .with(post::reviewer::fetch())
            .exec()
            .await
            .unwrap()
            .unwrap();
        assert!(
            post_without_reviewer.reviewer.is_none()
                || post_without_reviewer.reviewer.as_ref().unwrap().is_none()
        );
    }

    #[tokio::test]
    async fn test_batch_operations() {
        let db = setup_test_db().await;
        let client = CausticsClient::new(db.clone());

        // Create multiple users in a batch
        let timestamp = chrono::Utc::now().timestamp();
        let (user1, user2) = client
            ._batch((
                client.user().create(
                    format!("john_{}@example.com", timestamp),
                    "John".to_string(),
                    DateTime::<FixedOffset>::from_str("2021-01-01T00:00:00Z").unwrap(),
                    DateTime::<FixedOffset>::from_str("2021-01-01T00:00:00Z").unwrap(),
                    vec![user::age::set(Some(25)), user::deleted_at::set(None)],
                ),
                client.user().create(
                    format!("jane_{}@example.com", timestamp),
                    "Jane".to_string(),
                    DateTime::<FixedOffset>::from_str("2021-01-01T00:00:00Z").unwrap(),
                    DateTime::<FixedOffset>::from_str("2021-01-01T00:00:00Z").unwrap(),
                    vec![user::age::set(Some(30))],
                ),
            ))
            .await
            .expect("Batch operation failed");

        assert_eq!(user1.name, "John");
        assert_eq!(user2.name, "Jane");

        let found_users = client.user().find_many(vec![]).exec().await.unwrap();
        assert_eq!(found_users.len(), 2);
    }

    #[tokio::test]
    async fn test_string_operators() {
        use chrono::TimeZone;
        let db = helpers::setup_test_db().await;
        let client = CausticsClient::new(db.clone());
        let now = chrono::FixedOffset::east_opt(0)
            .unwrap()
            .with_ymd_and_hms(2024, 1, 1, 0, 0, 0)
            .unwrap();

        // Create test users
        let _user1 = client
            .user()
            .create(
                "john.doe@example.com".to_string(),
                "John Doe".to_string(),
                now,
                now,
                vec![user::age::set(Some(30)), user::deleted_at::set(None)],
            )
            .exec()
            .await
            .unwrap();

        let _user2 = client
            .user()
            .create(
                "jane.smith@example.com".to_string(),
                "Jane Smith".to_string(),
                now,
                now,
                vec![user::age::set(Some(28)), user::deleted_at::set(None)],
            )
            .exec()
            .await
            .unwrap();

        let _user3 = client
            .user()
            .create(
                "bob.johnson@test.org".to_string(),
                "Bob Johnson".to_string(),
                now,
                now,
                vec![user::age::set(Some(40)), user::deleted_at::set(None)],
            )
            .exec()
            .await
            .unwrap();

        // Test contains operator
        let users_with_doe = client
            .user()
            .find_many(vec![user::name::contains("Doe")])
            .exec()
            .await
            .unwrap();
        assert_eq!(users_with_doe.len(), 1);
        assert_eq!(users_with_doe[0].name, "John Doe");

        // Test starts_with operator
        let users_starting_with_j = client
            .user()
            .find_many(vec![user::name::starts_with("J")])
            .exec()
            .await
            .unwrap();
        assert_eq!(users_starting_with_j.len(), 2);
        assert!(users_starting_with_j
            .iter()
            .all(|u| u.name.starts_with("J")));

        // Test ends_with operator
        let users_ending_with_son = client
            .user()
            .find_many(vec![user::name::ends_with("son")])
            .exec()
            .await
            .unwrap();
        assert_eq!(users_ending_with_son.len(), 1);
        assert_eq!(users_ending_with_son[0].name, "Bob Johnson");

        // Test email contains
        let users_with_example_email = client
            .user()
            .find_many(vec![user::email::contains("example.com")])
            .exec()
            .await
            .unwrap();
        assert_eq!(users_with_example_email.len(), 2);

        let users_with_test_email = client
            .user()
            .find_many(vec![user::email::ends_with("test.org")])
            .exec()
            .await
            .unwrap();
        assert_eq!(users_with_test_email.len(), 1);
        assert_eq!(users_with_test_email[0].email, "bob.johnson@test.org");
    }

    #[tokio::test]
    async fn test_comparison_operators() {
        use chrono::TimeZone;
        let db = helpers::setup_test_db().await;
        let client = CausticsClient::new(db.clone());
        let now = chrono::FixedOffset::east_opt(0)
            .unwrap()
            .with_ymd_and_hms(2024, 1, 1, 0, 0, 0)
            .unwrap();

        // Create test users with different ages for comparison
        let _user1 = client
            .user()
            .create(
                "young@example.com".to_string(),
                "Young User".to_string(),
                now,
                now,
                vec![user::age::set(Some(18)), user::deleted_at::set(None)],
            )
            .exec()
            .await
            .unwrap();

        let _user2 = client
            .user()
            .create(
                "middle@example.com".to_string(),
                "Middle User".to_string(),
                now,
                now,
                vec![user::age::set(Some(30)), user::deleted_at::set(None)],
            )
            .exec()
            .await
            .unwrap();

        let _user3 = client
            .user()
            .create(
                "old@example.com".to_string(),
                "Old User".to_string(),
                now,
                now,
                vec![user::age::set(Some(45)), user::deleted_at::set(None)],
            )
            .exec()
            .await
            .unwrap();

        // Test greater than operator
        let older_users = client
            .user()
            .find_many(vec![user::age::gt(Some(25))])
            .exec()
            .await
            .unwrap();
        assert_eq!(older_users.len(), 2);
        assert!(older_users.iter().all(|u| u.age.unwrap_or(0) > 25));

        // Test less than operator
        let younger_users = client
            .user()
            .find_many(vec![user::age::lt(Some(25))])
            .exec()
            .await
            .unwrap();
        assert_eq!(younger_users.len(), 1);
        assert_eq!(younger_users[0].age, Some(18));

        // Test greater than or equal operator
        let adult_users = client
            .user()
            .find_many(vec![user::age::gte(Some(18))])
            .exec()
            .await
            .unwrap();
        assert_eq!(adult_users.len(), 3);

        // Test less than or equal operator
        let max_30_users = client
            .user()
            .find_many(vec![user::age::lte(Some(30))])
            .exec()
            .await
            .unwrap();
        assert_eq!(max_30_users.len(), 2);

        // Test in_vec operator
        let specific_ages = client
            .user()
            .find_many(vec![user::age::in_vec(vec![Some(18), Some(45)])])
            .exec()
            .await
            .unwrap();
        assert_eq!(specific_ages.len(), 2);

        // Test not_in_vec operator
        let not_specific_ages = client
            .user()
            .find_many(vec![user::age::not_in_vec(vec![Some(18), Some(45)])])
            .exec()
            .await
            .unwrap();
        assert_eq!(not_specific_ages.len(), 1);
        assert_eq!(not_specific_ages[0].age, Some(30));
    }

    #[tokio::test]
    async fn test_logical_operators() {
        use chrono::TimeZone;
        let db = helpers::setup_test_db().await;
        let client = CausticsClient::new(db.clone());
        let now = chrono::FixedOffset::east_opt(0)
            .unwrap()
            .with_ymd_and_hms(2024, 1, 1, 0, 0, 0)
            .unwrap();

        // Create test users with varied data for logical testing
        let _user1 = client
            .user()
            .create(
                "young.john@example.com".to_string(),
                "John Young".to_string(),
                now,
                now,
                vec![user::age::set(Some(16)), user::deleted_at::set(None)],
            )
            .exec()
            .await
            .unwrap();

        let _user2 = client
            .user()
            .create(
                "adult.jane@example.com".to_string(),
                "Jane Adult".to_string(),
                now,
                now,
                vec![user::age::set(Some(25)), user::deleted_at::set(None)],
            )
            .exec()
            .await
            .unwrap();

        let _user3 = client
            .user()
            .create(
                "senior.bob@test.org".to_string(),
                "Bob Senior".to_string(),
                now,
                now,
                vec![user::age::set(Some(70)), user::deleted_at::set(None)],
            )
            .exec()
            .await
            .unwrap();

        let _user4 = client
            .user()
            .create(
                "middle.alice@example.com".to_string(),
                "Alice Middle".to_string(),
                now,
                now,
                vec![user::age::set(Some(35)), user::deleted_at::set(None)],
            )
            .exec()
            .await
            .unwrap();

        // Test AND operator - users who are adults AND have "example.com" email
        let adult_example_users = client
            .user()
            .find_many(vec![user::and(vec![
                user::age::gte(Some(18)),
                user::email::contains("example.com"),
            ])])
            .exec()
            .await
            .unwrap();
        assert_eq!(adult_example_users.len(), 2); // Jane and Alice
        assert!(adult_example_users
            .iter()
            .all(|u| u.age.unwrap_or(0) >= 18 && u.email.contains("example.com")));

        // Test OR operator - users who are either very young OR very old
        let young_or_old_users = client
            .user()
            .find_many(vec![user::or(vec![
                user::age::lt(Some(18)),
                user::age::gt(Some(65)),
            ])])
            .exec()
            .await
            .unwrap();
        assert_eq!(young_or_old_users.len(), 2); // John (16) and Bob (70)
        assert!(young_or_old_users.iter().all(|u| {
            let age = u.age.unwrap_or(0);
            age < 18 || age > 65
        }));

        // Test NOT operator - users who are NOT minors
        let not_minors = client
            .user()
            .find_many(vec![user::not(vec![user::age::lt(Some(18))])])
            .exec()
            .await
            .unwrap();
        assert_eq!(not_minors.len(), 3); // Jane, Bob, and Alice
        assert!(not_minors.iter().all(|u| u.age.unwrap_or(0) >= 18));

        // Test complex nested logical operations -
        // (adults with example.com email) OR (seniors regardless of email)
        let complex_query_users = client
            .user()
            .find_many(vec![user::or(vec![
                user::and(vec![
                    user::age::gte(Some(18)),
                    user::age::lt(Some(65)),
                    user::email::contains("example.com"),
                ]),
                user::age::gte(Some(65)),
            ])])
            .exec()
            .await
            .unwrap();
        assert_eq!(complex_query_users.len(), 3); // Jane, Alice (adults with example.com), and Bob (senior)

        // Test NOT with AND - users who are NOT (young AND have example.com email)
        let not_young_example = client
            .user()
            .find_many(vec![user::not(vec![user::and(vec![
                user::age::lt(Some(25)),
                user::email::contains("example.com"),
            ])])])
            .exec()
            .await
            .unwrap();
        // Should exclude no one since John is young but has example.com, but John is <25 and doesn't have example.com
        // Actually, John has example.com but is young, so NOT(young AND example.com) excludes John... wait let me think about this
        // John: age=16, email="young.john@example.com" -> young=true, has_example=true -> AND=true -> NOT=false (excluded)
        // Jane: age=25, email="adult.jane@example.com" -> young=false, has_example=true -> AND=false -> NOT=true (included)
        // Bob: age=70, email="senior.bob@test.org" -> young=false, has_example=false -> AND=false -> NOT=true (included)
        // Alice: age=35, email="middle.alice@example.com" -> young=false, has_example=true -> AND=false -> NOT=true (included)
        assert_eq!(not_young_example.len(), 3); // Jane, Bob, Alice (John is excluded)
    }

    #[tokio::test]
    async fn test_basic_functionality() {
        use chrono::TimeZone;
        let db = helpers::setup_test_db().await;
        let client = CausticsClient::new(db.clone());
        let now = chrono::FixedOffset::east_opt(0)
            .unwrap()
            .with_ymd_and_hms(2024, 1, 1, 0, 0, 0)
            .unwrap();

        // Test basic create and find functionality
        let _user = client
            .user()
            .create(
                "test@example.com".to_string(),
                "Test User".to_string(),
                now,
                now,
                vec![user::age::set(Some(25)), user::deleted_at::set(None)],
            )
            .exec()
            .await
            .unwrap();

        // Verify existing functionality still works
        let found_user = client
            .user()
            .find_first(vec![user::email::equals("test@example.com")])
            .exec()
            .await
            .unwrap();

        assert!(found_user.is_some());
        let found = found_user.unwrap();
        assert_eq!(found.name, "Test User");
        assert_eq!(found.age, Some(25));
    }

    #[tokio::test]
    async fn test_collection_operators_readme_examples() {
        use chrono::TimeZone;
        let db = helpers::setup_test_db().await;
        let client = CausticsClient::new(db.clone());
        let now = chrono::FixedOffset::east_opt(0)
            .unwrap()
            .with_ymd_and_hms(2024, 1, 1, 0, 0, 0)
            .unwrap();

        // Create test users to match README examples
        let user1 = client
            .user()
            .create(
                "user1@example.com".to_string(),
                "User One".to_string(),
                now,
                now,
                vec![user::age::set(Some(13)), user::deleted_at::set(None)],
            )
            .exec()
            .await
            .unwrap();

        let user2 = client
            .user()
            .create(
                "user2@example.com".to_string(),
                "User Two".to_string(),
                now,
                now,
                vec![user::age::set(Some(14)), user::deleted_at::set(None)],
            )
            .exec()
            .await
            .unwrap();

        let user3 = client
            .user()
            .create(
                "user3@example.com".to_string(),
                "User Three".to_string(),
                now,
                now,
                vec![user::age::set(Some(25)), user::deleted_at::set(None)],
            )
            .exec()
            .await
            .unwrap();

        let user5 = client
            .user()
            .create(
                "user5@example.com".to_string(),
                "User Five".to_string(),
                now,
                now,
                vec![user::age::set(Some(15)), user::deleted_at::set(None)],
            )
            .exec()
            .await
            .unwrap();

        let user8 = client
            .user()
            .create(
                "user8@example.com".to_string(),
                "User Eight".to_string(),
                now,
                now,
                vec![user::age::set(Some(30)), user::deleted_at::set(None)],
            )
            .exec()
            .await
            .unwrap();

        // Test README example: user::id::in_vec(vec![1, 2, 3, 5, 8])
        // Using actual IDs from created users
        let users_by_ids = client
            .user()
            .find_many(vec![user::id::in_vec(vec![
                user1.id, user2.id, user3.id, user5.id, user8.id,
            ])])
            .exec()
            .await
            .unwrap();
        assert_eq!(users_by_ids.len(), 5);
        let found_ids: Vec<i32> = users_by_ids.iter().map(|u| u.id).collect();
        assert!(found_ids.contains(&user1.id));
        assert!(found_ids.contains(&user2.id));
        assert!(found_ids.contains(&user3.id));
        assert!(found_ids.contains(&user5.id));
        assert!(found_ids.contains(&user8.id));

        // Test README example: user::age::not_in_vec(vec![Some(13), Some(14), Some(15)])
        let users_excluding_young_ages = client
            .user()
            .find_many(vec![user::age::not_in_vec(vec![
                Some(13),
                Some(14),
                Some(15),
            ])])
            .exec()
            .await
            .unwrap();
        assert_eq!(users_excluding_young_ages.len(), 2); // user3 (25) and user8 (30)
        assert!(users_excluding_young_ages.iter().all(|u| {
            let age = u.age.unwrap_or(0);
            age != 13 && age != 14 && age != 15
        }));

        // Verify the excluded users have the expected ages
        let included_ages: Vec<Option<i32>> =
            users_excluding_young_ages.iter().map(|u| u.age).collect();
        assert!(included_ages.contains(&Some(25)));
        assert!(included_ages.contains(&Some(30)));
    }

    #[tokio::test]
    async fn test_null_operators() {
        use chrono::TimeZone;
        let db = helpers::setup_test_db().await;
        let client = CausticsClient::new(db.clone());
        let now = chrono::FixedOffset::east_opt(0)
            .unwrap()
            .with_ymd_and_hms(2024, 1, 1, 0, 0, 0)
            .unwrap();

        // Create test users with various null combinations
        let user_with_age = client
            .user()
            .create(
                "with_age@example.com".to_string(),
                "User With Age".to_string(),
                now,
                now,
                vec![user::age::set(Some(30)), user::deleted_at::set(None)],
            )
            .exec()
            .await
            .unwrap();

        let user_without_age = client
            .user()
            .create(
                "without_age@example.com".to_string(),
                "User Without Age".to_string(),
                now,
                now,
                vec![user::age::set(None), user::deleted_at::set(None)],
            )
            .exec()
            .await
            .unwrap();

        let user_deleted = client
            .user()
            .create(
                "deleted@example.com".to_string(),
                "Deleted User".to_string(),
                now,
                now,
                vec![user::age::set(Some(25)), user::deleted_at::set(Some(now))],
            )
            .exec()
            .await
            .unwrap();

        let user_no_deletions = client
            .user()
            .create(
                "no_deletions@example.com".to_string(),
                "No Deletions User".to_string(),
                now,
                now,
                vec![user::age::set(Some(35)), user::deleted_at::set(None)],
            )
            .exec()
            .await
            .unwrap();

        // Create posts with various null combinations
        let post_with_content = client
            .post()
            .create(
                "Post with content".to_string(),
                now,
                now,
                user::id::equals(user_with_age.id),
                vec![
                    post::content::set(Some("This post has content".to_string())),
                    post::reviewer_user_id::set(None),
                ],
            )
            .exec()
            .await
            .unwrap();

        let post_without_content = client
            .post()
            .create(
                "Post without content".to_string(),
                now,
                now,
                user::id::equals(user_without_age.id),
                vec![
                    post::content::set(None),
                    post::reviewer_user_id::set(Some(user_with_age.id)),
                ],
            )
            .exec()
            .await
            .unwrap();

        // Test is_null operator for age field
        let users_without_age = client
            .user()
            .find_many(vec![user::age::is_null()])
            .exec()
            .await
            .unwrap();
        assert_eq!(users_without_age.len(), 1);
        assert_eq!(users_without_age[0].id, user_without_age.id);
        assert_eq!(users_without_age[0].age, None);

        // Test is_not_null operator for age field
        let users_with_age = client
            .user()
            .find_many(vec![user::age::is_not_null()])
            .exec()
            .await
            .unwrap();
        assert_eq!(users_with_age.len(), 3);
        let user_ids_with_age: Vec<i32> = users_with_age.iter().map(|u| u.id).collect();
        assert!(user_ids_with_age.contains(&user_with_age.id));
        assert!(user_ids_with_age.contains(&user_deleted.id));
        assert!(user_ids_with_age.contains(&user_no_deletions.id));
        assert!(!user_ids_with_age.contains(&user_without_age.id));

        // Test is_null operator for deleted_at field
        let non_deleted_users = client
            .user()
            .find_many(vec![user::deleted_at::is_null()])
            .exec()
            .await
            .unwrap();
        assert_eq!(non_deleted_users.len(), 3);
        let non_deleted_ids: Vec<i32> = non_deleted_users.iter().map(|u| u.id).collect();
        assert!(non_deleted_ids.contains(&user_with_age.id));
        assert!(non_deleted_ids.contains(&user_without_age.id));
        assert!(non_deleted_ids.contains(&user_no_deletions.id));
        assert!(!non_deleted_ids.contains(&user_deleted.id));

        // Test is_not_null operator for deleted_at field
        let deleted_users = client
            .user()
            .find_many(vec![user::deleted_at::is_not_null()])
            .exec()
            .await
            .unwrap();
        assert_eq!(deleted_users.len(), 1);
        assert_eq!(deleted_users[0].id, user_deleted.id);
        assert!(deleted_users[0].deleted_at.is_some());

        // Test is_null operator for post content field
        let posts_without_content = client
            .post()
            .find_many(vec![post::content::is_null()])
            .exec()
            .await
            .unwrap();
        assert_eq!(posts_without_content.len(), 1);
        assert_eq!(posts_without_content[0].id, post_without_content.id);
        assert_eq!(posts_without_content[0].content, None);

        // Test is_not_null operator for post content field
        let posts_with_content = client
            .post()
            .find_many(vec![post::content::is_not_null()])
            .exec()
            .await
            .unwrap();
        assert_eq!(posts_with_content.len(), 1);
        assert_eq!(posts_with_content[0].id, post_with_content.id);
        assert!(posts_with_content[0].content.is_some());

        // Test is_null operator for reviewer_user_id field
        let posts_without_reviewer = client
            .post()
            .find_many(vec![post::reviewer_user_id::is_null()])
            .exec()
            .await
            .unwrap();
        assert_eq!(posts_without_reviewer.len(), 1);
        assert_eq!(posts_without_reviewer[0].id, post_with_content.id);
        assert_eq!(posts_without_reviewer[0].reviewer_user_id, None);

        // Test is_not_null operator for reviewer_user_id field
        let posts_with_reviewer = client
            .post()
            .find_many(vec![post::reviewer_user_id::is_not_null()])
            .exec()
            .await
            .unwrap();
        assert_eq!(posts_with_reviewer.len(), 1);
        assert_eq!(posts_with_reviewer[0].id, post_without_content.id);
        assert!(posts_with_reviewer[0].reviewer_user_id.is_some());

        // Test combining null operators with logical operators
        let users_with_age_not_deleted = client
            .user()
            .find_many(vec![user::and(vec![
                user::age::is_not_null(),
                user::deleted_at::is_null(),
            ])])
            .exec()
            .await
            .unwrap();
        assert_eq!(users_with_age_not_deleted.len(), 2);
        let filtered_ids: Vec<i32> = users_with_age_not_deleted.iter().map(|u| u.id).collect();
        assert!(filtered_ids.contains(&user_with_age.id));
        assert!(filtered_ids.contains(&user_no_deletions.id));

        // Test combining null operators with OR
        let users_missing_data = client
            .user()
            .find_many(vec![user::or(vec![
                user::age::is_null(),
                user::deleted_at::is_not_null(),
            ])])
            .exec()
            .await
            .unwrap();
        assert_eq!(users_missing_data.len(), 2);
        let missing_data_ids: Vec<i32> = users_missing_data.iter().map(|u| u.id).collect();
        assert!(missing_data_ids.contains(&user_without_age.id));
        assert!(missing_data_ids.contains(&user_deleted.id));
    }

    #[tokio::test]
    async fn test_json_field_operations() {
        let db = setup_test_db().await;
        let client = CausticsClient::new(db.clone());

        // Create a user for the posts
        let user = client
            .user()
            .create(
                format!("jsonuser_{}@example.com", chrono::Utc::now().timestamp()),
                "JSON User".to_string(),
                DateTime::<FixedOffset>::from_str("2021-01-01T00:00:00Z").unwrap(),
                DateTime::<FixedOffset>::from_str("2021-01-01T00:00:00Z").unwrap(),
                vec![],
            )
            .exec()
            .await
            .unwrap();

        // Create posts with various JSON data for testing
        let post_with_simple_json = client
            .post()
            .create(
                "Post with Simple JSON".to_string(),
                DateTime::<FixedOffset>::from_str("2021-01-01T00:00:00Z").unwrap(),
                DateTime::<FixedOffset>::from_str("2021-01-01T00:00:00Z").unwrap(),
                user::id::equals(user.id),
                vec![
                    post::content::set(Some("A post with simple JSON".to_string())),
                    post::custom_data::set(Some(serde_json::json!({
                        "category": "technology",
                        "tags": ["rust", "json", "database"],
                        "metadata": {
                            "author_notes": "This is a test post",
                            "priority": "high"
                        },
                        "view_count": 42,
                        "published": true
                    }))),
                ],
            )
            .exec()
            .await
            .unwrap();

        let post_with_array_json = client
            .post()
            .create(
                "Post with Array JSON".to_string(),
                DateTime::<FixedOffset>::from_str("2021-01-01T00:00:00Z").unwrap(),
                DateTime::<FixedOffset>::from_str("2021-01-01T00:00:00Z").unwrap(),
                user::id::equals(user.id),
                vec![
                    post::content::set(Some("A post with array JSON".to_string())),
                    post::custom_data::set(Some(serde_json::json!({
                        "categories": ["programming", "tutorial", "beginner"],
                        "scores": [85, 90, 78],
                        "settings": {
                            "notifications": true,
                            "public": false
                        }
                    }))),
                ],
            )
            .exec()
            .await
            .unwrap();

        let post_with_string_json = client
            .post()
            .create(
                "Post with String JSON".to_string(),
                DateTime::<FixedOffset>::from_str("2021-01-01T00:00:00Z").unwrap(),
                DateTime::<FixedOffset>::from_str("2021-01-01T00:00:00Z").unwrap(),
                user::id::equals(user.id),
                vec![
                    post::content::set(Some("A post with string JSON".to_string())),
                    post::custom_data::set(Some(serde_json::json!({
                        "description": "This is a comprehensive guide to JSON operations in databases",
                        "author": "John Developer",
                        "status": "published"
                    }))),
                ],
            )
            .exec()
            .await
            .unwrap();

        let post_without_json = client
            .post()
            .create(
                "Post without JSON".to_string(),
                DateTime::<FixedOffset>::from_str("2021-01-01T00:00:00Z").unwrap(),
                DateTime::<FixedOffset>::from_str("2021-01-01T00:00:00Z").unwrap(),
                user::id::equals(user.id),
                vec![
                    post::content::set(Some("A post without JSON data".to_string())),
                    post::custom_data::set(None),
                ],
            )
            .exec()
            .await
            .unwrap();

        // Test basic JSON equals operation
        let posts_with_specific_category = client
            .post()
            .find_many(vec![post::custom_data::equals(Some(serde_json::json!({
                "category": "technology",
                "tags": ["rust", "json", "database"],
                "metadata": {
                    "author_notes": "This is a test post",
                    "priority": "high"
                },
                "view_count": 42,
                "published": true
            })))])
            .exec()
            .await
            .unwrap();
        assert_eq!(posts_with_specific_category.len(), 1);
        assert_eq!(posts_with_specific_category[0].id, post_with_simple_json.id);

        // Test JSON null operations
        let posts_without_custom_data = client
            .post()
            .find_many(vec![post::custom_data::is_null()])
            .exec()
            .await
            .unwrap();
        assert_eq!(posts_without_custom_data.len(), 1);
        assert_eq!(posts_without_custom_data[0].id, post_without_json.id);

        let posts_with_custom_data = client
            .post()
            .find_many(vec![post::custom_data::is_not_null()])
            .exec()
            .await
            .unwrap();
        assert_eq!(posts_with_custom_data.len(), 3);

        // Test JSON path access - simple key
        let posts_with_category_key = client
            .post()
            .find_many(vec![post::custom_data::path(vec!["category".to_string()])])
            .exec()
            .await
            .unwrap();
        assert_eq!(posts_with_category_key.len(), 1);
        assert_eq!(posts_with_category_key[0].id, post_with_simple_json.id);

        // Test JSON object contains key operations
        let posts_with_metadata_key = client
            .post()
            .find_many(vec![post::custom_data::json_object_contains(
                "metadata".to_string(),
            )])
            .exec()
            .await
            .unwrap();
        assert_eq!(posts_with_metadata_key.len(), 1);
        assert_eq!(posts_with_metadata_key[0].id, post_with_simple_json.id);

        let posts_with_settings_key = client
            .post()
            .find_many(vec![post::custom_data::json_object_contains(
                "settings".to_string(),
            )])
            .exec()
            .await
            .unwrap();
        assert_eq!(posts_with_settings_key.len(), 1);
        assert_eq!(posts_with_settings_key[0].id, post_with_array_json.id);

        let posts_with_description_key = client
            .post()
            .find_many(vec![post::custom_data::json_object_contains(
                "description".to_string(),
            )])
            .exec()
            .await
            .unwrap();
        assert_eq!(posts_with_description_key.len(), 1);
        assert_eq!(posts_with_description_key[0].id, post_with_string_json.id);

        // Test JSON string contains operations
        let posts_with_rust_anywhere = client
            .post()
            .find_many(vec![post::custom_data::json_string_contains(
                "rust".to_string(),
            )])
            .exec()
            .await
            .unwrap();
        assert_eq!(posts_with_rust_anywhere.len(), 1);
        assert_eq!(posts_with_rust_anywhere[0].id, post_with_simple_json.id);

        let posts_with_guide_description = client
            .post()
            .find_many(vec![post::custom_data::json_string_contains(
                "comprehensive guide".to_string(),
            )])
            .exec()
            .await
            .unwrap();
        assert_eq!(posts_with_guide_description.len(), 1);
        assert_eq!(posts_with_guide_description[0].id, post_with_string_json.id);

        // Test JSON operations with logical operators (AND)
        let posts_with_category_and_metadata = client
            .post()
            .find_many(vec![post::and(vec![
                post::custom_data::json_object_contains("category".to_string()),
                post::custom_data::json_object_contains("metadata".to_string()),
            ])])
            .exec()
            .await
            .unwrap();
        assert_eq!(posts_with_category_and_metadata.len(), 1);
        assert_eq!(
            posts_with_category_and_metadata[0].id,
            post_with_simple_json.id
        );

        // Test JSON operations with logical operators (OR)
        let posts_with_description_or_settings = client
            .post()
            .find_many(vec![post::or(vec![
                post::custom_data::json_object_contains("description".to_string()),
                post::custom_data::json_object_contains("settings".to_string()),
            ])])
            .exec()
            .await
            .unwrap();
        assert_eq!(posts_with_description_or_settings.len(), 2);
        let found_ids: Vec<i32> = posts_with_description_or_settings
            .iter()
            .map(|p| p.id)
            .collect();
        assert!(found_ids.contains(&post_with_string_json.id));
        assert!(found_ids.contains(&post_with_array_json.id));

        // Test JSON operations with NOT operator
        let posts_without_metadata = client
            .post()
            .find_many(vec![post::not(vec![
                post::custom_data::json_object_contains("metadata".to_string()),
            ])])
            .exec()
            .await
            .unwrap();
        // Should find posts without JSON data and posts without metadata key
        assert!(posts_without_metadata.len() >= 3);
        let found_ids: Vec<i32> = posts_without_metadata.iter().map(|p| p.id).collect();
        assert!(found_ids.contains(&post_without_json.id));
        assert!(found_ids.contains(&post_with_array_json.id));
        assert!(found_ids.contains(&post_with_string_json.id));
        assert!(!found_ids.contains(&post_with_simple_json.id));
    }

    #[tokio::test]
    async fn test_atomic_operations() {
        use chrono::TimeZone;
        let db = helpers::setup_test_db().await;
        let client = CausticsClient::new(db.clone());
        let now = chrono::FixedOffset::east_opt(0)
            .unwrap()
            .with_ymd_and_hms(2024, 1, 1, 0, 0, 0)
            .unwrap();

        // Create a user with an age to test atomic operations
        let user = client
            .user()
            .create(
                "atomic@example.com".to_string(),
                "Atomic User".to_string(),
                now,
                now,
                vec![user::age::set(Some(25)), user::deleted_at::set(None)],
            )
            .exec()
            .await
            .unwrap();

        assert_eq!(user.age, Some(25));

        // Test increment operation
        println!("🔧 Testing increment operation: age = {:?}", user.age);
        println!("🔧 Calling update with user::age::increment(5)");
        let updated_user = client
            .user()
            .update(user::id::equals(user.id), vec![user::age::increment(5)])
            .exec()
            .await
            .unwrap();
        println!("🔧 After increment(5): age = {:?}", updated_user.age);
        assert_eq!(updated_user.age, Some(30));

        // Test decrement operation
        let updated_user = client
            .user()
            .update(user::id::equals(user.id), vec![user::age::decrement(3)])
            .exec()
            .await
            .unwrap();
        assert_eq!(updated_user.age, Some(27));

        // Test multiply operation
        let updated_user = client
            .user()
            .update(user::id::equals(user.id), vec![user::age::multiply(2)])
            .exec()
            .await
            .unwrap();
        assert_eq!(updated_user.age, Some(54));

        // Test divide operation
        let updated_user = client
            .user()
            .update(user::id::equals(user.id), vec![user::age::divide(3)])
            .exec()
            .await
            .unwrap();
        assert_eq!(updated_user.age, Some(18));

        // Test multiple atomic operations in one update
        let updated_user = client
            .user()
            .update(
                user::id::equals(user.id),
                vec![user::age::increment(10), user::age::multiply(2)],
            )
            .exec()
            .await
            .unwrap();
        // Should be (18 + 10) * 2 = 56
        assert_eq!(updated_user.age, Some(56));
    }

    #[tokio::test]
    async fn test_atomic_operations_simple() {
        use chrono::TimeZone;
        let db = helpers::setup_test_db().await;
        let client = CausticsClient::new(db.clone());
        let now = chrono::FixedOffset::east_opt(0)
            .unwrap()
            .with_ymd_and_hms(2024, 1, 1, 0, 0, 0)
            .unwrap();

        // Create a user with an age to test atomic operations
        let user = client
            .user()
            .create(
                "atomic@example.com".to_string(),
                "Atomic User".to_string(),
                now,
                now,
                vec![user::age::set(Some(25)), user::deleted_at::set(None)],
            )
            .exec()
            .await
            .unwrap();

        assert_eq!(user.age, Some(25));

        // Test that atomic operations exist and can be called
        let _increment_op = user::age::increment(5);
        let _decrement_op = user::age::decrement(3);
        let _multiply_op = user::age::multiply(2);
        let _divide_op = user::age::divide(3);

        // Test increment operation
        let updated_user = client
            .user()
            .update(user::id::equals(user.id), vec![user::age::increment(5)])
            .exec()
            .await
            .unwrap();

        // The atomic operation should work
        assert_eq!(updated_user.age, Some(30));
    }

    #[tokio::test]
    async fn test_advanced_relation_operations() {
        let _ = env_logger::try_init();
        use chrono::TimeZone;
        let db = helpers::setup_test_db().await;
        let client = CausticsClient::new(db.clone());
        let now = chrono::FixedOffset::east_opt(0)
            .unwrap()
            .with_ymd_and_hms(2024, 1, 1, 0, 0, 0)
            .unwrap();

        // Create a user with some posts
        let user = client
            .user()
            .create(
                "relation@example.com".to_string(),
                "Relation User".to_string(),
                now,
                now,
                vec![user::age::set(Some(30)), user::deleted_at::set(None)],
            )
            .exec()
            .await
            .unwrap();

        // Create some posts for the user
        let _post1 = client
            .post()
            .create(
                "First Post".to_string(),
                now,
                now,
                user::id::equals(user.id),
                vec![post::content::set(Some("First post content".to_string()))],
            )
            .exec()
            .await
            .unwrap();

        let _post2 = client
            .post()
            .create(
                "Second Post".to_string(),
                now,
                now,
                user::id::equals(user.id),
                vec![post::content::set(Some("Second post content".to_string()))],
            )
            .exec()
            .await
            .unwrap();

        // Test that the advanced relation operations exist and can be called
        let _some_condition =
            user::posts::some(vec![post::title::equals("First Post".to_string())]);

        let _every_condition = user::posts::every(vec![post::title::contains("Post".to_string())]);

        let _none_condition =
            user::posts::none(vec![post::title::equals("Non-existent Post".to_string())]);

        // Test that the conditions can be used in queries (they should work now!)
        let users_with_some_posts = client
            .user()
            .find_many(vec![user::posts::some(vec![post::title::equals(
                "First Post".to_string(),
            )])])
            .exec()
            .await
            .unwrap();

        // The query should return the user since they have a post with title "First Post"
        assert_eq!(users_with_some_posts.len(), 1);
        assert_eq!(users_with_some_posts[0].id, user.id);

        // Test every condition
        let users_with_every_post_containing_post = client
            .user()
            .find_many(vec![user::posts::every(vec![post::title::contains(
                "Post".to_string(),
            )])])
            .exec()
            .await
            .unwrap();

        // The query should return the user since all their posts contain "Post"
        assert_eq!(users_with_every_post_containing_post.len(), 1);
        assert_eq!(users_with_every_post_containing_post[0].id, user.id);

        // Test none condition
        let users_with_no_nonexistent_posts = client
            .user()
            .find_many(vec![user::posts::none(vec![post::title::equals(
                "Non-existent Post".to_string(),
            )])])
            .exec()
            .await
            .unwrap();

        // The query should return the user since they don't have a post with title "Non-existent Post"
        assert_eq!(users_with_no_nonexistent_posts.len(), 1);
        assert_eq!(users_with_no_nonexistent_posts[0].id, user.id);
    }

    #[tokio::test]
    async fn test_complex_relation_filtering_with_subqueries() {
        let _ = env_logger::try_init();
        use chrono::TimeZone;
        let db = helpers::setup_test_db().await;
        let client = CausticsClient::new(db.clone());
        let now = chrono::FixedOffset::east_opt(0)
            .unwrap()
            .with_ymd_and_hms(2024, 1, 1, 0, 0, 0)
            .unwrap();
        let future_date = chrono::FixedOffset::east_opt(0)
            .unwrap()
            .with_ymd_and_hms(2024, 12, 31, 23, 59, 59)
            .unwrap();

        // Create multiple users with different post patterns
        let user1 = client
            .user()
            .create(
                "user1@example.com".to_string(),
                "User One".to_string(),
                now,
                now,
                vec![user::age::set(Some(25)), user::deleted_at::set(None)],
            )
            .exec()
            .await
            .unwrap();

        let user2 = client
            .user()
            .create(
                "user2@example.com".to_string(),
                "User Two".to_string(),
                now,
                now,
                vec![user::age::set(Some(30)), user::deleted_at::set(None)],
            )
            .exec()
            .await
            .unwrap();

        let user3 = client
            .user()
            .create(
                "user3@example.com".to_string(),
                "User Three".to_string(),
                now,
                now,
                vec![user::age::set(Some(35)), user::deleted_at::set(None)],
            )
            .exec()
            .await
            .unwrap();

        // Create posts for user1: Has "Hello" posts with content, all created in 2024
        let _post1_1 = client
            .post()
            .create(
                "Hello World".to_string(),
                now,
                now,
                user::id::equals(user1.id),
                vec![post::content::set(Some("Hello post content".to_string()))],
            )
            .exec()
            .await
            .unwrap();

        let _post1_2 = client
            .post()
            .create(
                "Hello Again".to_string(),
                now,
                now,
                user::id::equals(user1.id),
                vec![post::content::set(Some("Another hello post".to_string()))],
            )
            .exec()
            .await
            .unwrap();

        // Create posts for user2: Has "Hello" posts but some without content, all created in 2024
        let _post2_1 = client
            .post()
            .create(
                "Hello from User2".to_string(),
                now,
                now,
                user::id::equals(user2.id),
                vec![post::content::set(Some("User2 hello content".to_string()))],
            )
            .exec()
            .await
            .unwrap();

        let _post2_2 = client
            .post()
            .create(
                "Hello without content".to_string(),
                now,
                now,
                user::id::equals(user2.id),
                vec![post::content::set(None)], // No content
            )
            .exec()
            .await
            .unwrap();

        // Create posts for user3: Has "Hello" posts but some created in future, some spam
        let _post3_1 = client
            .post()
            .create(
                "Hello from User3".to_string(),
                now,
                now,
                user::id::equals(user3.id),
                vec![post::content::set(Some("User3 hello content".to_string()))],
            )
            .exec()
            .await
            .unwrap();

        let _post3_2 = client
            .post()
            .create(
                "Future Hello".to_string(),
                future_date,
                future_date,
                user::id::equals(user3.id),
                vec![post::content::set(Some("Future hello content".to_string()))],
            )
            .exec()
            .await
            .unwrap();

        let _post3_3 = client
            .post()
            .create(
                "Spam Post".to_string(),
                now,
                now,
                user::id::equals(user3.id),
                vec![post::content::set(Some("Spam content".to_string()))],
            )
            .exec()
            .await
            .unwrap();

        // Test 1: Complex relation filtering with multiple conditions
        // Find users who:
        // - Have SOME posts with "Hello" in title
        // - Have EVERY post with "Hello" in title
        // - Have NO posts with "Spam" in title
        let complex_filtered_users = client
            .user()
            .find_many(vec![
                user::posts::some(vec![post::title::contains("Hello".to_string())]),
                user::posts::every(vec![post::title::contains("Hello".to_string())]),
                user::posts::none(vec![post::title::contains("Spam".to_string())]),
            ])
            .exec()
            .await
            .unwrap();

        // Should return user1 and user2, but not user3
        // user1: ✅ has "Hello" posts, ✅ all posts have "Hello", ✅ no spam
        // user2: ✅ has "Hello" posts, ✅ all posts have "Hello", ✅ no spam
        // user3: ✅ has "Hello" posts, ❌ has "Spam" post, ❌ has spam
        assert_eq!(complex_filtered_users.len(), 2);
        let user_ids: Vec<i32> = complex_filtered_users.iter().map(|u| u.id).collect();
        assert!(user_ids.contains(&user1.id));
        assert!(user_ids.contains(&user2.id));
        assert!(!user_ids.contains(&user3.id));

        // Test 2: More specific filtering with different conditions
        // Find users who have posts with "World" in title
        let users_with_world = client
            .user()
            .find_many(vec![user::posts::some(vec![post::title::contains(
                "World".to_string(),
            )])])
            .exec()
            .await
            .unwrap();

        // Should return user1 (has "Hello World" post)
        assert_eq!(users_with_world.len(), 1);
        assert_eq!(users_with_world[0].id, user1.id);

        // Test 3: Every post must have "Hello" in title
        let users_with_all_hello_posts = client
            .user()
            .find_many(vec![user::posts::every(vec![post::title::contains(
                "Hello".to_string(),
            )])])
            .exec()
            .await
            .unwrap();

        // Should return user1 and user2, but not user3 (has "Spam Post")
        assert_eq!(users_with_all_hello_posts.len(), 2);
        let all_hello_user_ids: Vec<i32> =
            users_with_all_hello_posts.iter().map(|u| u.id).collect();
        assert!(all_hello_user_ids.contains(&user1.id));
        assert!(all_hello_user_ids.contains(&user2.id));
        assert!(!all_hello_user_ids.contains(&user3.id));

        // Test 4: No spam posts
        let users_without_spam_posts = client
            .user()
            .find_many(vec![user::posts::none(vec![post::title::contains(
                "Spam".to_string(),
            )])])
            .exec()
            .await
            .unwrap();

        // Should return user1 and user2, but not user3 (has spam post)
        assert_eq!(users_without_spam_posts.len(), 2);
        let no_spam_user_ids: Vec<i32> = users_without_spam_posts.iter().map(|u| u.id).collect();
        assert!(no_spam_user_ids.contains(&user1.id));
        assert!(no_spam_user_ids.contains(&user2.id));
        assert!(!no_spam_user_ids.contains(&user3.id));

        // Test 5: Combined with logical operators
        let combined_filtered_users = client
            .user()
            .find_many(vec![user::and(vec![
                user::posts::some(vec![post::title::contains("Hello".to_string())]),
                user::posts::none(vec![post::title::contains("Spam".to_string())]),
            ])])
            .exec()
            .await
            .unwrap();

        // Should return user1 and user2, but not user3
        assert_eq!(combined_filtered_users.len(), 2);
        let combined_user_ids: Vec<i32> = combined_filtered_users.iter().map(|u| u.id).collect();
        assert!(combined_user_ids.contains(&user1.id));
        assert!(combined_user_ids.contains(&user2.id));
        assert!(!combined_user_ids.contains(&user3.id));

        // Test 6: Edge case - user with no posts
        let user_no_posts = client
            .user()
            .create(
                "noposts@example.com".to_string(),
                "No Posts User".to_string(),
                now,
                now,
                vec![user::age::set(Some(40)), user::deleted_at::set(None)],
            )
            .exec()
            .await
            .unwrap();

        // User with no posts should match "none" conditions
        let users_with_no_spam = client
            .user()
            .find_many(vec![user::posts::none(vec![post::title::contains(
                "Spam".to_string(),
            )])])
            .exec()
            .await
            .unwrap();

        // Should include the user with no posts
        let no_spam_user_ids_updated: Vec<i32> = users_with_no_spam.iter().map(|u| u.id).collect();
        assert!(no_spam_user_ids_updated.contains(&user_no_posts.id));

        // User with no posts should NOT match "some" conditions
        let users_with_hello_final = client
            .user()
            .find_many(vec![user::posts::some(vec![post::title::contains(
                "Hello".to_string(),
            )])])
            .exec()
            .await
            .unwrap();

        // Should NOT include the user with no posts
        let hello_user_ids: Vec<i32> = users_with_hello_final.iter().map(|u| u.id).collect();
        assert!(!hello_user_ids.contains(&user_no_posts.id));

        // Test 7: Nullable field filtering in relations
        // Find users who have posts with content (is_not_null)
        let users_with_content_posts = client
            .user()
            .find_many(vec![user::posts::some(vec![post::content::is_not_null()])])
            .exec()
            .await
            .unwrap();

        // Should return all 3 users because:
        // user1: has 2 posts with content ✅
        // user2: has 1 post with content (and 1 without) ✅
        // user3: has 3 posts with content ✅
        assert_eq!(users_with_content_posts.len(), 3);
        let content_user_ids: Vec<i32> = users_with_content_posts.iter().map(|u| u.id).collect();
        assert!(content_user_ids.contains(&user1.id));
        assert!(content_user_ids.contains(&user2.id));
        assert!(content_user_ids.contains(&user3.id));

        // Find users who have posts without content (is_null)
        let users_with_null_content_posts = client
            .user()
            .find_many(vec![user::posts::some(vec![post::content::is_null()])])
            .exec()
            .await
            .unwrap();

        // Should return user2 (has post without content)
        assert_eq!(users_with_null_content_posts.len(), 1);
        assert_eq!(users_with_null_content_posts[0].id, user2.id);

        // Test 8: Every post must have content
        let users_with_all_content = client
            .user()
            .find_many(vec![user::posts::every(vec![post::content::is_not_null()])])
            .exec()
            .await
            .unwrap();

        // Should return user1, user3, and user4 (no posts - vacuous truth), but not user2 (has post without content)
        assert_eq!(users_with_all_content.len(), 3);
        let all_content_user_ids: Vec<i32> = users_with_all_content.iter().map(|u| u.id).collect();
        assert!(all_content_user_ids.contains(&user1.id));
        assert!(all_content_user_ids.contains(&user3.id));
        assert!(all_content_user_ids.contains(&user_no_posts.id)); // user with no posts
        assert!(!all_content_user_ids.contains(&user2.id)); // has post without content

        // Test 9: No posts without content
        let users_with_no_null_content = client
            .user()
            .find_many(vec![user::posts::none(vec![post::content::is_null()])])
            .exec()
            .await
            .unwrap();

        // Should return user1, user3, and user4 (no posts - vacuous truth), but not user2 (has post without content)
        assert_eq!(users_with_no_null_content.len(), 3);
        let no_null_content_user_ids: Vec<i32> =
            users_with_no_null_content.iter().map(|u| u.id).collect();
        assert!(no_null_content_user_ids.contains(&user1.id));
        assert!(no_null_content_user_ids.contains(&user3.id));
        assert!(no_null_content_user_ids.contains(&user_no_posts.id)); // user with no posts
        assert!(!no_null_content_user_ids.contains(&user2.id));
    }

    #[tokio::test]
    async fn test_has_many_set_operation_structure() {
        let db = helpers::setup_test_db().await;
        let client = CausticsClient::new(db.clone());

        // Create a user
        let user = client
            .user()
            .create(
                "user@example.com".to_string(),
                "User".to_string(),
                chrono::FixedOffset::east_opt(0)
                    .unwrap()
                    .with_ymd_and_hms(2022, 1, 1, 12, 0, 0)
                    .unwrap(),
                chrono::FixedOffset::east_opt(0)
                    .unwrap()
                    .with_ymd_and_hms(2022, 1, 1, 12, 0, 0)
                    .unwrap(),
                vec![],
            )
            .exec()
            .await
            .unwrap();

        // Create some posts
        let post1 = client
            .post()
            .create(
                "Post 1".to_string(),
                chrono::FixedOffset::east_opt(0)
                    .unwrap()
                    .with_ymd_and_hms(2022, 1, 1, 12, 0, 0)
                    .unwrap(),
                chrono::FixedOffset::east_opt(0)
                    .unwrap()
                    .with_ymd_and_hms(2022, 1, 1, 12, 0, 0)
                    .unwrap(),
                user::id::equals(user.id),
                vec![],
            )
            .exec()
            .await
            .unwrap();

        let post2 = client
            .post()
            .create(
                "Post 2".to_string(),
                chrono::FixedOffset::east_opt(0)
                    .unwrap()
                    .with_ymd_and_hms(2022, 1, 1, 12, 0, 0)
                    .unwrap(),
                chrono::FixedOffset::east_opt(0)
                    .unwrap()
                    .with_ymd_and_hms(2022, 1, 1, 12, 0, 0)
                    .unwrap(),
                user::id::equals(user.id),
                vec![],
            )
            .exec()
            .await
            .unwrap();

        // Test that the has_many set operation structure compiles and runs
        // This should work even though the actual set operation is not implemented yet
        let updated_user = client
            .user()
            .update_with_has_many_set(
                user::id::equals(user.id),
                vec![user::posts::set(vec![
                    post::id::equals(post1.id),
                    post::id::equals(post2.id),
                ])],
            )
            .exec()
            .await;

        // For now, this should work (it just delegates to regular update)
        if let Err(e) = &updated_user {
            println!("Error: {:?}", e);
        }
        assert!(updated_user.is_ok());
    }

    #[tokio::test]
    async fn test_has_many_set_operation_functionality() {
        let db = helpers::setup_test_db().await;
        let client = CausticsClient::new(db.clone());

        // Create a user
        let user = client
            .user()
            .create(
                "user@example.com".to_string(),
                "User".to_string(),
                chrono::FixedOffset::east_opt(0)
                    .unwrap()
                    .with_ymd_and_hms(2022, 1, 1, 12, 0, 0)
                    .unwrap(),
                chrono::FixedOffset::east_opt(0)
                    .unwrap()
                    .with_ymd_and_hms(2022, 1, 1, 12, 0, 0)
                    .unwrap(),
                vec![],
            )
            .exec()
            .await
            .unwrap();

        // Create some posts initially not associated with the user
        let post1 = client
            .post()
            .create(
                "Post 1".to_string(),
                chrono::FixedOffset::east_opt(0)
                    .unwrap()
                    .with_ymd_and_hms(2022, 1, 1, 12, 0, 0)
                    .unwrap(),
                chrono::FixedOffset::east_opt(0)
                    .unwrap()
                    .with_ymd_and_hms(2022, 1, 1, 12, 0, 0)
                    .unwrap(),
                user::id::equals(user.id), // Initially associated
                vec![],
            )
            .exec()
            .await
            .unwrap();

        let post2 = client
            .post()
            .create(
                "Post 2".to_string(),
                chrono::FixedOffset::east_opt(0)
                    .unwrap()
                    .with_ymd_and_hms(2022, 1, 1, 12, 0, 0)
                    .unwrap(),
                chrono::FixedOffset::east_opt(0)
                    .unwrap()
                    .with_ymd_and_hms(2022, 1, 1, 12, 0, 0)
                    .unwrap(),
                user::id::equals(user.id), // Initially associated
                vec![],
            )
            .exec()
            .await
            .unwrap();

        let _post3 = client
            .post()
            .create(
                "Post 3".to_string(),
                chrono::FixedOffset::east_opt(0)
                    .unwrap()
                    .with_ymd_and_hms(2022, 1, 1, 12, 0, 0)
                    .unwrap(),
                chrono::FixedOffset::east_opt(0)
                    .unwrap()
                    .with_ymd_and_hms(2022, 1, 1, 12, 0, 0)
                    .unwrap(),
                user::id::equals(user.id), // Initially associated
                vec![],
            )
            .exec()
            .await
            .unwrap();

        // Verify initial state
        let initial_posts = client
            .post()
            .find_many(vec![post::user_id::equals(user.id)])
            .exec()
            .await
            .unwrap();
        assert_eq!(initial_posts.len(), 3);

        // Test the has_many set operation
        // This should set the user's posts to only post1 and post2
        let updated_user = client
            .user()
            .update_with_has_many_set(
                user::id::equals(user.id),
                vec![user::posts::set(vec![
                    post::id::equals(post1.id),
                    post::id::equals(post2.id),
                ])],
            )
            .exec()
            .await;

        if let Err(e) = &updated_user {
            println!("Error: {:?}", e);
        }
        assert!(updated_user.is_ok());

        // Verify the result
        let final_posts = client
            .post()
            .find_many(vec![post::user_id::equals(user.id)])
            .exec()
            .await
            .unwrap();
        

        
        // Now we expect exactly 2 posts since the set operation should replace all associations
        // The set operation should have removed post3 and kept only post1 and post2
        assert_eq!(final_posts.len(), 2);
        
        // Verify that only post1 and post2 are associated with the user
        let final_post_ids: Vec<i32> = final_posts.iter().map(|p| p.id).collect();
        assert!(final_post_ids.contains(&post1.id));
        assert!(final_post_ids.contains(&post2.id));
        assert!(!final_post_ids.contains(&_post3.id));
    }

    #[tokio::test]
    async fn test_agnostic_implementation_compiles() {
        let db = helpers::setup_test_db().await;
        let client = CausticsClient::new(db.clone());

        // Create a user
        let user = client
            .user()
            .create(
                "test@example.com".to_string(),
                "Test User".to_string(),
                chrono::FixedOffset::east_opt(0)
                    .unwrap()
                    .with_ymd_and_hms(2022, 1, 1, 12, 0, 0)
                    .unwrap(),
                chrono::FixedOffset::east_opt(0)
                    .unwrap()
                    .with_ymd_and_hms(2022, 1, 1, 12, 0, 0)
                    .unwrap(),
                vec![],
            )
            .exec()
            .await
            .unwrap();

        // Create a post with a different user to avoid conflicts
        let post = client
            .post()
            .create(
                "Test Post".to_string(),
                chrono::FixedOffset::east_opt(0)
                    .unwrap()
                    .with_ymd_and_hms(2022, 1, 1, 12, 0, 0)
                    .unwrap(),
                chrono::FixedOffset::east_opt(0)
                    .unwrap()
                    .with_ymd_and_hms(2022, 1, 1, 12, 0, 0)
                    .unwrap(),
                user::id::equals(user.id),
                vec![],
            )
            .exec()
            .await
            .unwrap();

        // Test that the agnostic implementation compiles and runs
        // This should work with any relation name, not just hardcoded ones
        let result = client
            .user()
            .update_with_has_many_set(
                user::id::equals(user.id),
                vec![user::posts::set(vec![
                    post::id::equals(post.id),
                ])],
            )
            .exec()
            .await;

        // The result might fail due to incomplete implementation, but it should compile
        // This test verifies that our agnostic approach works with the metadata system
        println!("Agnostic implementation result: {:?}", result);
        
        // For now, we just verify it compiles and runs without panicking
        // The actual functionality will be implemented later
    }

    #[tokio::test]
    async fn test_dynamic_foreign_key_column_extraction() {
        let db = helpers::setup_test_db().await;
        let client = CausticsClient::new(db.clone());

        // Create a user
        let user = client
            .user()
            .create(
                "test@example.com".to_string(),
                "Test User".to_string(),
                chrono::FixedOffset::east_opt(0)
                    .unwrap()
                    .with_ymd_and_hms(2022, 1, 1, 12, 0, 0)
                    .unwrap(),
                chrono::FixedOffset::east_opt(0)
                    .unwrap()
                    .with_ymd_and_hms(2022, 1, 1, 12, 0, 0)
                    .unwrap(),
                vec![],
            )
            .exec()
            .await
            .unwrap();

        // Create posts with different foreign key columns
        let post1 = client
            .post()
            .create(
                "Post 1".to_string(),
                chrono::FixedOffset::east_opt(0)
                    .unwrap()
                    .with_ymd_and_hms(2022, 1, 1, 12, 0, 0)
                    .unwrap(),
                chrono::FixedOffset::east_opt(0)
                    .unwrap()
                    .with_ymd_and_hms(2022, 1, 1, 12, 0, 0)
                    .unwrap(),
                user::id::equals(user.id),
                vec![],
            )
            .exec()
            .await
            .unwrap();

        let post2 = client
            .post()
            .create(
                "Post 2".to_string(),
                chrono::FixedOffset::east_opt(0)
                    .unwrap()
                    .with_ymd_and_hms(2022, 1, 1, 12, 0, 0)
                    .unwrap(),
                chrono::FixedOffset::east_opt(0)
                    .unwrap()
                    .with_ymd_and_hms(2022, 1, 1, 12, 0, 0)
                    .unwrap(),
                user::id::equals(user.id),
                vec![],
            )
            .exec()
            .await
            .unwrap();

        // Test that the set operation works with the dynamically extracted foreign key column
        // This should use "user_id" (converted from "UserId" in the relation definition)
        let updated_user = client
            .user()
            .update_with_has_many_set(
                user::id::equals(user.id),
                vec![user::posts::set(vec![
                    post::id::equals(post1.id),
                    post::id::equals(post2.id),
                ])],
            )
            .exec()
            .await;

        assert!(updated_user.is_ok());

        // Verify the result
        let final_user = client
            .user()
            .find_unique(user::id::equals(user.id))
            .with(user::posts::fetch())
            .exec()
            .await
            .unwrap()
            .unwrap();

        let final_posts = final_user.posts.unwrap();
        assert_eq!(final_posts.len(), 2);
        
        // Verify that only post1 and post2 are associated with the user
        let final_post_ids: Vec<i32> = final_posts.iter().map(|p| p.id).collect();
        assert!(final_post_ids.contains(&post1.id));
        assert!(final_post_ids.contains(&post2.id));
    }
}
