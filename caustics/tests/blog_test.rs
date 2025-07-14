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
    use chrono::{DateTime, FixedOffset};

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
                user::email::equals(author.email),
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
                vec![
                    post::content::set("This post hasn't been reviewed yet".to_string()),
                ],
            )
            .exec()
            .await
            .unwrap();

        // Test fetching user with posts
        let user_with_posts = client
            .user()
            .find_unique(user::id::equals(author.id))
            .with(user::posts::fetch(vec![]))
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
        assert!(post_without_reviewer.reviewer.is_none() || post_without_reviewer.reviewer.as_ref().unwrap().is_none());
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
        let now = chrono::FixedOffset::east_opt(0).unwrap().with_ymd_and_hms(2024, 1, 1, 0, 0, 0).unwrap();

        // Create test users
        let _user1 = client.user().create(
            "john.doe@example.com".to_string(),
            "John Doe".to_string(),
            now,
            now,
            vec![user::age::set(Some(30)), user::deleted_at::set(None)],
        ).exec().await.unwrap();

        let _user2 = client.user().create(
            "jane.smith@example.com".to_string(),
            "Jane Smith".to_string(),
            now,
            now,
            vec![user::age::set(Some(28)), user::deleted_at::set(None)],
        ).exec().await.unwrap();

        let _user3 = client.user().create(
            "bob.johnson@test.org".to_string(),
            "Bob Johnson".to_string(),
            now,
            now,
            vec![user::age::set(Some(40)), user::deleted_at::set(None)],
        ).exec().await.unwrap();

        // Test contains operator
        let users_with_doe = client.user()
            .find_many(vec![user::name::contains("Doe")])
            .exec()
            .await
            .unwrap();
        assert_eq!(users_with_doe.len(), 1);
        assert_eq!(users_with_doe[0].name, "John Doe");

        // Test starts_with operator
        let users_starting_with_j = client.user()
            .find_many(vec![user::name::starts_with("J")])
            .exec()
            .await
            .unwrap();
        assert_eq!(users_starting_with_j.len(), 2);
        assert!(users_starting_with_j.iter().all(|u| u.name.starts_with("J")));

        // Test ends_with operator
        let users_ending_with_son = client.user()
            .find_many(vec![user::name::ends_with("son")])
            .exec()
            .await
            .unwrap();
        assert_eq!(users_ending_with_son.len(), 1);
        assert_eq!(users_ending_with_son[0].name, "Bob Johnson");

        // Test email contains
        let users_with_example_email = client.user()
            .find_many(vec![user::email::contains("example.com")])
            .exec()
            .await
            .unwrap();
        assert_eq!(users_with_example_email.len(), 2);

        let users_with_test_email = client.user()
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
        let now = chrono::FixedOffset::east_opt(0).unwrap().with_ymd_and_hms(2024, 1, 1, 0, 0, 0).unwrap();

        // Create test users with different ages
        let _user1 = client.user().create(
            "john@example.com".to_string(),
            "John".to_string(),
            now,
            now,
            vec![user::age::set(Some(25)), user::deleted_at::set(None)],
        ).exec().await.unwrap();

        let _user2 = client.user().create(
            "jane@example.com".to_string(),
            "Jane".to_string(),
            now,
            now,
            vec![user::age::set(Some(30)), user::deleted_at::set(None)],
        ).exec().await.unwrap();

        let _user3 = client.user().create(
            "bob@example.com".to_string(),
            "Bob".to_string(),
            now,
            now,
            vec![user::age::set(Some(35)), user::deleted_at::set(None)],
        ).exec().await.unwrap();

        // Test greater than (gt)
        let users_older_than_25 = client.user()
            .find_many(vec![user::age::gt(25)])
            .exec()
            .await
            .unwrap();
        assert_eq!(users_older_than_25.len(), 2);
        assert!(users_older_than_25.iter().all(|u| u.age.unwrap() > 25));

        // Test greater than or equal (gte)
        let users_30_or_older = client.user()
            .find_many(vec![user::age::gte(30)])
            .exec()
            .await
            .unwrap();
        assert_eq!(users_30_or_older.len(), 2);
        assert!(users_30_or_older.iter().all(|u| u.age.unwrap() >= 30));

        // Test less than (lt)
        let users_younger_than_35 = client.user()
            .find_many(vec![user::age::lt(35)])
            .exec()
            .await
            .unwrap();
        assert_eq!(users_younger_than_35.len(), 2);
        assert!(users_younger_than_35.iter().all(|u| u.age.unwrap() < 35));

        // Test less than or equal (lte)
        let users_30_or_younger = client.user()
            .find_many(vec![user::age::lte(30)])
            .exec()
            .await
            .unwrap();
        assert_eq!(users_30_or_younger.len(), 2);
        assert!(users_30_or_younger.iter().all(|u| u.age.unwrap() <= 30));

        // Test with DateTime fields
        let future_date = now + chrono::Duration::days(1);
        let users_created_before_future = client.user()
            .find_many(vec![user::created_at::lt(future_date)])
            .exec()
            .await
            .unwrap();
        assert_eq!(users_created_before_future.len(), 3);

        // Test with String fields (lexicographic comparison)
        let users_name_after_j = client.user()
            .find_many(vec![user::name::gt("J")])
            .exec()
            .await
            .unwrap();
        assert_eq!(users_name_after_j.len(), 2);
        let names: Vec<_> = users_name_after_j.iter().map(|u| u.name.as_str()).collect();
        assert!(names.contains(&"John"));
        assert!(names.contains(&"Jane"));

        // Test combination of operators
        let users_age_between_25_and_35 = client.user()
            .find_many(vec![
                user::age::gte(25),
                user::age::lte(35),
            ])
            .exec()
            .await
            .unwrap();
        assert_eq!(users_age_between_25_and_35.len(), 3);
        assert!(users_age_between_25_and_35.iter().all(|u| {
            let age = u.age.unwrap();
            age >= 25 && age <= 35
        }));

        // Test with Option<DateTime<FixedOffset>> (deleted_at)
        // Set deleted_at for one user
        let deleted_time = now + chrono::Duration::days(2);
        let _user4 = client.user().create(
            "deleted@example.com".to_string(),
            "Deleted".to_string(),
            now,
            now,
            vec![user::age::set(Some(40)), user::deleted_at::set(Some(deleted_time))],
        ).exec().await.unwrap();

        // gt: deleted_at > now + 1 day (should match user4)
        let users_deleted_after = client.user()
            .find_many(vec![user::deleted_at::gt(now + chrono::Duration::days(1))])
            .exec().await.unwrap();
        assert_eq!(users_deleted_after.len(), 1);
        assert_eq!(users_deleted_after[0].email, "deleted@example.com");

        // lte: deleted_at <= deleted_time (should match user4)
        let users_deleted_on_or_before = client.user()
            .find_many(vec![user::deleted_at::lte(deleted_time)])
            .exec().await.unwrap();
        assert_eq!(users_deleted_on_or_before.len(), 1);
        assert_eq!(users_deleted_on_or_before[0].email, "deleted@example.com");

        // Test with Option<String> (post::content)
        // Create posts with and without content
        let _post1 = client.post().create(
            "Post 1".to_string(),
            now,
            now,
            user::id::equals(1),
            vec![post::content::set(Some("Hello".to_string()))],
        ).exec().await.unwrap();
        let _post2 = client.post().create(
            "Post 2".to_string(),
            now,
            now,
            user::id::equals(1),
            vec![post::content::set(Some("World".to_string()))],
        ).exec().await.unwrap();
        let _post3 = client.post().create(
            "Post 3".to_string(),
            now,
            now,
            user::id::equals(1),
            vec![post::content::set(None)],
        ).exec().await.unwrap();

        // gt: content > "Hello" (should match post2)
        let posts_gt_hello = client.post()
            .find_many(vec![post::content::gt(Some("Hello".to_string()))])
            .exec().await.unwrap();
        assert!(posts_gt_hello.iter().any(|p| p.title == "Post 2"));

        // lte: content <= "World" (should match post1 and post2)
        let posts_lte_world = client.post()
            .find_many(vec![post::content::lte(Some("World".to_string()))])
            .exec().await.unwrap();
        assert!(posts_lte_world.iter().any(|p| p.title == "Post 1"));
        assert!(posts_lte_world.iter().any(|p| p.title == "Post 2"));

        // lt: content < "World" (should match post1)
        let posts_lt_world = client.post()
            .find_many(vec![post::content::lt(Some("World".to_string()))])
            .exec().await.unwrap();
        assert!(posts_lt_world.iter().any(|p| p.title == "Post 1"));

        // gte: content >= "Hello" (should match post1 and post2)
        let posts_gte_hello = client.post()
            .find_many(vec![post::content::gte(Some("Hello".to_string()))])
            .exec().await.unwrap();
        assert!(posts_gte_hello.iter().any(|p| p.title == "Post 1"));
        assert!(posts_gte_hello.iter().any(|p| p.title == "Post 2"));
    }
}
