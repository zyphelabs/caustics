use caustics_example::generated::db::user::CreateUnchecked;
use caustics_example::generated::db::PrismaClient;
use caustics_example::generated::db::*;
use once_cell::sync::Lazy;
use prisma_client_rust::Raw;
use std::env;
use std::process::Command;
use std::sync::Arc;
use std::time::Duration;
use testcontainers::{clients, Container, GenericImage};
use tokio::sync::{Mutex, MutexGuard};
use tokio::time::sleep;

static DOCKER: Lazy<clients::Cli> = Lazy::new(|| clients::Cli::default());
static POSTGRES_IMAGE: Lazy<GenericImage> = Lazy::new(|| {
    GenericImage::new("postgres", "14")
        .with_env_var("POSTGRES_USER", "postgres")
        .with_env_var("POSTGRES_PASSWORD", "postgres")
        .with_env_var("POSTGRES_DB", "postgres")
        .with_exposed_port(5432)
});

// Wrapper struct to ensure container cleanup
pub struct TestContainer<'a>(Container<'a, GenericImage>);

impl<'a> Drop for TestContainer<'a> {
    fn drop(&mut self) {
        println!("Cleaning up test container...");
        // The container will be automatically removed when dropped
        self.0.stop();
    }
}

pub mod helpers {
    use super::*;

    pub async fn setup_test_db<'a>(
    ) -> Result<(Arc<Mutex<PrismaClient>>, TestContainer<'a>), Box<dyn std::error::Error>> {
        println!("Setting up test database...");
        println!("Starting PostgreSQL container...");
        let container = TestContainer(DOCKER.run(POSTGRES_IMAGE.clone()));
        let pg_port = container.0.get_host_port_ipv4(5432);
        println!("PostgreSQL running on port: {}", pg_port);
        let db_url = format!(
            "postgresql://postgres:postgres@127.0.0.1:{}/postgres",
            pg_port
        );
        println!("Database URL: {}", db_url);
        env::set_var("DATABASE_URL", &db_url);
        // Wait for PostgreSQL to be ready
        println!("Waiting for PostgreSQL to be ready...");
        let mut retries = 0;
        while retries < 30 {
            let status = Command::new("pg_isready")
                .args(["-h", "127.0.0.1", "-p", &pg_port.to_string()])
                .status()?;
            if status.success() {
                break;
            }
            sleep(Duration::from_secs(1)).await;
            retries += 1;
        }
        if retries >= 30 {
            return Err("PostgreSQL failed to become ready".into());
        }
        // Run Prisma migrations
        println!("Running Prisma migrations...");
        env::set_var("PGPASSWORD", "postgres");
        let migration_sql =
            include_str!("../../prisma/migrations/20240320000000_init/migration.sql");
        let output = Command::new("psql")
            .args([
                "-h",
                "127.0.0.1",
                "-p",
                &pg_port.to_string(),
                "-U",
                "postgres",
                "-d",
                "postgres",
                "-c",
                migration_sql,
            ])
            .output()?;
        if !output.status.success() {
            println!(
                "Migration error: {}",
                String::from_utf8_lossy(&output.stderr)
            );
            return Err("Failed to run Prisma migrations".into());
        }
        println!("Migrations completed successfully");
        // Verify tables exist
        println!("Verifying tables exist...");
        let output = Command::new("psql")
            .args([
                "-h",
                "127.0.0.1",
                "-p",
                &pg_port.to_string(),
                "-U",
                "postgres",
                "-d",
                "postgres",
                "-c",
                "\\dt",
            ])
            .output()?;
        println!(
            "Tables in database: {}",
            String::from_utf8_lossy(&output.stdout)
        );
        println!("Connecting to database...");
        let client = PrismaClient::_builder().with_url(db_url).build().await?;
        let client = Arc::new(Mutex::new(client));
        println!("Database setup complete!");
        Ok((client, container))
    }

    pub async fn cleanup_test_db(client: &PrismaClient) -> Result<(), Box<dyn std::error::Error>> {
        println!("Cleaning up test database...");
        client
            ._query_raw::<()>(Raw::new(
                "TRUNCATE TABLE \"Post\", \"User\" RESTART IDENTITY CASCADE;",
                Vec::new(),
            ))
            .exec()
            .await?;
        println!("Database cleanup complete!");
        Ok(())
    }

    pub async fn teardown_test_db(
        client_guard: MutexGuard<'_, PrismaClient>,
        container: TestContainer<'_>,
    ) -> Result<(), Box<dyn std::error::Error>> {
        cleanup_test_db(&client_guard).await?;
        container.0.stop();
        println!("Test teardown complete!");
        Ok(())
    }
}

mod client_tests {
    use super::helpers::{setup_test_db, teardown_test_db};
    use super::*;

    #[tokio::test(flavor = "multi_thread", worker_threads = 1)]
    async fn test_prisma_client() -> Result<(), Box<dyn std::error::Error>> {
        let (client, container) = setup_test_db().await?;
        let client = client.lock().await;
        // Test that we can execute a simple query
        #[derive(serde::Deserialize)]
        struct Row {
            value: i32,
        }
        let row: Vec<Row> = client
            ._query_raw(Raw::new("SELECT 1 as value", Vec::new()))
            .exec()
            .await?;
        assert_eq!(row.len(), 1);
        assert_eq!(row[0].value, 1);
        teardown_test_db(client, container).await?;

        Ok(())
    }
}

mod query_builder_tests {
    use super::helpers::{setup_test_db, teardown_test_db};
    use super::*;
    use caustics_example::generated::db::PrismaClient as DbClient;
    use chrono::{DateTime, FixedOffset};
    use std::str::FromStr;

    async fn setup<'a>(
    ) -> Result<(Arc<Mutex<DbClient>>, TestContainer<'a>), Box<dyn std::error::Error>> {
        let (client, container) = setup_test_db().await?;
        Ok((client, container))
    }

    #[tokio::test(flavor = "multi_thread", worker_threads = 1)]
    async fn test_find_operations() -> Result<(), Box<dyn std::error::Error>> {
        let (client, container) = setup().await?;
        let client = client.lock().await;

        // Find unique
        let user = client
            .user()
            .find_unique(user::id::equals(1))
            .exec()
            .await?;
        assert!(user.is_none());

        // Find first with multiple conditions
        let user = client
            .user()
            .find_first(vec![user::name::equals("John"), user::age::gt(18)])
            .exec()
            .await?;
        assert!(user.is_none());

        // Find many with pagination and sorting
        let users = client
            .user()
            .find_many(vec![user::age::gt(18)])
            .order_by(user::created_at::order(SortOrder::Desc))
            .take(10)
            .skip(0)
            .exec()
            .await?;
        assert!(users.is_empty());

        teardown_test_db(client, container).await?;

        Ok(())
    }

    #[tokio::test(flavor = "multi_thread", worker_threads = 1)]
    async fn test_create_operations() -> Result<(), Box<dyn std::error::Error>> {
        let (client, container) = setup().await?;
        let client = client.lock().await;

        // Create user with a unique email
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
            .await?;

        let found_user = client
            .user()
            .find_unique(user::email::equals(&email))
            .exec()
            .await?
            .ok_or("Not found")?;
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
            .await?;

        let found_post = client
            .post()
            .find_unique(post::id::equals(post.id))
            .exec()
            .await?
            .ok_or("Not found")?;
        assert_eq!(found_post.title, "Hello World");
        assert_eq!(
            found_post.content,
            Some("This is my first post".to_string())
        );
        assert_eq!(found_post.user_id, user.id);

        teardown_test_db(client, container).await?;

        Ok(())
    }

    #[tokio::test(flavor = "multi_thread", worker_threads = 1)]
    async fn test_update_operations() -> Result<(), Box<dyn std::error::Error>> {
        let (client, container) = setup().await?;
        let client = client.lock().await;

        // Create initial user with a unique email
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
            .await?;

        // Update user
        let updated_user = client
            .user()
            .update(
                user::id::equals(user.id),
                vec![
                    user::name::set("John Updated".to_string()),
                    user::age::set(Some(26)),
                    user::email::set(email.clone()), // Clone the email for the update
                ],
            )
            .exec()
            .await?;

        assert_eq!(updated_user.name, "John Updated");
        assert_eq!(updated_user.age, Some(26));
        assert_eq!(updated_user.email, email);

        teardown_test_db(client, container).await?;

        Ok(())
    }

    #[tokio::test(flavor = "multi_thread", worker_threads = 1)]
    async fn test_pagination_and_sorting() -> Result<(), Box<dyn std::error::Error>> {
        let (client, container) = setup().await?;
        let client = client.lock().await;

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

        teardown_test_db(client, container).await?;

        Ok(())
    }

    #[tokio::test(flavor = "multi_thread", worker_threads = 1)]
    async fn test_upsert_operations() -> Result<(), Box<dyn std::error::Error>> {
        let (client, container) = setup().await?;
        let client = client.lock().await;

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
                    _params: vec![user::age::set(Some(25))],
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

        teardown_test_db(client, container).await?;

        Ok(())
    }

    #[tokio::test(flavor = "multi_thread", worker_threads = 1)]
    async fn test_delete_operations() -> Result<(), Box<dyn std::error::Error>> {
        let (client, container) = setup().await?;
        let client = client.lock().await;

        // Create user with a unique email
        let email = format!("john_{}@example.com", chrono::Utc::now().timestamp());
        let user = client
            .user()
            .create(
                email,
                "John".to_string(),
                DateTime::<FixedOffset>::from_str("2021-01-01T00:00:00Z").unwrap(),
                DateTime::<FixedOffset>::from_str("2021-01-01T00:00:00Z").unwrap(),
                vec![user::age::set(Some(25)), user::deleted_at::set(None)],
            )
            .exec()
            .await?;

        // Delete user
        let deleted_user = client
            .user()
            .delete(user::id::equals(user.id))
            .exec()
            .await?;

        assert_eq!(deleted_user.id, user.id);

        // Verify user is deleted
        let found_user = client
            .user()
            .find_unique(user::id::equals(user.id))
            .exec()
            .await?;
        assert!(found_user.is_none());

        teardown_test_db(client, container).await?;

        Ok(())
    }

    #[tokio::test(flavor = "multi_thread", worker_threads = 1)]
    async fn test_transaction_operations() -> Result<(), Box<dyn std::error::Error>> {
        let (client, container) = setup().await?;
        let client = client.lock().await;

        // Create user and post in a transaction
        let (user, post) = client
            ._transaction()
            .run(|tx| async move {
                let user = tx
                    .user()
                    .create(
                        "john@example.com".to_string(),
                        "John".to_string(),
                        DateTime::<FixedOffset>::from_str("2021-01-01T00:00:00Z").unwrap(),
                        DateTime::<FixedOffset>::from_str("2021-01-01T00:00:00Z").unwrap(),
                        vec![user::age::set(Some(25)), user::deleted_at::set(None)],
                    )
                    .exec()
                    .await?;

                let post = tx
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
                    .await?;

                Ok::<_, prisma_client_rust::QueryError>((user, post))
            })
            .await?;

        assert_eq!(user.name, "John");
        assert_eq!(post.title, "Hello World");
        assert_eq!(post.user_id, user.id);

        teardown_test_db(client, container).await?;

        Ok(())
    }

    #[tokio::test(flavor = "multi_thread", worker_threads = 1)]
    async fn test_relations() -> Result<(), Box<dyn std::error::Error>> {
        let (client, container) = setup().await?;
        let client = client.lock().await;

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

        // Verify reviewer exists before connecting
        let reviewer_exists = client
            .user()
            .find_unique(user::id::equals(reviewer.id))
            .exec()
            .await
            .unwrap()
            .is_some();
        assert!(reviewer_exists, "Reviewer user not found before connecting");

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
        println!(
            "post_without_reviewer.reviewer: {:?}",
            post_without_reviewer.reviewer
        );
        assert!(
            post_without_reviewer.reviewer.is_none()
                || post_without_reviewer.reviewer.as_ref().unwrap().is_none()
        );

        teardown_test_db(client, container).await?;

        Ok(())
    }

    #[tokio::test(flavor = "multi_thread", worker_threads = 1)]
    async fn test_create_many_operations() -> Result<(), Box<dyn std::error::Error>> {
        let (client, container) = setup().await?;
        let client = client.lock().await;

        let timestamp = chrono::Utc::now().timestamp();
        // Create multiple users
        let count = client
            .user()
            .create_many(vec![
                CreateUnchecked {
                    email: format!("john_{}@example.com", timestamp),
                    name: "John".to_string(),
                    created_at: DateTime::<FixedOffset>::from_str("2021-01-01T00:00:00Z").unwrap(),
                    updated_at: DateTime::<FixedOffset>::from_str("2021-01-01T00:00:00Z").unwrap(),
                    _params: vec![user::age::set(Some(25)), user::deleted_at::set(None)],
                },
                CreateUnchecked {
                    email: format!("jane_{}@example.com", timestamp),
                    name: "Jane".to_string(),
                    created_at: DateTime::<FixedOffset>::from_str("2021-01-01T00:00:00Z").unwrap(),
                    updated_at: DateTime::<FixedOffset>::from_str("2021-01-01T00:00:00Z").unwrap(),
                    _params: vec![user::age::set(Some(30)), user::deleted_at::set(None)],
                },
            ])
            .exec()
            .await?;

        assert_eq!(count, 2);

        teardown_test_db(client, container).await?;

        Ok(())
    }

    #[tokio::test(flavor = "multi_thread", worker_threads = 1)]
    async fn test_batch_operations() -> Result<(), Box<dyn std::error::Error>> {
        let (client, container) = setup().await?;
        let client = client.lock().await;

        // Create multiple users
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
            .await?;

        assert_eq!(user1.name, "John");
        assert_eq!(user2.name, "Jane");

        teardown_test_db(client, container).await?;

        Ok(())
    }
}
