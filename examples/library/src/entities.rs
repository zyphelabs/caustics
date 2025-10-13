use caustics_macros::caustics;

#[caustics]
pub mod author {
    use caustics_macros::Caustics;
    use sea_orm::entity::prelude::*;
    use caustics::chrono::Utc;

    #[derive(Caustics, Clone, Debug, PartialEq, DeriveEntityModel)]
    #[sea_orm(table_name = "authors")]
    pub struct Model {
        #[sea_orm(primary_key, auto_increment = true, column_name = "authorId")]
        pub id: i32,
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
    pub enum Relation {
        #[sea_orm(has_many = "super::book::Entity", from = "Column::Id", to = "super::book::Column::AuthorId")]
        /// #[caustics(field_name="published_works")]
        Books,
        #[sea_orm(has_one = "super::api_key::Entity", from = "Column::Id", to = "super::api_key::Column::AuthorId")]
        /// #[caustics(field_name="access_key")]
        ApiKey,
        #[sea_orm(has_one = "super::profile::Entity", from = "Column::Id", to = "super::profile::Column::AuthorId")]
        /// #[caustics(field_name="profile", nullable)]
        Profile,
    }

    impl Related<super::book::Entity> for Entity {
        fn to() -> RelationDef {
            Relation::Books.def()
        }
    }

    impl Related<super::api_key::Entity> for Entity {
        fn to() -> RelationDef {
            Relation::ApiKey.def()
        }
    }

    impl Related<super::profile::Entity> for Entity {
        fn to() -> RelationDef {
            Relation::Profile.def()
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
        #[sea_orm(primary_key, auto_increment = false, column_name = "bookTitle")]
        pub title: String,
        #[sea_orm(primary_key, auto_increment = false, column_name = "authorId")]
        pub author_id: i32,
        #[sea_orm(column_name = "publicationYear")]
        pub publication_year: i32,
        #[sea_orm(column_name = "genres", column_type = "Json")]
        pub genres: serde_json::Value,
        #[sea_orm(column_name = "createdAt")]
        /// #[caustics(default)]
        pub created_at: DateTime<Utc>,
        #[sea_orm(column_name = "updatedAt")]
        /// #[caustics(default)]
        pub updated_at: DateTime<Utc>,
    }

    #[derive(Caustics, Copy, Clone, Debug, EnumIter, DeriveRelation)]
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
    #[sea_orm(table_name = "ApiKey")]
        pub struct Model {
        #[sea_orm(primary_key, auto_increment = false)]
        pub id: String,
        pub key: String,
        pub allowed_origins: String,
        pub options: serde_json::Value,
        #[sea_orm(column_name = "authorId")]
        pub author_id: i32,
        pub created_at: NaiveDateTime,
        pub updated_at: NaiveDateTime,
        pub deleted: bool,
        pub deleted_at: Option<NaiveDateTime>,
    }

    #[derive(Caustics, Copy, Clone, Debug, EnumIter, DeriveRelation)]
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
pub mod profile {
    use caustics_macros::Caustics;
    use caustics::prelude::*;

    #[derive(Caustics, Clone, Debug, PartialEq, DeriveEntityModel)]
    #[sea_orm(table_name = "profiles")]
    pub struct Model {
        #[sea_orm(primary_key, auto_increment = true)]
        pub id: i32,
        #[sea_orm(column_name = "authorId")]
        pub author_id: i32,
        pub bio: Option<String>,
        pub website: Option<String>,
        pub twitter_handle: Option<String>,
        pub location: Option<String>,
        pub avatar_url: Option<String>,
        pub created_at: NaiveDateTime,
        pub updated_at: NaiveDateTime,
    }

    #[derive(Caustics, Copy, Clone, Debug, EnumIter, DeriveRelation)]
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