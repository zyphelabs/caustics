use library::*;
use sea_orm::{Database, DatabaseConnection, DbErr, ConnectionTrait};
use library::entities::{author, book, api_key, profile};
use caustics::SortOrder;

async fn setup_db() -> Result<DatabaseConnection, DbErr> {
    use sea_orm::Schema;
    
    // Use SQLite in-memory database with proper configuration
    let db = Database::connect("sqlite::memory:?mode=rwc").await?;

    // Create schema
    let schema = Schema::new(db.get_database_backend());

    // Create authors table
    let mut author_table = schema.create_table_from_entity(library::entities::author::Entity);
    author_table.if_not_exists();
    db.execute(db.get_database_backend().build(&author_table)).await?;

    // Create books table
    let mut book_table = schema.create_table_from_entity(library::entities::book::Entity);
    book_table.if_not_exists();
    db.execute(db.get_database_backend().build(&book_table)).await?;

    // Create API key table
    let mut api_key_table = schema.create_table_from_entity(library::entities::api_key::Entity);
    api_key_table.if_not_exists();
    db.execute(db.get_database_backend().build(&api_key_table)).await?;

    // Create profiles table
    let mut profile_table = schema.create_table_from_entity(library::entities::profile::Entity);
    profile_table.if_not_exists();
    db.execute(db.get_database_backend().build(&profile_table)).await?;

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
async fn test_composite_primary_key_create() -> Result<(), DbErr> {
    let db = setup_db().await?;
    let client = CausticsClient::new(db.clone());

    // Test composite primary key create method
    let author_client = client.author();
    let book_client = client.book();

    // Create an author
    let now = chrono::Utc::now();
    
    let author = author_client.create(
        "John".to_string(), // first_name
        "Doe".to_string(), // last_name
        "john.doe@example.com".to_string(), // email
        now, // created_at
        now, // updated_at
        vec![author::date_of_birth::set(None)] // _params
    ).exec().await?;

    // Verify the author was created
    assert_eq!(author.first_name, "John");
    assert_eq!(author.last_name, "Doe");
    assert_eq!(author.email, "john.doe@example.com");

    // Create a book with composite primary key (title + author_id)
    let book = book_client.create(
        "Test Book".to_string(), // title (primary key)
        author.id, // author_id (primary key)
        2023, // publication_year
        serde_json::json!(["Fantasy", "Science Fiction"]), // genres
        vec![] // _params
    ).exec().await?;

    // Verify the book was created with correct field mapping
    assert_eq!(book.title, "Test Book");
    assert_eq!(book.author_id, author.id);
    assert_eq!(book.publication_year, 2023);

    // Test that we can find the book by its composite primary key using find_many
    let found_books = book_client.find_many(vec![
        book::title::equals("Test Book".to_string()),
        book::author_id::equals(author.id)
    ]).exec().await?;

    assert_eq!(found_books.len(), 1);
    let found_book = &found_books[0];
    assert_eq!(found_book.title, "Test Book");
    assert_eq!(found_book.author_id, author.id);

    // Test find_unique with composite primary key
    let found_book_unique = book_client.find_unique(
        book::UniqueWhereParam::TitleAndAuthorId("Test Book".to_string(), author.id)
    ).exec().await?;

    assert!(found_book_unique.is_some());
    let found_book_unique = found_book_unique.unwrap();
    assert_eq!(found_book_unique.title, "Test Book");
    assert_eq!(found_book_unique.author_id, author.id);

    // Test relation fetching with composite keys
    // Test find_many to get books for an author (simulating has_many)
    let books_by_author = book_client.find_many(vec![
        book::author_id::equals(author.id)
    ]).exec().await?;

    assert_eq!(books_by_author.len(), 1);
    assert_eq!(books_by_author[0].title, "Test Book");
    assert_eq!(books_by_author[0].author_id, author.id);

    // Test update with composite primary key
    let updated_book = book_client.update(
        book::UniqueWhereParam::TitleAndAuthorId("Test Book".to_string(), author.id),
        vec![book::publication_year::set(2024)]
    ).exec().await?;

    assert_eq!(updated_book.publication_year, 2024);

    // Test delete with composite primary key
    let deleted_book = book_client.delete(
        book::UniqueWhereParam::TitleAndAuthorId("Test Book".to_string(), author.id)
    ).exec().await?;

    assert_eq!(deleted_book.title, "Test Book");
    assert_eq!(deleted_book.publication_year, 2024);

    // Verify the book was deleted
    let found_book_after_delete = book_client.find_unique(
        book::UniqueWhereParam::TitleAndAuthorId("Test Book".to_string(), author.id)
    ).exec().await?;

    assert!(found_book_after_delete.is_none());

    Ok(())
}

#[tokio::test]
async fn test_composite_primary_key_edge_cases() -> Result<(), DbErr> {
    let db = setup_db().await?;
    let client = CausticsClient::new(db.clone());
    let author_client = client.author();
    let book_client = client.book();

    let now = chrono::Utc::now();

    // Create multiple authors
    let author1 = author_client.create(
        "Alice".to_string(),
        "Smith".to_string(),
        "alice.smith@example.com".to_string(),
        now,
        now,
        vec![]
    ).exec().await?;

    let author2 = author_client.create(
        "Bob".to_string(),
        "Johnson".to_string(),
        "bob.johnson@example.com".to_string(),
        now,
        now,
        vec![]
    ).exec().await?;

    // Test 1: Multiple books with same title but different authors
    let book1 = book_client.create(
        "The Great Novel".to_string(), // Same title
        author1.id, // Different author
        2023,
        serde_json::json!(["Fiction"]),
        vec![]
    ).exec().await?;

    let book2 = book_client.create(
        "The Great Novel".to_string(), // Same title
        author2.id, // Different author
        2024,
        serde_json::json!(["Drama"]),
        vec![]
    ).exec().await?;

    // Verify both books exist with same title but different authors
    assert_eq!(book1.title, "The Great Novel");
    assert_eq!(book1.author_id, author1.id);
    assert_eq!(book2.title, "The Great Novel");
    assert_eq!(book2.author_id, author2.id);
    assert_ne!(book1.author_id, book2.author_id);

    // Test 2: Find books by composite key - should return specific book
    let found_book1 = book_client.find_unique(
        book::UniqueWhereParam::TitleAndAuthorId("The Great Novel".to_string(), author1.id)
    ).exec().await?;

    let found_book2 = book_client.find_unique(
        book::UniqueWhereParam::TitleAndAuthorId("The Great Novel".to_string(), author2.id)
    ).exec().await?;

    assert!(found_book1.is_some());
    assert!(found_book2.is_some());
    assert_eq!(found_book1.unwrap().author_id, author1.id);
    assert_eq!(found_book2.unwrap().author_id, author2.id);

    // Test 3: Update specific book by composite key
    let updated_book1 = book_client.update(
        book::UniqueWhereParam::TitleAndAuthorId("The Great Novel".to_string(), author1.id),
        vec![book::publication_year::set(2025)]
    ).exec().await?;

    assert_eq!(updated_book1.publication_year, 2025);
    assert_eq!(updated_book1.author_id, author1.id);

    // Verify the other book wasn't affected
    let unchanged_book2 = book_client.find_unique(
        book::UniqueWhereParam::TitleAndAuthorId("The Great Novel".to_string(), author2.id)
    ).exec().await?;

    assert!(unchanged_book2.is_some());
    assert_eq!(unchanged_book2.unwrap().publication_year, 2024);

    // Test 4: Delete specific book by composite key
    let deleted_book = book_client.delete(
        book::UniqueWhereParam::TitleAndAuthorId("The Great Novel".to_string(), author1.id)
    ).exec().await?;

    assert_eq!(deleted_book.title, "The Great Novel");
    assert_eq!(deleted_book.author_id, author1.id);

    // Verify the other book still exists
    let remaining_book = book_client.find_unique(
        book::UniqueWhereParam::TitleAndAuthorId("The Great Novel".to_string(), author2.id)
    ).exec().await?;

    assert!(remaining_book.is_some());
    assert_eq!(remaining_book.unwrap().author_id, author2.id);

    Ok(())
}

#[tokio::test]
async fn test_composite_foreign_key_relations() -> Result<(), DbErr> {
    let db = setup_db().await?;
    let client = CausticsClient::new(db.clone());
    let author_client = client.author();
    let book_client = client.book();

    let now = chrono::Utc::now();

    // Create authors
    let author1 = author_client.create(
        "Jane".to_string(),
        "Doe".to_string(),
        "jane.doe@example.com".to_string(),
        now,
        now,
        vec![]
    ).exec().await?;

    let author2 = author_client.create(
        "John".to_string(),
        "Smith".to_string(),
        "john.smith@example.com".to_string(),
        now,
        now,
        vec![]
    ).exec().await?;

    // Create books for both authors
    let _book1 = book_client.create(
        "Adventure Book".to_string(),
        author1.id,
        2023,
        serde_json::json!(["Adventure", "Fiction"]),
        vec![]
    ).exec().await?;

    let _book2 = book_client.create(
        "Mystery Book".to_string(),
        author1.id,
        2023,
        serde_json::json!(["Mystery", "Thriller"]),
        vec![]
    ).exec().await?;

    let _book3 = book_client.create(
        "Science Book".to_string(),
        author2.id,
        2024,
        serde_json::json!(["Science", "Education"]),
        vec![]
    ).exec().await?;

    // Test 1: Find books by author (foreign key filtering)
    let author1_books = book_client.find_many(vec![
        book::author_id::equals(author1.id)
    ]).exec().await?;

    let author2_books = book_client.find_many(vec![
        book::author_id::equals(author2.id)
    ]).exec().await?;

    assert_eq!(author1_books.len(), 2);
    assert_eq!(author2_books.len(), 1);
    assert!(author1_books.iter().all(|b| b.author_id == author1.id));
    assert!(author2_books.iter().all(|b| b.author_id == author2.id));

    // Test 2: Complex queries with composite keys
    let recent_books = book_client.find_many(vec![
        book::publication_year::gte(2023),
        book::author_id::equals(author1.id)
    ]).exec().await?;

    assert_eq!(recent_books.len(), 2);
    assert!(recent_books.iter().all(|b| b.author_id == author1.id));
    assert!(recent_books.iter().all(|b| b.publication_year >= 2023));

    // Test 3: Update multiple books by author
    let updated_count = book_client.update_many(
        vec![book::author_id::equals(author1.id)],
        vec![book::publication_year::set(2025)]
    ).exec().await?;

    assert_eq!(updated_count, 2);

    // Test 4: Delete books by author
    let deleted_count = book_client.delete_many(
        vec![book::author_id::equals(author2.id)]
    ).exec().await?;

    assert_eq!(deleted_count, 1);

    // Verify author1's books still exist
    let remaining_books = book_client.find_many(vec![
        book::author_id::equals(author1.id)
    ]).exec().await?;

    assert_eq!(remaining_books.len(), 2);

    Ok(())
}

#[tokio::test]
async fn test_composite_key_pagination_and_sorting() -> Result<(), DbErr> {
    let db = setup_db().await?;
    let client = CausticsClient::new(db.clone());
    let author_client = client.author();
    let book_client = client.book();

    let now = chrono::Utc::now();

    // Create an author
    let author = author_client.create(
        "Test".to_string(),
        "Author".to_string(),
        "test.author@example.com".to_string(),
        now,
        now,
        vec![]
    ).exec().await?;

    // Create multiple books with different titles
    let books = vec![
        ("Book A", 2020),
        ("Book B", 2021),
        ("Book C", 2022),
        ("Book D", 2023),
        ("Book E", 2024),
    ];

    for (title, year) in books {
        book_client.create(
            title.to_string(),
            author.id,
            year,
            serde_json::json!(["Test"]),
            vec![]
        ).exec().await?;
    }

    // Test 1: Pagination with composite keys
    let first_page = book_client.find_many(vec![
        book::author_id::equals(author.id)
    ])
    .order_by(book::title::order(SortOrder::Asc))
    .take(2)
    .exec().await?;

    assert_eq!(first_page.len(), 2);
    assert_eq!(first_page[0].title, "Book A");
    assert_eq!(first_page[1].title, "Book B");

    // Test 2: Sorting by composite key fields
    let sorted_by_title = book_client.find_many(vec![
        book::author_id::equals(author.id)
    ])
    .order_by(book::title::order(SortOrder::Desc))
    .exec().await?;

    assert_eq!(sorted_by_title.len(), 5);
    assert_eq!(sorted_by_title[0].title, "Book E");
    assert_eq!(sorted_by_title[4].title, "Book A");

    // Test 3: Complex sorting with multiple fields
    let sorted_by_year_then_title = book_client.find_many(vec![
        book::author_id::equals(author.id)
    ])
    .order_by(book::publication_year::order(SortOrder::Desc))
    .order_by(book::title::order(SortOrder::Asc))
    .exec().await?;

    assert_eq!(sorted_by_year_then_title.len(), 5);
    assert_eq!(sorted_by_year_then_title[0].publication_year, 2024);
    assert_eq!(sorted_by_year_then_title[4].publication_year, 2020);

    // Test 4: Skip and take with composite keys
    let middle_page = book_client.find_many(vec![
        book::author_id::equals(author.id)
    ])
    .order_by(book::title::order(SortOrder::Asc))
    .skip(1)
    .take(2)
    .exec().await?;

    assert_eq!(middle_page.len(), 2);
    assert_eq!(middle_page[0].title, "Book B");
    assert_eq!(middle_page[1].title, "Book C");

    Ok(())
}

#[tokio::test]
async fn test_composite_key_error_handling() -> Result<(), DbErr> {
    let db = setup_db().await?;
    let client = CausticsClient::new(db.clone());
    let author_client = client.author();
    let book_client = client.book();

    let now = chrono::Utc::now();

    // Create an author
    let author = author_client.create(
        "Error".to_string(),
        "Test".to_string(),
        "error.test@example.com".to_string(),
        now,
        now,
        vec![]
    ).exec().await?;

    // Create a book
    let _book = book_client.create(
        "Error Book".to_string(),
        author.id,
        2023,
        serde_json::json!(["Error"]),
        vec![]
    ).exec().await?;

    // Test 1: Find non-existent book by composite key
    let non_existent = book_client.find_unique(
        book::UniqueWhereParam::TitleAndAuthorId("Non-existent Book".to_string(), author.id)
    ).exec().await?;

    assert!(non_existent.is_none());

    // Test 2: Find book with wrong author ID
    let wrong_author = book_client.find_unique(
        book::UniqueWhereParam::TitleAndAuthorId("Error Book".to_string(), 99999)
    ).exec().await?;

    assert!(wrong_author.is_none());

    // Test 3: Update non-existent book
    let update_result = book_client.update(
        book::UniqueWhereParam::TitleAndAuthorId("Non-existent Book".to_string(), author.id),
        vec![book::publication_year::set(2025)]
    ).exec().await;

    assert!(update_result.is_err());

    // Test 4: Delete non-existent book
    let delete_result = book_client.delete(
        book::UniqueWhereParam::TitleAndAuthorId("Non-existent Book".to_string(), author.id)
    ).exec().await;

    assert!(delete_result.is_err());

    // Test 5: Create duplicate composite key (should fail)
    let duplicate_result = book_client.create(
        "Error Book".to_string(), // Same title
        author.id, // Same author
        2024,
        serde_json::json!(["Duplicate"]),
        vec![]
    ).exec().await;

    assert!(duplicate_result.is_err());

    Ok(())
}

#[tokio::test]
async fn test_automatic_pluralization() -> Result<(), DbErr> {
    let db = setup_db().await?;
    let client = CausticsClient::new(db.clone());
    let author_client = client.author();
    let book_client = client.book();

    let now = chrono::Utc::now();

    // Create an author
    let author = author_client.create(
        "J.K.".to_string(),
        "Rowling".to_string(),
        "jk.rowling@example.com".to_string(),
        now,
        now,
        vec![]
    ).exec().await?;

    // Create multiple books for the author
    let _book1 = book_client.create(
        "Harry Potter and the Philosopher's Stone".to_string(),
        author.id,
        1997,
        serde_json::json!(["Fantasy", "Children's"]),
        vec![]
    ).exec().await?;

    let _book2 = book_client.create(
        "Harry Potter and the Chamber of Secrets".to_string(),
        author.id,
        1998,
        serde_json::json!(["Fantasy", "Children's"]),
        vec![]
    ).exec().await?;

    let _book3 = book_client.create(
        "Harry Potter and the Prisoner of Azkaban".to_string(),
        author.id,
        1999,
        serde_json::json!(["Fantasy", "Children's"]),
        vec![]
    ).exec().await?;

    // Test that we can find books by author (simulating the pluralized relation)
    let books_by_author = book_client.find_many(vec![
        book::author_id::equals(author.id)
    ]).exec().await?;

    assert_eq!(books_by_author.len(), 3);

    // Verify all books belong to the author
    for book in &books_by_author {
        assert_eq!(book.author_id, author.id);
    }

    // Test that the book titles are correct
    let titles: Vec<&str> = books_by_author.iter().map(|b| b.title.as_str()).collect();
    assert!(titles.contains(&"Harry Potter and the Philosopher's Stone"));
    assert!(titles.contains(&"Harry Potter and the Chamber of Secrets"));
    assert!(titles.contains(&"Harry Potter and the Prisoner of Azkaban"));

    // Note: The actual pluralization happens in the generated code structure
    // The field names in the ModelWithRelations struct will be pluralized
    // This test verifies the basic functionality works

    Ok(())
}

#[tokio::test]
async fn test_relation_field_naming() -> Result<(), DbErr> {
    let db = setup_db().await?;
    let client = CausticsClient::new(db.clone());
    let author_client = client.author();
    let book_client = client.book();

    let now = chrono::Utc::now();

    // Create an author
    let author = author_client.create(
        "George".to_string(),
        "Orwell".to_string(),
        "george.orwell@example.com".to_string(),
        now,
        now,
        vec![]
    ).exec().await?;

    // Create books for the author
    let _book1 = book_client.create(
        "1984".to_string(),
        author.id,
        1949,
        serde_json::json!(["Dystopian", "Fiction"]),
        vec![]
    ).exec().await?;

    let _book2 = book_client.create(
        "Animal Farm".to_string(),
        author.id,
        1945,
        serde_json::json!(["Satire", "Fiction"]),
        vec![]
    ).exec().await?;

    // Test that we can find books by author
    let books_by_author = book_client.find_many(vec![
        book::author_id::equals(author.id)
    ]).exec().await?;

    assert_eq!(books_by_author.len(), 2);

    // Verify the books
    let titles: Vec<&str> = books_by_author.iter().map(|b| b.title.as_str()).collect();
    assert!(titles.contains(&"1984"));
    assert!(titles.contains(&"Animal Farm"));

    // Note: The pluralization happens in the generated ModelWithRelations struct
    // The field names will be automatically pluralized for HasMany relations

    Ok(())
}

#[tokio::test]
async fn test_pluralization_feature_demonstration() -> Result<(), DbErr> {
    let db = setup_db().await?;
    let client = CausticsClient::new(db.clone());
    let author_client = client.author();
    let book_client = client.book();

    let now = chrono::Utc::now();

    // Create an author
    let author = author_client.create(
        "Agatha".to_string(),
        "Christie".to_string(),
        "agatha.christie@example.com".to_string(),
        now,
        now,
        vec![]
    ).exec().await?;

    // Create books for the author
    let _book1 = book_client.create(
        "Murder on the Orient Express".to_string(),
        author.id,
        1934,
        serde_json::json!(["Mystery", "Crime"]),
        vec![]
    ).exec().await?;

    let _book2 = book_client.create(
        "Death on the Nile".to_string(),
        author.id,
        1937,
        serde_json::json!(["Mystery", "Crime"]),
        vec![]
    ).exec().await?;

    // Test that we can find books by author
    let books_by_author = book_client.find_many(vec![
        book::author_id::equals(author.id)
    ]).exec().await?;

    assert_eq!(books_by_author.len(), 2);

    // Verify the books
    let titles: Vec<&str> = books_by_author.iter().map(|b| b.title.as_str()).collect();
    assert!(titles.contains(&"Murder on the Orient Express"));
    assert!(titles.contains(&"Death on the Nile"));

    // Note: The custom field name feature allows you to specify custom relation field names
    // In the generated ModelWithRelations struct, the field will be named "published_works" 
    // instead of the default "books" for the Books relation

    Ok(())
}

#[tokio::test]
async fn test_custom_field_name_verification() -> Result<(), DbErr> {
    let db = setup_db().await?;
    let client = CausticsClient::new(db.clone());
    let author_client = client.author();
    let book_client = client.book();

    let now = chrono::Utc::now();

    let author = author_client.create(
        "Test".to_string(),
        "Author".to_string(),
        "test@example.com".to_string(),
        now,
        now,
        vec![]
    ).exec().await?;

    let _book = book_client.create(
        "Test Book".to_string(),
        author.id,
        2023,
        serde_json::json!(["Test"]),
        vec![]
    ).exec().await?;

    let author_with_books = author_client.find_first(vec![
        author::id::equals(author.id)
    ]).with(author::published_works::include(|rel| rel)).exec().await?
        .expect("Author should exist");

    assert!(author_with_books.published_works.is_some());
    let books = author_with_books.published_works.unwrap();
    assert_eq!(books.len(), 1);
    assert_eq!(books[0].title, "Test Book");
    assert_eq!(books[0].author_id, author.id);
    assert_eq!(books[0].publication_year, 2023);

    let book_with_author = book_client.find_first(vec![
        book::title::equals("Test Book".to_string())
    ]).with(book::author::include(|rel| rel)).exec().await?
        .expect("Book should exist");

    assert!(book_with_author.author.is_some());
    let loaded_author = book_with_author.author.unwrap();
    assert_eq!(loaded_author.id, author.id);
    assert_eq!(loaded_author.first_name, "Test");
    assert_eq!(loaded_author.last_name, "Author");
    assert_eq!(loaded_author.email, "test@example.com");

    Ok(())
}

#[tokio::test]
async fn test_has_one_relation_compilation() -> Result<(), DbErr> {
    let db = setup_db().await?;
    let client = CausticsClient::new(db.clone());
    
    let author_client = client.author();
    let api_key_client = client.api_key();

    let now = chrono::Utc::now();
    let author = author_client.create(
        "John".to_string(),
        "Doe".to_string(),
        "john.doe@example.com".to_string(),
        now,
        now,
        vec![author::date_of_birth::set(Some(now))]
    ).exec().await?;

    let author_without_api_key = author_client.find_first(vec![
        author::id::equals(author.id)
    ]).exec().await?
        .expect("Author should exist");

    assert!(author_without_api_key.access_key.is_none());

    let api_key_id = "test-key-123";
    let api_key_value = "secret-key-value";
    let allowed_origins = "https://example.com";
    let options = serde_json::json!({"permissions": ["read", "write"]});
    
    let _api_key = api_key_client.create(
        api_key_id.to_string(),
        api_key_value.to_string(),
        allowed_origins.to_string(),
        options,
        now.naive_utc(),
        now.naive_utc(),
        false,
        author::id::equals(author.id),
        vec![]
    ).exec().await?;

    // Debug: check if the api_key was created
    let created_api_key = api_key_client.find_first(vec![]).exec().await?;
    assert!(created_api_key.is_some());

    let author_with_api_key = author_client.find_first(vec![
        author::id::equals(author.id)
    ]).with(author::access_key::include(|rel| rel)).exec().await?
        .expect("Author should exist");


    assert!(author_with_api_key.access_key.is_some());
    let loaded_api_key = author_with_api_key.access_key.unwrap();
    
    assert_eq!(loaded_api_key.id, api_key_id);
    assert_eq!(loaded_api_key.key, api_key_value);
    assert_eq!(loaded_api_key.allowed_origins, allowed_origins);
    assert_eq!(loaded_api_key.author_id, author.id);
    assert_eq!(loaded_api_key.deleted, false);
    assert!(loaded_api_key.deleted_at.is_none());

    let api_key_with_author = api_key_client.find_first(vec![
        api_key::id::equals(api_key_id.to_string())
    ]).with(api_key::author::include(|rel| rel)).exec().await?
        .expect("API key should exist");
    
    assert!(api_key_with_author.author.is_some());
    let loaded_author = api_key_with_author.author.unwrap();
    assert_eq!(loaded_author.id, author.id);
    assert_eq!(loaded_author.first_name, "John");
    assert_eq!(loaded_author.last_name, "Doe");
    assert_eq!(loaded_author.email, "john.doe@example.com");

    Ok(())
}

#[tokio::test]
async fn test_nullable_has_one_relation() -> Result<(), DbErr> {
    let db = setup_db().await?;
    let client = CausticsClient::new(db.clone());
    
    let author_client = client.author();
    let profile_client = client.profile();

    let now = chrono::Utc::now();
    
    // Create an author without a profile
    let author = author_client.create(
        "Jane".to_string(),
        "Smith".to_string(),
        "jane.smith@example.com".to_string(),
        now,
        now,
        vec![author::date_of_birth::set(Some(now))]
    ).exec().await?;

    // Test 1: Author without profile - should have Some(None)
    let author_without_profile = author_client.find_first(vec![
        author::id::equals(author.id)
    ]).with(author::profile::include(|rel| rel)).exec().await?
        .expect("Author should exist");

    assert!(author_without_profile.profile.is_some());
    assert!(author_without_profile.profile.as_ref().unwrap().is_none());

    // Test 2: Create a profile for the author
    let profile = profile_client.create(
        now.naive_utc(),
        now.naive_utc(),
        author::id::equals(author.id),
        vec![
            profile::bio::set(Some("Software engineer and tech enthusiast".to_string())),
            profile::website::set(Some("https://janesmith.dev".to_string())),
            profile::twitter_handle::set(Some("@janesmith".to_string())),
            profile::location::set(Some("San Francisco, CA".to_string())),
            profile::avatar_url::set(Some("https://avatars.com/janesmith.jpg".to_string())),
        ]
    ).exec().await?;

    // Test 3: Author with profile - should have Some(Some(profile))
    let author_with_profile = author_client.find_first(vec![
        author::id::equals(author.id)
    ]).with(author::profile::include(|rel| rel)).exec().await?
        .expect("Author should exist");

    assert!(author_with_profile.profile.is_some());
    assert!(author_with_profile.profile.as_ref().unwrap().is_some());
    let loaded_profile = author_with_profile.profile.as_ref().unwrap().as_ref().unwrap();
    assert_eq!(loaded_profile.id, profile.id);
    assert_eq!(loaded_profile.author_id, author.id);
    assert_eq!(loaded_profile.bio, Some("Software engineer and tech enthusiast".to_string()));
    assert_eq!(loaded_profile.website, Some("https://janesmith.dev".to_string()));
    assert_eq!(loaded_profile.twitter_handle, Some("@janesmith".to_string()));
    assert_eq!(loaded_profile.location, Some("San Francisco, CA".to_string()));
    assert_eq!(loaded_profile.avatar_url, Some("https://avatars.com/janesmith.jpg".to_string()));


    // Test 4: Profile with author (belongs_to relation)
    let profile_with_author = profile_client.find_first(vec![
        profile::id::equals(profile.id)
    ]).with(profile::author::include(|rel| rel)).exec().await?
        .expect("Profile should exist");

    assert!(profile_with_author.author.is_some());
    let loaded_author = profile_with_author.author.unwrap();
    assert_eq!(loaded_author.id, author.id);
    assert_eq!(loaded_author.first_name, "Jane");
    assert_eq!(loaded_author.last_name, "Smith");
    assert_eq!(loaded_author.email, "jane.smith@example.com");

    // Test 5: Create another author without a profile to test multiple authors
    let author2 = author_client.create(
        "Bob".to_string(),
        "Johnson".to_string(),
        "bob.johnson@example.com".to_string(),
        now,
        now,
        vec![]
    ).exec().await?;

    let author2_without_profile = author_client.find_first(vec![
        author::id::equals(author2.id)
    ]).with(author::profile::include(|rel| rel)).exec().await?
        .expect("Author should exist");

    assert!(author2_without_profile.profile.is_some());
    assert!(author2_without_profile.profile.as_ref().unwrap().is_none());

    // Test 6: Update profile to test nullable fields
    let updated_profile = profile_client.update(
        profile::id::equals(profile.id),
        vec![
            profile::bio::set(Some("Senior software engineer and open source contributor".to_string())),
            profile::website::set(None), // Make website nullable
            profile::twitter_handle::set(Some("@janesmith_dev".to_string())),
        ]
    ).exec().await?;

    assert_eq!(updated_profile.bio, Some("Senior software engineer and open source contributor".to_string()));
    assert_eq!(updated_profile.website, None);
    assert_eq!(updated_profile.twitter_handle, Some("@janesmith_dev".to_string()));

    // Test 7: Verify the updated profile is correctly loaded
    let author_with_updated_profile = author_client.find_first(vec![
        author::id::equals(author.id)
    ]).with(author::profile::include(|rel| rel)).exec().await?
        .expect("Author should exist");

    assert!(author_with_updated_profile.profile.is_some());
    assert!(author_with_updated_profile.profile.as_ref().unwrap().is_some());
    let final_profile = author_with_updated_profile.profile.as_ref().unwrap().as_ref().unwrap();
    assert_eq!(final_profile.bio, Some("Senior software engineer and open source contributor".to_string()));
    assert_eq!(final_profile.website, None);
    assert_eq!(final_profile.twitter_handle, Some("@janesmith_dev".to_string()));

    Ok(())
}

#[tokio::test]
async fn test_nullable_has_one_with_optional_target_fk() -> Result<(), DbErr> {
    let db = setup_db().await?;
    let client = CausticsClient::new(db.clone());
    
    let author_client = client.author();
    let profile_client = client.profile();

    let now = chrono::Utc::now();
    
    // Create an author
    let author = author_client.create(
        "Test".to_string(),
        "Author".to_string(),
        "test.author@example.com".to_string(),
        now,
        now,
        vec![]
    ).exec().await?;

    // Test 1: Author without profile - should have Some(None) when relation is fetched
    // This demonstrates the Option<Option<Box<>>> type:
    // - First Option: whether the relation was fetched
    // - Second Option: whether the related record exists
    let author_without_profile = author_client.find_first(vec![
        author::id::equals(author.id)
    ]).with(author::profile::include(|rel| rel)).exec().await?
        .expect("Author should exist");

    // The profile field should be Some(None) - relation was fetched but no profile exists
    // With nullable, this should be Option<Option<Box<>>>
    assert!(author_without_profile.profile.is_some());
    assert!(author_without_profile.profile.as_ref().unwrap().is_none());
    

    // Test 2: Create a profile for the author
    let profile = profile_client.create(
        now.naive_utc(),
        now.naive_utc(),
        author::id::equals(author.id),
        vec![
            profile::bio::set(Some("Test bio".to_string())),
            profile::website::set(Some("https://test.com".to_string())),
            profile::twitter_handle::set(Some("@test".to_string())),
            profile::location::set(Some("Test City".to_string())),
            profile::avatar_url::set(Some("https://test.com/avatar.jpg".to_string())),
        ]
    ).exec().await?;

    // Test 3: Author with profile - should have Some(Some(profile)) when relation is fetched
    let author_with_profile = author_client.find_first(vec![
        author::id::equals(author.id)
    ]).with(author::profile::include(|rel| rel)).exec().await?
        .expect("Author should exist");

    // The profile field should be Some(Some(profile)) - relation was fetched and profile exists
    assert!(author_with_profile.profile.is_some());
    assert!(author_with_profile.profile.as_ref().unwrap().is_some());
    
    let loaded_profile = author_with_profile.profile.as_ref().unwrap().as_ref().unwrap();
    assert_eq!(loaded_profile.id, profile.id);
    assert_eq!(loaded_profile.bio, Some("Test bio".to_string()));
    

    // Test 4: Author without fetching the relation - should have None
    let author_without_relation = author_client.find_first(vec![
        author::id::equals(author.id)
    ]).exec().await?
        .expect("Author should exist");

    // The profile field should be None - relation was not fetched
    assert!(author_without_relation.profile.is_none());
    

    Ok(())
}
