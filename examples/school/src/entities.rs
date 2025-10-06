use caustics_macros::caustics;

#[caustics(namespace = "school")]
pub mod student {
    use caustics_macros::Caustics;
    use sea_orm::entity::prelude::*;

    #[derive(Caustics, Clone, Debug, PartialEq, DeriveEntityModel)]
    #[sea_orm(table_name = "students")]
    pub struct Model {
        #[sea_orm(primary_key)]
        pub id: i32,
        #[sea_orm(unique)]
        pub student_number: String,
        pub first_name: String,
        pub last_name: String,
        #[sea_orm(nullable)]
        pub email: Option<String>,
        #[sea_orm(nullable)]
        pub phone: Option<String>,
        pub date_of_birth: DateTime<FixedOffset>,
        pub enrollment_date: DateTime<FixedOffset>,
        #[sea_orm(nullable)]
        pub graduation_date: Option<DateTime<FixedOffset>>,
        pub is_active: bool,
        pub created_at: DateTime<FixedOffset>,
        pub updated_at: DateTime<FixedOffset>,
        #[sea_orm(nullable)]
        pub deleted_at: Option<DateTime<FixedOffset>>,
    }

    #[derive(Caustics, Copy, Clone, Debug, EnumIter, DeriveRelation)]
    pub enum Relation {
        #[sea_orm(
            has_many = "super::enrollment::Entity",
            from = "Column::Id",
            to = "super::enrollment::Column::StudentId"
        )]
        Enrollments,
        #[sea_orm(
            has_many = "super::grade::Entity",
            from = "Column::Id",
            to = "super::grade::Column::StudentId"
        )]
        Grades,
    }

    impl Related<super::enrollment::Entity> for Entity {
        fn to() -> RelationDef {
            Relation::Enrollments.def()
        }
    }

    impl Related<super::grade::Entity> for Entity {
        fn to() -> RelationDef {
            Relation::Grades.def()
        }
    }
}

#[caustics(namespace = "school")]
pub mod teacher {
    use caustics_macros::Caustics;
    use sea_orm::entity::prelude::*;

    #[derive(Caustics, Clone, Debug, PartialEq, DeriveEntityModel)]
    #[sea_orm(table_name = "teachers")]
    pub struct Model {
        #[sea_orm(primary_key)]
        pub id: i32,
        #[sea_orm(unique)]
        pub employee_number: String,
        pub first_name: String,
        pub last_name: String,
        #[sea_orm(unique)]
        pub email: String,
        #[sea_orm(nullable)]
        pub phone: Option<String>,
        pub hire_date: DateTime<FixedOffset>,
        #[sea_orm(nullable)]
        pub termination_date: Option<DateTime<FixedOffset>>,
        pub is_active: bool,
        pub created_at: DateTime<FixedOffset>,
        pub updated_at: DateTime<FixedOffset>,
        #[sea_orm(nullable)]
        pub deleted_at: Option<DateTime<FixedOffset>>,
        #[sea_orm(column_name = "department_id")]
        pub department_id: i32,
    }

    #[derive(Caustics, Copy, Clone, Debug, EnumIter, DeriveRelation)]
    pub enum Relation {
        #[sea_orm(
            has_many = "super::course::Entity",
            from = "Column::Id",
            to = "super::course::Column::TeacherId"
        )]
        Courses,
        #[sea_orm(
            belongs_to = "super::department::Entity",
            from = "Column::DepartmentId",
            to = "super::department::Column::Id",
            on_update = "NoAction",
            on_delete = "NoAction"
        )]
        Department,
    }

    impl Related<super::course::Entity> for Entity {
        fn to() -> RelationDef {
            Relation::Courses.def()
        }
    }

    impl Related<super::department::Entity> for Entity {
        fn to() -> RelationDef {
            Relation::Department.def()
        }
    }
}

#[caustics(namespace = "school")]
pub mod department {
    use caustics_macros::Caustics;
    use sea_orm::entity::prelude::*;

    #[derive(Caustics, Clone, Debug, PartialEq, DeriveEntityModel)]
    #[sea_orm(table_name = "departments")]
    pub struct Model {
        #[sea_orm(primary_key)]
        pub id: i32,
        #[sea_orm(unique)]
        pub code: String,
        pub name: String,
        #[sea_orm(nullable)]
        pub description: Option<String>,
        pub created_at: DateTime<FixedOffset>,
        pub updated_at: DateTime<FixedOffset>,
        #[sea_orm(nullable)]
        pub deleted_at: Option<DateTime<FixedOffset>>,
    }

    #[derive(Caustics, Copy, Clone, Debug, EnumIter, DeriveRelation)]
    pub enum Relation {
        #[sea_orm(
            has_many = "super::course::Entity",
            from = "Column::Id",
            to = "super::course::Column::DepartmentId"
        )]
        Courses,
        #[sea_orm(
            has_many = "super::teacher::Entity",
            from = "Column::Id",
            to = "super::teacher::Column::DepartmentId"
        )]
        Teachers,
    }

    impl Related<super::course::Entity> for Entity {
        fn to() -> RelationDef {
            Relation::Courses.def()
        }
    }

    impl Related<super::teacher::Entity> for Entity {
        fn to() -> RelationDef {
            Relation::Teachers.def()
        }
    }
}

#[caustics(namespace = "school")]
pub mod course {
    use caustics_macros::Caustics;
    use sea_orm::entity::prelude::*;

    #[derive(Caustics, Clone, Debug, PartialEq, DeriveEntityModel)]
    #[sea_orm(table_name = "courses")]
    pub struct Model {
        #[sea_orm(primary_key)]
        pub id: i32,
        #[sea_orm(unique)]
        pub code: String,
        pub name: String,
        #[sea_orm(nullable)]
        pub description: Option<String>,
        pub credits: i32,
        pub max_students: i32,
        pub is_active: bool,
        pub created_at: DateTime<FixedOffset>,
        pub updated_at: DateTime<FixedOffset>,
        #[sea_orm(nullable)]
        pub deleted_at: Option<DateTime<FixedOffset>>,
        #[sea_orm(column_name = "teacher_id")]
        pub teacher_id: i32,
        #[sea_orm(column_name = "department_id")]
        pub department_id: i32,
        #[sea_orm(column_name = "semester_id", nullable)]
        pub semester_id: Option<i32>,
    }

    #[derive(Caustics, Copy, Clone, Debug, EnumIter, DeriveRelation)]
    pub enum Relation {
        #[sea_orm(
            belongs_to = "super::teacher::Entity",
            from = "Column::TeacherId",
            to = "super::teacher::Column::Id",
            on_update = "NoAction",
            on_delete = "NoAction"
        )]
        Teacher,
        #[sea_orm(
            belongs_to = "super::department::Entity",
            from = "Column::DepartmentId",
            to = "super::department::Column::Id",
            on_update = "NoAction",
            on_delete = "NoAction"
        )]
        Department,
        #[sea_orm(
            belongs_to = "super::semester::Entity",
            from = "Column::SemesterId",
            to = "super::semester::Column::Id",
            on_update = "NoAction",
            on_delete = "NoAction"
        )]
        Semester,
        #[sea_orm(
            has_many = "super::enrollment::Entity",
            from = "Column::Id",
            to = "super::enrollment::Column::CourseId"
        )]
        Enrollments,
        #[sea_orm(
            has_many = "super::grade::Entity",
            from = "Column::Id",
            to = "super::grade::Column::CourseId"
        )]
        Grades,
    }

    impl Related<super::teacher::Entity> for Entity {
        fn to() -> RelationDef {
            Relation::Teacher.def()
        }
    }

    impl Related<super::department::Entity> for Entity {
        fn to() -> RelationDef {
            Relation::Department.def()
        }
    }

    impl Related<super::enrollment::Entity> for Entity {
        fn to() -> RelationDef {
            Relation::Enrollments.def()
        }
    }

    impl Related<super::grade::Entity> for Entity {
        fn to() -> RelationDef {
            Relation::Grades.def()
        }
    }

    impl Related<super::semester::Entity> for Entity {
        fn to() -> RelationDef {
            Relation::Semester.def()
        }
    }
}

#[caustics(namespace = "school")]
pub mod enrollment {
    use caustics_macros::Caustics;
    use sea_orm::entity::prelude::*;

    #[derive(Caustics, Clone, Debug, PartialEq, DeriveEntityModel)]
    #[sea_orm(table_name = "enrollments")]
    pub struct Model {
        #[sea_orm(primary_key)]
        pub id: i32,
        pub enrollment_date: DateTime<FixedOffset>,
        #[sea_orm(nullable)]
        pub withdrawal_date: Option<DateTime<FixedOffset>>,
        pub status: String, // "enrolled", "withdrawn", "completed"
        pub created_at: DateTime<FixedOffset>,
        pub updated_at: DateTime<FixedOffset>,
        #[sea_orm(nullable)]
        pub deleted_at: Option<DateTime<FixedOffset>>,
        #[sea_orm(column_name = "student_id")]
        pub student_id: i32,
        #[sea_orm(column_name = "course_id")]
        pub course_id: i32,
    }

    #[derive(Caustics, Copy, Clone, Debug, EnumIter, DeriveRelation)]
    pub enum Relation {
        #[sea_orm(
            belongs_to = "super::student::Entity",
            from = "Column::StudentId",
            to = "super::student::Column::Id",
            on_update = "NoAction",
            on_delete = "NoAction"
        )]
        Student,
        #[sea_orm(
            belongs_to = "super::course::Entity",
            from = "Column::CourseId",
            to = "super::course::Column::Id",
            on_update = "NoAction",
            on_delete = "NoAction"
        )]
        Course,
    }

    impl Related<super::student::Entity> for Entity {
        fn to() -> RelationDef {
            Relation::Student.def()
        }
    }

    impl Related<super::course::Entity> for Entity {
        fn to() -> RelationDef {
            Relation::Course.def()
        }
    }
}

#[caustics(namespace = "school")]
pub mod grade {
    use caustics_macros::Caustics;
    use sea_orm::entity::prelude::*;

    #[derive(Caustics, Clone, Debug, PartialEq, DeriveEntityModel)]
    #[sea_orm(table_name = "grades")]
    pub struct Model {
        #[sea_orm(primary_key)]
        pub id: i32,
        pub grade_value: i32, // Using integer for grade points (e.g., 85 for 85%)
        #[sea_orm(nullable)]
        pub letter_grade: Option<String>,
        #[sea_orm(nullable)]
        pub comments: Option<String>,
        pub graded_at: DateTime<FixedOffset>,
        pub created_at: DateTime<FixedOffset>,
        pub updated_at: DateTime<FixedOffset>,
        #[sea_orm(nullable)]
        pub deleted_at: Option<DateTime<FixedOffset>>,
        #[sea_orm(column_name = "student_id")]
        pub student_id: i32,
        #[sea_orm(column_name = "course_id")]
        pub course_id: i32,
        #[sea_orm(column_name = "teacher_id")]
        pub teacher_id: i32,
    }

    #[derive(Caustics, Copy, Clone, Debug, EnumIter, DeriveRelation)]
    pub enum Relation {
        #[sea_orm(
            belongs_to = "super::student::Entity",
            from = "Column::StudentId",
            to = "super::student::Column::Id",
            on_update = "NoAction",
            on_delete = "NoAction"
        )]
        Student,
        #[sea_orm(
            belongs_to = "super::course::Entity",
            from = "Column::CourseId",
            to = "super::course::Column::Id",
            on_update = "NoAction",
            on_delete = "NoAction"
        )]
        Course,
        #[sea_orm(
            belongs_to = "super::teacher::Entity",
            from = "Column::TeacherId",
            to = "super::teacher::Column::Id",
            on_update = "NoAction",
            on_delete = "NoAction"
        )]
        Teacher,
    }

    impl Related<super::student::Entity> for Entity {
        fn to() -> RelationDef {
            Relation::Student.def()
        }
    }

    impl Related<super::course::Entity> for Entity {
        fn to() -> RelationDef {
            Relation::Course.def()
        }
    }

    impl Related<super::teacher::Entity> for Entity {
        fn to() -> RelationDef {
            Relation::Teacher.def()
        }
    }
}

#[caustics(namespace = "school")]
pub mod semester {
    use caustics_macros::Caustics;
    use sea_orm::entity::prelude::*;

    #[derive(Caustics, Clone, Debug, PartialEq, DeriveEntityModel)]
    #[sea_orm(table_name = "semesters")]
    pub struct Model {
        #[sea_orm(primary_key)]
        pub id: i32,
        #[sea_orm(unique)]
        pub code: String,
        pub name: String,
        pub start_date: DateTime<FixedOffset>,
        pub end_date: DateTime<FixedOffset>,
        pub is_active: bool,
        pub created_at: DateTime<FixedOffset>,
        pub updated_at: DateTime<FixedOffset>,
        #[sea_orm(nullable)]
        pub deleted_at: Option<DateTime<FixedOffset>>,
    }

    #[derive(Caustics, Copy, Clone, Debug, EnumIter, DeriveRelation)]
    pub enum Relation {
        #[sea_orm(
            has_many = "super::course::Entity",
            from = "Column::Id",
            to = "super::course::Column::SemesterId"
        )]
        Courses,
    }

    impl Related<super::course::Entity> for Entity {
        fn to() -> RelationDef {
            Relation::Courses.def()
        }
    }
}
