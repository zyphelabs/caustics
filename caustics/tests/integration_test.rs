include!(concat!(env!("OUT_DIR"), "/caustics_client_test.rs"));

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
        pub age: i32,
        pub created_at: DateTime<FixedOffset>,
        pub updated_at: DateTime<FixedOffset>,
        pub deleted_at: Option<DateTime<FixedOffset>>,
    }

    #[derive(Copy, Clone, Debug, EnumIter)]
    pub enum Relation {}
    impl sea_orm::RelationTrait for Relation {
        fn def(&self) -> sea_orm::RelationDef {
            panic!("No relations defined")
        }
    }

    impl sea_orm::ActiveModelBehavior for ActiveModel {}
}
/* 
pub mod post {
    use caustics_macros::Caustics;
    use sea_orm::entity::prelude::*;
    use chrono::{DateTime, FixedOffset};
    
    #[derive(Caustics)]
    #[derive(Clone, Debug, PartialEq, DeriveEntityModel)]
    #[sea_orm(table_name = "posts")]
    pub struct Model {
        #[sea_orm(primary_key)]
        pub id: i32,
        pub title: String,
        pub content: String,
        #[sea_orm(created_at)]
        pub created_at: DateTime<FixedOffset>,
        #[sea_orm(updated_at)]
        pub updated_at: DateTime<FixedOffset>,
        #[sea_orm(column_name = "user_id")]
        pub user_id: i32,
    }
    #[derive(Copy, Clone, Debug, EnumIter, DeriveRelation)]
    pub enum Relation {}
    impl ActiveModelBehavior for ActiveModel {}
}
*/

#[path = "helpers.rs"] mod helpers;

mod client_tests {
    use super::*;
    use crate::helpers::*;

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
    use super::*;
    use crate::helpers::*;

    #[tokio::test]
    async fn test_find_operations() {
        let db = setup_test_db().await;
        let client = CausticsClient::new(db.clone());

        // Find unique
        let user = client
            .user()
            .find_unique(user::id::equals(1))
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
            .await
            .unwrap();
        assert!(user.is_none());

        // Find many
        let users = client
            .user()
            .find_many(vec![
                user::age::gt(18),
            ])
            .await
            .unwrap();
        assert!(users.is_empty());
    }
}

mod query_builder_tests {
    use super::*;
    use crate::helpers::*;

    #[tokio::test]
    async fn test_create_operations() {
        let db = setup_test_db().await;
        let client = CausticsClient::new(db.clone());

        // Create user
        let user = client
            .user()
            .create(
                user::name::set("John"),
                user::email::set("john@example.com"),
                vec![
                    user::age::set(25),
                ],
            )
            .await
            .unwrap();
        assert_eq!(user.name, "John");
        assert_eq!(user.email, "john@example.com");
        assert_eq!(user.age, 25);

        // Create post
        let post = client
            .post()
            .create(
                post::title::set("Hello World"),
                post::content::set("This is my first post"),
                post::user_id::set(user.id),
                vec![],
            )
            .await
            .unwrap();
        assert_eq!(post.title, "Hello World");
        assert_eq!(post.content, "This is my first post");
        assert_eq!(post.user_id, user.id);

        teardown_test_db(&db).await;
    }
/*
    #[tokio::test]
    async fn test_update_operations() {
        let db = setup_test_db().await;
        let client = CausticsClient::new(db.clone());

        // Create user
        let user = client
            .user()
            .create(
                user::name::set("John"),
                user::email::set("john@example.com"),
                vec![
                    user::age::set(25),
                ],
            )
            .await
            .unwrap();

        // Update user
        let updated_user = client
            .user()
            .update(
                user::id::equals(user.id),
                user::name::set("John Doe"),
                user::age::set(26),
            )
            .await
            .unwrap();
        assert_eq!(updated_user.name, "John Doe");
        assert_eq!(updated_user.age, 26);

        teardown_test_db(&db).await;
    }

    #[tokio::test]
    async fn test_delete_operations() {
        let db = setup_test_db().await;
        let client = CausticsClient::new(db.clone());

        // Create user
        let user = client
            .user()
            .create(
                user::name::set("John"),
                user::email::set("john@example.com"),
                vec![
                    user::age::set(25),
                ],
            )
            .await
            .unwrap();

        // Delete user
        client
            .user()
            .delete(user.id)
            .await
            .unwrap();

        // Verify deletion
        let deleted_user = client
            .user()
            .find_unique(user::id::equals(user.id))
            .await
            .unwrap();
        assert!(deleted_user.is_none());

        teardown_test_db(&db).await;
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
                user::name::set("John"),
                user::age::set(25),
                vec![],
            )
            .await
            .unwrap();
        assert_eq!(user.name, "John");
        assert_eq!(user.age, 25);

        // Update existing user
        let updated_user = client
            .user()
            .upsert(
                user::email::equals("john@example.com"),
                user::name::set("John Doe"),
                user::age::set(26),
                vec![],
            )
            .await
            .unwrap();
        assert_eq!(updated_user.name, "John Doe");
        assert_eq!(updated_user.age, 26);

        teardown_test_db(&db).await;
    }

    #[tokio::test]
    async fn test_transaction() {
        let db = setup_test_db().await;
        let client = CausticsClient::new(db.clone());

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
            .await
            .unwrap();

        assert_eq!(result.0.name, "John");
        assert_eq!(result.1.title, "Hello World");

        teardown_test_db(&db).await;
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
                    user::name::set(format!("User {}", i)),
                    user::email::set(format!("user{}@example.com", i)),
                    vec![
                        user::age::set(20 + i),
                    ],
                )
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
            .await
            .unwrap();

        assert_eq!(users.len(), 2);
        assert_eq!(users[0].age, 23);
        assert_eq!(users[1].age, 22);

        teardown_test_db(&db).await;
    }

    #[tokio::test]
    async fn test_relations() {
        let db = setup_test_db().await;
        let client = CausticsClient::new(db.clone());

        // Create user
        let user = client
            .user()
            .create(
                user::name::set("John"),
                user::email::set("john@example.com"),
                vec![],
            )
            .await
            .unwrap();

        // Create posts
        for i in 0..3 {
            client
                .post()
                .create(
                    post::title::set(format!("Post {}", i)),
                    post::content::set(format!("Content {}", i)),
                    post::user_id::set(user.id),
                    vec![],
                )
                .await
                .unwrap();
        }

        // Test fetching user with posts
        let user_with_posts = client
            .user()
            .find_unique(user::id::equals(user.id))
            .with(user::posts::fetch(vec![]))
            .await
            .unwrap()
            .unwrap();

        assert_eq!(user_with_posts.posts.len(), 3);
        assert_eq!(user_with_posts.posts[0].title, "Post 0");
        assert_eq!(user_with_posts.posts[1].title, "Post 1");
        assert_eq!(user_with_posts.posts[2].title, "Post 2");

        teardown_test_db(&db).await;
    }

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
    }*/
}