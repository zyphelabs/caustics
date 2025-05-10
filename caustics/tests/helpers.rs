use sea_orm::{Database, DatabaseConnection};


#[cfg(test)]
#[allow(dead_code)]
pub async fn setup_test_db() -> DatabaseConnection {
    let db = Database::connect("sqlite::memory:").await.unwrap();
    db
}
