include!(concat!(env!("OUT_DIR"), "/caustics_client_test.rs"));

use std::sync::Arc;

pub mod user {
    use caustics_macros::Caustics;
    use sea_orm::entity::prelude::*;
    use chrono::{DateTime, FixedOffset};

    #[derive(Caustics, Clone, Debug, PartialEq, DeriveEntityModel)]
    #[sea_orm(table_name = "users")]
    pub struct Model {
        #[sea_orm(primary_key)]
        pub id: i32,
        pub email: String,
        pub name: String,
        #[sea_orm(nullable)]
        pub age: Option<i32>,
        pub created_at: DateTime<FixedOffset>,
        pub updated_at: DateTime<FixedOffset>,
        #[sea_orm(nullable)]
        pub deleted_at: Option<DateTime<FixedOffset>>,
    }

    #[derive(Copy, Clone, Debug, EnumIter, DeriveRelation)]
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

pub mod post {
    use caustics_macros::Caustics;
    use sea_orm::entity::prelude::*;
    use chrono::{DateTime, FixedOffset};
    
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

    #[derive(Copy, Clone, Debug, EnumIter, DeriveRelation)]
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
    
        let db = Database::connect("sqlite::memory:").await.unwrap();
        
        // Create schema
        let schema = Schema::new(db.get_database_backend());
        
        // Create users table
        let mut user_table =schema
        .create_table_from_entity(user::Entity);
        let create_users = user_table
            .if_not_exists();
        let create_users_sql = db.get_database_backend().build(create_users);
        db.execute(create_users_sql)
            .await
            .unwrap();
    
        // Create posts table
        let mut post_table = schema
            .create_table_from_entity(post::Entity);
        let create_posts = post_table
            .if_not_exists();
        let create_posts_sql = db.get_database_backend().build(create_posts);
        db.execute(create_posts_sql)
            .await
            .unwrap();
    
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
            .find_first(vec![
                user::name::equals("John"),
                user::age::gt(18),
            ])
            .exec()
            .await
            .unwrap();
        assert!(user.is_none());

        // Find many
        let users = client
            .user()
            .find_many(vec![
                user::age::gt(18),
            ])
            .exec()
            .await
            .unwrap();
        assert!(users.is_empty());
    }

    #[tokio::test]
    async fn test_create_operations() {
        let db = setup_test_db().await;
        let client = CausticsClient::new(db.clone());

        // Create user
        let user = client
            .user()
            .create(
                "john@example.com".to_string(),
                "John".to_string(),
                DateTime::<FixedOffset>::from_str("2021-01-01T00:00:00Z").unwrap(),
                DateTime::<FixedOffset>::from_str("2021-01-01T00:00:00Z").unwrap(),
                vec![
                    user::age::set(Some(25)),
                    user::deleted_at::set(None),
                ],
            )
            .exec()
            .await
            .unwrap();

        let found_user = client
            .user()
            .find_first(vec![user::email::equals("john@example.com")])
            .exec()
            .await
            .unwrap()
            .unwrap();
        assert_eq!(found_user.name, "John");
        assert_eq!(found_user.email, "john@example.com");
        assert_eq!(found_user.age, Some(25));

        // Create post
        let post = client
            .post()
            .create(
                "Hello World".to_string(),
                DateTime::<FixedOffset>::from_str("2021-01-01T00:00:00Z").unwrap(),
                DateTime::<FixedOffset>::from_str("2021-01-01T00:00:00Z").unwrap(),
                user.id,
                vec![
                    post::content::set(Some("This is my first post".to_string())),
                ],
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
        assert_eq!(found_post.content, Some("This is my first post".to_string()));
        assert_eq!(found_post.user_id, user.id);
    }

    #[tokio::test]
    async fn test_update_operations() {
        let db = setup_test_db().await;
        let client = CausticsClient::new(db.clone());

        // Create user
        let user = client
            .user()
            .create(
                "john@example.com".to_string(),
                "John".to_string(),
                DateTime::<FixedOffset>::from_str("2021-01-01T00:00:00Z").unwrap(),
                DateTime::<FixedOffset>::from_str("2021-01-01T00:00:00Z").unwrap(),
                vec![
                    user::age::set(Some(25)),
                    user::deleted_at::set(None),
                ],
            )
            .exec()
            .await
            .unwrap();

        // Update user
        let updated_user = client
            .user()
            .update(
                user::id::equals(user.id),
                vec![
                    user::name::set("John Doe"),
                    user::age::set(Some(26)),
                ],
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
                    vec![
                        user::age::set(Some(20 + i)),
                        user::deleted_at::set(None),
                    ],
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

        // Create user
        let user = client
            .user()
            .create(
                "John".to_string(),
                "john@example.com".to_string(),
                DateTime::<FixedOffset>::from_str("2021-01-01T00:00:00Z").unwrap(),
                DateTime::<FixedOffset>::from_str("2021-01-01T00:00:00Z").unwrap(),
                vec![
                    user::age::set(25),
                ],
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
                vec![
                    user::name::set("John"),
                    user::age::set(25),
                ],
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
                vec![
                    user::name::set("John Doe"),
                    user::age::set(26),
                ],
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

        let result = client
            ._transaction()
            .run(|tx| Box::pin(async move {
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
                        user.id,
                        vec![
                            post::content::set("This is my first post".to_string()),
                        ],
                    )
                    .exec()
                    .await?;

                Ok((user, post))
            }))
            .await
            .expect("Transaction failed");

        assert_eq!(result.0.name, "John");
        assert_eq!(result.1.title, "Hello World");

        // Verify data is persisted
        let found_user = client
            .user()
            .find_first(vec![user::email::equals("john@example.com")])
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

        let result: Result<(), QueryError> = client
            ._transaction()
            .run(|tx| Box::pin(async move {
                // Create user
                let _user = tx
                    .user()
                    .create(
                        "rollback@example.com".to_string(),
                        "Rollback".to_string(),
                        DateTime::<FixedOffset>::from_str("2021-01-01T00:00:00Z").unwrap(),
                        DateTime::<FixedOffset>::from_str("2021-01-01T00:00:00Z").unwrap(),
                        vec![],
                    )
                    .exec()
                    .await?;

                // Intentionally return an error to trigger rollback
                Err(QueryError::Custom("Intentional rollback".into()))
            }))
            .await;

        assert!(result.is_err());

        // Verify data is NOT persisted
        let found_user = client
            .user()
            .find_first(vec![user::email::equals("rollback@example.com")])
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
                author.id,
                vec![
                    post::content::set("This post has been reviewed".to_string()),
                    post::reviewer_user_id::set(Some(reviewer.id)),
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
                author.id,
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
            // .with(user::posts::fetch(vec![]))
            .exec()
            .await
            .unwrap()
            .unwrap();

        /*assert_eq!(user_with_posts.posts.len(), 3);
        assert_eq!(user_with_posts.posts[0].title, "Reviewed Post");
        assert_eq!(user_with_posts.posts[1].title, "Unreviewed Post");*/
    }
/*
    #[tokio::test]
    async fn test_batch_operations() {
        let db = setup_test_db().await;
        let client = CausticsClient::new(db.clone());

        // Create multiple users in a batch
        let users = client
            .user()
            .create_many(vec![
                (
                    user::name::set("John"),
                    user::email::set("john@example.com"),
                    vec![user::age::set(25)],
                ),
                (
                    user::name::set("Jane"),
                    user::email::set("jane@example.com"),
                    vec![user::age::set(30)],
                ),
                (
                    user::name::set("Bob"),
                    user::email::set("bob@example.com"),
                    vec![user::age::set(35)],
                ),
            ])
            .await
            .unwrap();

        assert_eq!(users.len(), 3);
        assert_eq!(users[0].name, "John");
        assert_eq!(users[1].name, "Jane");
        assert_eq!(users[2].name, "Bob");

        // Create multiple posts in a batch
        let posts = client
            .post()
            .create_many(vec![
                (
                    post::title::set("Post 1"),
                    post::content::set("Content 1"),
                    post::user_id::set(users[0].id),
                    vec![],
                ),
                (
                    post::title::set("Post 2"),
                    post::content::set("Content 2"),
                    post::user_id::set(users[1].id),
                    vec![],
                ),
            ])
            .await
            .unwrap();

        assert_eq!(posts.len(), 2);
        assert_eq!(posts[0].title, "Post 1");
        assert_eq!(posts[1].title, "Post 2");

        // Update multiple users in a batch
        let updated_users = client
            .user()
            .update_many(
                user::age::less_than(30),
                user::age::set(30),
            )
            .await
            .unwrap();

        assert_eq!(updated_users.len(), 1);
        assert_eq!(updated_users[0].age, 30);

        // Delete multiple posts in a batch
        let deleted_count = client
            .post()
            .delete_many(post::user_id::equals(users[0].id))
            .await
            .unwrap();

        assert_eq!(deleted_count, 1);

        teardown_test_db(&db).await;
    }
*/

}