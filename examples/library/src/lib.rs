pub mod entities;

// Include the generated client directly in the root module
include!(concat!(env!("OUT_DIR"), "/caustics_client_library.rs"));

#[cfg(test)]
mod tests {
    use super::*;
    use sea_orm::{Database, DatabaseConnection, DbErr, ConnectionTrait};
    use crate::entities::{author, book};

    async fn setup_db() -> Result<DatabaseConnection, DbErr> {
        use sea_orm::Schema;
        
        // Use SQLite in-memory database with proper configuration
        let db = Database::connect("sqlite::memory:?mode=rwc").await?;

        // Create schema
        let schema = Schema::new(db.get_database_backend());

        // Create authors table
        let mut author_table = schema.create_table_from_entity(crate::entities::author::Entity);
        author_table.if_not_exists();
        db.execute(db.get_database_backend().build(&author_table)).await?;

        // Create books table
        let mut book_table = schema.create_table_from_entity(crate::entities::book::Entity);
        book_table.if_not_exists();
        db.execute(db.get_database_backend().build(&book_table)).await?;

        Ok(db)
    }

    #[tokio::test]
    async fn test_library_client_works() -> Result<(), DbErr> {
        let db = setup_db().await?;
        let client = CausticsClient::new(db.clone());

        // Test that all entity methods are available
        let _book_client = client.book();
        let _author_client = client.author();

        Ok(())
    }

    #[tokio::test]
    async fn test_camelcase_column_resolution() -> Result<(), DbErr> {
        let db = setup_db().await?;
        let client = CausticsClient::new(db.clone());

        // Test that we can create entities with camelCase column names
        let author_client = client.author();
        let book_client = client.book();

        // Create an author
        let now = chrono::Utc::now();
        
        let author = author_client.create(
            "John".to_string(),
            "Doe".to_string(),
            "john.doe@example.com".to_string(),
            now,
            now,
            vec![author::date_of_birth::set(None)]
        ).exec().await?;

        // Verify the author was created with correct field mapping
        assert_eq!(author.first_name, "John");
        assert_eq!(author.last_name, "Doe");
        assert_eq!(author.email, "john.doe@example.com");

        // Create a book with the author
        let book = book_client.create(
            "Test Book".to_string(),
            vec!["Fantasy".to_string(), "Science Fiction".to_string()],
            now,
            now,
            author::id::equals(author.id.clone()),
            vec![]
        ).exec().await?;

        // Verify the book was created with correct field mapping
        assert_eq!(book.title, "Test Book");
        assert_eq!(book.author_id, author.id.clone());
        assert_eq!(book.genres, vec!["Fantasy".to_string(), "Science Fiction".to_string()]);

        // Test querying with camelCase column names
        let found_author = author_client.find_unique(author::id::equals(author.id.clone()))
            .exec()
            .await?
            .expect("Author should exist");

        assert_eq!(found_author.first_name, "John");

        // Test querying books by author
        let author_books = book_client.find_many(vec![book::author_id::equals(author.id.clone())])
            .exec()
            .await?;

        assert_eq!(author_books.len(), 1);
        assert_eq!(author_books[0].title, "Test Book");
        assert_eq!(author_books[0].genres, vec!["Fantasy".to_string(), "Science Fiction".to_string()]);

        // Test that the camelCase column names are working by checking the database schema
        // This verifies that caustics correctly maps the field names to the database column names
        let raw_query = client._query_raw::<serde_json::Value>(caustics::raw!(
            "SELECT sql FROM sqlite_master WHERE type='table' AND name='authors'"
        )).exec().await?;
        
        let schema_sql = raw_query[0]["sql"].as_str().unwrap();
        println!("Authors table schema: {}", schema_sql);
        
        // Verify camelCase column names exist in the schema
        assert!(schema_sql.contains("authorId"), "authorId column not found in schema");
        assert!(schema_sql.contains("firstName"), "firstName column not found in schema");
        assert!(schema_sql.contains("lastName"), "lastName column not found in schema");
        assert!(schema_sql.contains("emailAddress"), "emailAddress column not found in schema");
        assert!(schema_sql.contains("dateOfBirth"), "dateOfBirth column not found in schema");
        assert!(schema_sql.contains("createdAt"), "createdAt column not found in schema");
        assert!(schema_sql.contains("updatedAt"), "updatedAt column not found in schema");

        // Also check the books table schema
        let books_query = client._query_raw::<serde_json::Value>(caustics::raw!(
            "SELECT sql FROM sqlite_master WHERE type='table' AND name='books'"
        )).exec().await?;
        
        let books_schema_sql = books_query[0]["sql"].as_str().unwrap();
        println!("Books table schema: {}", books_schema_sql);
        
        // Verify camelCase column names exist in the books schema
        assert!(books_schema_sql.contains("bookId"), "bookId column not found in schema");
        assert!(books_schema_sql.contains("bookTitle"), "bookTitle column not found in schema");
        assert!(books_schema_sql.contains("authorId"), "authorId column not found in schema");
        assert!(books_schema_sql.contains("createdAt"), "createdAt column not found in schema");
        assert!(books_schema_sql.contains("updatedAt"), "updatedAt column not found in schema");

        // Test that we can query using the actual camelCase column names
        let column_test_query = client._query_raw::<serde_json::Value>(caustics::raw!(
            "SELECT authorId, firstName, lastName, emailAddress FROM authors WHERE authorId = {}",
            author.id
        )).exec().await?;
        
        assert_eq!(column_test_query.len(), 1);
        let author_data = &column_test_query[0];
        assert_eq!(author_data["firstName"], "John");
        assert_eq!(author_data["lastName"], "Doe");
        assert_eq!(author_data["emailAddress"], "john.doe@example.com");
        
        println!("✅ Successfully queried using camelCase column names:");
        println!("  - authorId: {}", author_data["authorId"]);
        println!("  - firstName: {}", author_data["firstName"]);
        println!("  - lastName: {}", author_data["lastName"]);
        println!("  - emailAddress: {}", author_data["emailAddress"]);

        Ok(())
    }
}
