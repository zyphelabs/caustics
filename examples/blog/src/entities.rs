use caustics_macros::caustics;

#[caustics]
pub mod user {
    use caustics_macros::Caustics;
    use sea_orm::entity::prelude::*;

    #[derive(Caustics, Clone, Debug, PartialEq, DeriveEntityModel)]
    #[sea_orm(table_name = "users")]
    pub struct Model {
        #[sea_orm(primary_key, auto_increment = false, caustics_default)]
        pub id: Uuid,
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
    impl Related<super::post::Entity> for Entity {
        fn to() -> RelationDef {
            Relation::Posts.def()
        }
    }

    impl ActiveModelBehavior for ActiveModel {}
}

#[caustics]
pub mod post {
    use caustics_macros::Caustics;
    use sea_orm::entity::prelude::*;

    #[derive(Caustics, Clone, Debug, PartialEq, DeriveEntityModel)]
    #[sea_orm(table_name = "posts")]
    pub struct Model {
        #[sea_orm(primary_key, auto_increment = false, caustics_default)]
        pub id: Uuid,
        pub title: String,
        #[sea_orm(nullable)]
        pub content: Option<String>,
        #[sea_orm(created_at)]
        pub created_at: DateTime<FixedOffset>,
        #[sea_orm(updated_at)]
        pub updated_at: DateTime<FixedOffset>,
        #[sea_orm(column_name = "user_id")]
        pub user_id: Uuid,
        #[sea_orm(column_name = "reviewer_user_id", nullable)]
        pub reviewer_user_id: Option<Uuid>,
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

    impl ActiveModelBehavior for ActiveModel {}
}
