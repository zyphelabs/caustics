use caustics_macros::caustics;

#[caustics]
pub mod author {
    use caustics_macros::Caustics;
    use sea_orm::entity::prelude::*;
    use chrono::Utc;

    #[derive(Caustics, Clone, Debug, PartialEq, DeriveEntityModel)]
    #[sea_orm(table_name = "authors")]
    pub struct Model {
        #[sea_orm(primary_key, auto_increment = false, column_name = "authorId")]
        pub id: Uuid,
        #[sea_orm(column_name = "firstName")]
        pub first_name: String,
        #[sea_orm(column_name = "lastName")]
        pub last_name: String,
        #[sea_orm(column_name = "emailAddress")]
        pub email: String,
        #[sea_orm(column_name = "dateOfBirth")]
        pub date_of_birth: Option<DateTime<Utc>>,
        #[sea_orm(column_name = "createdAt")]
        pub created_at: DateTime<Utc>,
        #[sea_orm(column_name = "updatedAt")]
        pub updated_at: DateTime<Utc>,
    }

    #[derive(Caustics, Copy, Clone, Debug, EnumIter, DeriveRelation)]
    #[allow(unreachable_code)]
    pub enum Relation {
        #[sea_orm(has_many = "super::book::Entity", from = "Column::Id", to = "super::book::Column::AuthorId")]
        Books,
    }

    impl Related<super::book::Entity> for Entity {
        fn to() -> RelationDef {
            Relation::Books.def()
        }
    }

    impl ActiveModelBehavior for ActiveModel {}
}

#[caustics]
pub mod book {
    use caustics_macros::Caustics;
    use caustics::prelude::*;

    #[derive(Caustics, Clone, Debug, PartialEq, DeriveEntityModel)]
    #[sea_orm(table_name = "books")]
    pub struct Model {
        #[sea_orm(primary_key, auto_increment = false, column_name = "bookId")]
        pub id: Uuid,
        #[sea_orm(column_name = "bookTitle")]
        pub title: String,
        #[sea_orm(column_name = "authorId")]
        pub author_id: Uuid,
        #[sea_orm(column_name = "createdAt")]
        pub created_at: DateTime<Utc>,
        #[sea_orm(column_name = "updatedAt")]
        pub updated_at: DateTime<Utc>,
    }

    #[derive(Caustics, Copy, Clone, Debug, EnumIter, DeriveRelation)]
    #[allow(unreachable_code)]
    pub enum Relation {
        #[sea_orm(belongs_to = "super::author::Entity", from = "Column::AuthorId", to = "super::author::Column::Id")]
        Author,
    }

    impl Related<super::author::Entity> for Entity {
        fn to() -> RelationDef {
            Relation::Author.def()
        }
    }

    impl ActiveModelBehavior for ActiveModel {}
}


#[caustics]
pub mod api_key {
    use caustics_macros::Caustics;
    use caustics::prelude::*;

    #[derive(Caustics, Clone, Debug, PartialEq, Eq, DeriveEntityModel)]
    #[sea_orm(table_name = "ApiKey", schema_name = "api")]
        pub struct Model {
        #[sea_orm(primary_key)]
        pub id: Uuid,
        pub key: String,
        pub allowed_origins: String,
        pub options: serde_json::Value,
        pub created_at: NaiveDateTime,
        pub updated_at: NaiveDateTime,
        pub deleted: bool,
        pub deleted_at: Option<NaiveDateTime>,
    }

    #[derive(Caustics, Copy, Clone, Debug, EnumIter, DeriveRelation)]
    pub enum Relation {}

    impl ActiveModelBehavior for ActiveModel {}
}