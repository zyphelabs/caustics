#![feature(decl_macro)]

include!(concat!(env!("OUT_DIR"), "/caustics_client_school_test.rs"));

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

pub mod helpers {
    use sea_orm::{Database, DatabaseConnection, Schema};

    use super::{course, department, enrollment, grade, semester, student, teacher};

    pub async fn setup_test_db() -> DatabaseConnection {
        use sea_orm::ConnectionTrait;

        let db = Database::connect("sqlite::memory:").await.unwrap();

        // Create schema
        let schema = Schema::new(db.get_database_backend());

        // Create students table
        let mut student_table = schema.create_table_from_entity(student::Entity);
        let create_students = student_table.if_not_exists();
        let create_students_sql = db.get_database_backend().build(create_students);
        db.execute(create_students_sql).await.unwrap();

        // Create teachers table
        let mut teacher_table = schema.create_table_from_entity(teacher::Entity);
        let create_teachers = teacher_table.if_not_exists();
        let create_teachers_sql = db.get_database_backend().build(create_teachers);
        db.execute(create_teachers_sql).await.unwrap();

        // Create departments table
        let mut department_table = schema.create_table_from_entity(department::Entity);
        let create_departments = department_table.if_not_exists();
        let create_departments_sql = db.get_database_backend().build(create_departments);
        db.execute(create_departments_sql).await.unwrap();

        // Create courses table
        let mut course_table = schema.create_table_from_entity(course::Entity);
        let create_courses = course_table.if_not_exists();
        let create_courses_sql = db.get_database_backend().build(create_courses);
        db.execute(create_courses_sql).await.unwrap();

        // Create enrollments table
        let mut enrollment_table = schema.create_table_from_entity(enrollment::Entity);
        let create_enrollments = enrollment_table.if_not_exists();
        let create_enrollments_sql = db.get_database_backend().build(create_enrollments);
        db.execute(create_enrollments_sql).await.unwrap();

        // Create grades table
        let mut grade_table = schema.create_table_from_entity(grade::Entity);
        let create_grades = grade_table.if_not_exists();
        let create_grades_sql = db.get_database_backend().build(create_grades);
        db.execute(create_grades_sql).await.unwrap();

        // Create semesters table
        let mut semester_table = schema.create_table_from_entity(semester::Entity);
        let create_semesters = semester_table.if_not_exists();
        let create_semesters_sql = db.get_database_backend().build(create_semesters);
        db.execute(create_semesters_sql).await.unwrap();

        db
    }
}

#[cfg(test)]
mod caustics_school_tests {
    use super::helpers::setup_test_db;
    use super::*;
    use caustics::QueryError;
    use chrono::{DateTime, FixedOffset, TimeZone};
    // legacy import removed

    fn fixed_now() -> DateTime<FixedOffset> {
        FixedOffset::east_opt(0)
            .unwrap()
            .with_ymd_and_hms(2024, 1, 1, 0, 0, 0)
            .unwrap()
    }

    #[tokio::test]
    async fn test_create_and_query_student() {
        let db = setup_test_db().await;
        let client = CausticsClient::new(db.clone());

        // Create a student with required fields and SetParams for optional fields
        let student = client
            .student()
            .create(
                "S12345".to_string(),
                "Alice".to_string(),
                "Smith".to_string(),
                fixed_now(),
                fixed_now(),
                true,
                fixed_now(),
                fixed_now(),
                vec![
                    student::email::set(Some("alice@example.com".to_string())),
                    student::phone::set(Some("123456789".to_string())),
                    student::graduation_date::set(None),
                    student::deleted_at::set(None),
                ],
            )
            .exec()
            .await
            .unwrap();
        assert_eq!(student.student_number, "S12345");
        assert_eq!(student.first_name, "Alice");
        assert_eq!(student.last_name, "Smith");
        assert_eq!(student.email, Some("alice@example.com".to_string()));
        assert_eq!(student.phone, Some("123456789".to_string()));
        assert_eq!(student.graduation_date, None);
        assert_eq!(student.deleted_at, None);

        // Query by unique student_number
        let found = client
            .student()
            .find_unique(student::student_number::equals("S12345".to_string()))
            .exec()
            .await
            .unwrap();
        assert!(found.is_some());
        assert_eq!(found.unwrap().first_name, "Alice");
    }

    #[tokio::test]
    async fn test_select_parity_with_include_has_many() {
        let db = setup_test_db().await;
        let client = CausticsClient::new(db.clone());

        // Seed one student
        let _s = client
            .student()
            .create(
                "S00001".to_string(),
                "Zoe".to_string(),
                "Lee".to_string(),
                fixed_now(),
                fixed_now(),
                true,
                fixed_now(),
                fixed_now(),
                vec![
                    student::email::set(Some("zoe@example.com".to_string())),
                    student::phone::set(None),
                    student::graduation_date::set(None),
                    student::deleted_at::set(None),
                ],
            )
            .exec()
            .await
            .unwrap();

        let rows = client
            .student()
            .find_many(vec![])
            .take(1)
            .select(student::select!(first_name, last_name))
            .with(student::enrollments::fetch())
            .exec()
            .await
            .unwrap();
        assert!(!rows.is_empty());
        assert!(rows[0].first_name.is_some());
        assert!(rows[0].last_name.is_some());
        // Verify that only selected fields are populated
        assert!(
            rows[0].email.is_none(),
            "email should not be populated when not selected"
        );
        let _ = &rows[0].enrollments;
    }

    #[tokio::test]
    async fn test_unique_constraint_violation() {
        let db = setup_test_db().await;
        let client = CausticsClient::new(db.clone());

        // Create a department
        let _dept = client
            .department()
            .create(
                "CS".to_string(),
                "Computer Science".to_string(),
                fixed_now(),
                fixed_now(),
                vec![
                    department::description::set(Some("CS Dept".to_string())),
                    department::deleted_at::set(None),
                ],
            )
            .exec()
            .await
            .unwrap();

        // Try to create another department with the same code
        let result = client
            .department()
            .create(
                "CS".to_string(),
                "Another CS".to_string(),
                fixed_now(),
                fixed_now(),
                vec![],
            )
            .exec()
            .await;
        assert!(result.is_err());
    }

    #[tokio::test]
    async fn test_relations_and_enrollments() {
        let db = setup_test_db().await;
        let client = CausticsClient::new(db.clone());

        // Create department
        let dept = client
            .department()
            .create(
                "MATH".to_string(),
                "Mathematics".to_string(),
                fixed_now(),
                fixed_now(),
                vec![],
            )
            .exec()
            .await
            .unwrap();

        // Create a semester
        let semester = client
            .semester()
            .create(
                "2024S1".to_string(),
                "Spring 2024".to_string(),
                fixed_now(),
                fixed_now(),
                true,
                fixed_now(),
                fixed_now(),
                vec![semester::deleted_at::set(None)],
            )
            .exec()
            .await
            .unwrap();

        // Create teacher
        let teacher = client
            .teacher()
            .create(
                "T001".to_string(),
                "Bob".to_string(),
                "Jones".to_string(),
                "bob@school.edu".to_string(),
                fixed_now(),
                true,
                fixed_now(),
                fixed_now(),
                department::id::equals(dept.id),
                vec![
                    teacher::phone::set(None),
                    teacher::termination_date::set(None),
                    teacher::deleted_at::set(None),
                ],
            )
            .exec()
            .await
            .unwrap();

        // Create course
        let course = client
            .course()
            .create(
                "MATH101".to_string(),
                "Calculus".to_string(),
                6,
                30,
                true,
                fixed_now(),
                fixed_now(),
                teacher::id::equals(teacher.id),
                department::id::equals(dept.id),
                vec![
                    course::semester::connect(semester::id::equals(semester.id)),
                    course::description::set(None),
                    course::deleted_at::set(None),
                ],
            )
            .exec()
            .await
            .unwrap();

        // Create student
        let student = client
            .student()
            .create(
                "S54321".to_string(),
                "Charlie".to_string(),
                "Brown".to_string(),
                fixed_now(),
                fixed_now(),
                true,
                fixed_now(),
                fixed_now(),
                vec![
                    student::email::set(Some("charlie@school.edu".to_string())),
                    student::phone::set(None),
                    student::graduation_date::set(None),
                    student::deleted_at::set(None),
                ],
            )
            .exec()
            .await
            .unwrap();

        // Enroll student in course
        let enrollment = client
            .enrollment()
            .create(
                fixed_now(), // enrollment_date
                "enrolled".to_string(),
                fixed_now(), // created_at
                fixed_now(), // updated_at
                student::id::equals(student.id),
                course::id::equals(course.id),
                vec![
                    enrollment::withdrawal_date::set(None),
                    enrollment::deleted_at::set(None),
                ],
            )
            .exec()
            .await
            .unwrap();
        assert_eq!(enrollment.student_id, student.id);
        assert_eq!(enrollment.course_id, course.id);
        assert_eq!(enrollment.withdrawal_date, None);
        assert_eq!(enrollment.deleted_at, None);

        // Fetch all enrollments for student
        let enrollments = client
            .enrollment()
            .find_many(vec![enrollment::student_id::equals(student.id)])
            .exec()
            .await
            .unwrap();
        assert_eq!(enrollments.len(), 1);
        assert_eq!(enrollments[0].course_id, course.id);
    }

    #[tokio::test]
    async fn test_batch_and_transaction() {
        let db = setup_test_db().await;
        let client = CausticsClient::new(db.clone());

        // Batch insert students
        let (student1, student2) = client
            ._batch((
                client.student().create(
                    "S1".to_string(),
                    "A".to_string(),
                    "A".to_string(),
                    fixed_now(),
                    fixed_now(),
                    true,
                    fixed_now(),
                    fixed_now(),
                    vec![],
                ),
                client.student().create(
                    "S2".to_string(),
                    "B".to_string(),
                    "B".to_string(),
                    fixed_now(),
                    fixed_now(),
                    true,
                    fixed_now(),
                    fixed_now(),
                    vec![],
                ),
            ))
            .await
            .unwrap();
        assert_eq!(student1.first_name, "A");
        assert_eq!(student2.first_name, "B");

        // Transaction rollback
        let txn = client._transaction();
        let res = txn
            .run(|tx| async move {
                let _s = tx
                    .student()
                    .create(
                        "S3".to_string(),
                        "C".to_string(),
                        "C".to_string(),
                        fixed_now(),
                        fixed_now(),
                        true,
                        fixed_now(),
                        fixed_now(),
                        vec![],
                    )
                    .exec()
                    .await?;
                Err::<(), QueryError>(QueryError::Custom("force rollback".to_string()))
            })
            .await;
        assert!(res.is_err());
        // S3 should not exist
        let s3 = client
            .student()
            .find_unique(student::student_number::equals("S3".to_string()))
            .exec()
            .await
            .unwrap();
        assert!(s3.is_none());
    }

    #[tokio::test]
    async fn test_nullable_fields_update() {
        let db = setup_test_db().await;
        let client = CausticsClient::new(db.clone());
        let student = client
            .student()
            .create(
                "S200".to_string(),
                "Nullable".to_string(),
                "Fields".to_string(),
                fixed_now(),
                fixed_now(),
                true,
                fixed_now(),
                fixed_now(),
                vec![student::email::set(None)],
            )
            .exec()
            .await
            .unwrap();
        assert_eq!(student.email, None);
        // Update email
        let updated = client
            .student()
            .update(
                student::id::equals(student.id),
                vec![student::email::set(Some("nullable@school.edu".to_string()))],
            )
            .exec()
            .await
            .unwrap();
        assert_eq!(updated.email, Some("nullable@school.edu".to_string()));
        // Set email back to None
        let updated2 = client
            .student()
            .update(
                student::id::equals(student.id),
                vec![student::email::set(None)],
            )
            .exec()
            .await
            .unwrap();
        assert_eq!(updated2.email, None);
    }

    #[tokio::test]
    async fn test_case_insensitive_search_student_first_name() {
        use super::student;
        use caustics::QueryMode;
        let db = setup_test_db().await;
        let client = CausticsClient::new(db.clone());
        let now = chrono::FixedOffset::east_opt(0)
            .unwrap()
            .with_ymd_and_hms(2022, 1, 1, 12, 0, 0)
            .unwrap();
        // Insert a student with mixed-case first_name
        let _student = client
            .student()
            .create(
                "S12345".to_string(), // student_number
                "Alice".to_string(),  // first_name (mixed case)
                "Smith".to_string(),  // last_name
                now,
                now,
                true,
                now,
                now,
                vec![],
            )
            .exec()
            .await
            .expect("insert student");
        // Query with different case and QueryMode::Insensitive
        let found = client
            .student()
            .find_many(vec![
                student::first_name::contains("alice"),
                student::WhereParam::FirstNameMode(QueryMode::Insensitive),
            ])
            .exec()
            .await
            .expect("query student");
        assert!(
            !found.is_empty(),
            "Should find student with case-insensitive search"
        );
        assert_eq!(found[0].first_name, "Alice");
    }
}

#[cfg(test)]
mod caustics_school_advanced_tests {
    use super::helpers::setup_test_db;
    use super::*;
    // legacy imports removed
    use chrono::{DateTime, FixedOffset, TimeZone};

    fn fixed_now() -> DateTime<FixedOffset> {
        FixedOffset::east_opt(0)
            .unwrap()
            .with_ymd_and_hms(2024, 1, 1, 0, 0, 0)
            .unwrap()
    }

    #[tokio::test]
    async fn test_upsert_student() {
        let db = setup_test_db().await;
        let client = CausticsClient::new(db.clone());
        // Upsert (should update)
        let upserted = client
            .student()
            .upsert(
                student::student_number::equals("U1".to_string()),
                student::Create {
                    student_number: "U1".to_string(),
                    first_name: "Upsert".to_string(),
                    last_name: "Test".to_string(),
                    date_of_birth: fixed_now(),
                    enrollment_date: fixed_now(),
                    is_active: true,
                    created_at: fixed_now(),
                    updated_at: fixed_now(),
                    _params: vec![],
                },
                vec![student::first_name::set("Updated".to_string())],
            )
            .exec()
            .await
            .unwrap();
        assert_eq!(upserted.first_name, "Updated");
    }

    #[tokio::test]
    async fn test_upsert_student_create_with_nested_enrollments() {
        let db = setup_test_db().await;
        let client = CausticsClient::new(db.clone());

        // Create department, teacher, and course to enroll into
        let dept = client
            .department()
            .create(
                "UPDPT".to_string(),
                "Upsert Dept".to_string(),
                fixed_now(),
                fixed_now(),
                vec![],
            )
            .exec()
            .await
            .unwrap();

        let teacher = client
            .teacher()
            .create(
                "TUPS1".to_string(),
                "Teach".to_string(),
                "Upsert".to_string(),
                "teach.upsert@school.edu".to_string(),
                fixed_now(),
                true,
                fixed_now(),
                fixed_now(),
                department::id::equals(dept.id),
                vec![],
            )
            .exec()
            .await
            .unwrap();

        let course = client
            .course()
            .create(
                "CUPS1".to_string(),
                "Course Upsert".to_string(),
                3,
                30,
                true,
                fixed_now(),
                fixed_now(),
                teacher::id::equals(teacher.id),
                department::id::equals(dept.id),
                vec![],
            )
            .exec()
            .await
            .unwrap();

        // Upsert a student that does not exist, with nested enrollments create in Create params
        let snum = "UPS_STUD1".to_string();
        let _created = client
            .student()
            .upsert(
                student::student_number::equals(snum.clone()),
                student::Create {
                    student_number: snum.clone(),
                    first_name: "Nested".to_string(),
                    last_name: "Upsert".to_string(),
                    date_of_birth: fixed_now(),
                    enrollment_date: fixed_now(),
                    is_active: true,
                    created_at: fixed_now(),
                    updated_at: fixed_now(),
                    _params: vec![student::enrollments::create(vec![enrollment::Create {
                        enrollment_date: fixed_now(),
                        status: "enrolled".to_string(),
                        created_at: fixed_now(),
                        updated_at: fixed_now(),
                        student: student::student_number::equals(snum.clone()),
                        course: course::id::equals(course.id),
                        _params: vec![],
                    }])],
                },
                vec![],
            )
            .exec()
            .await
            .unwrap();

        // Verify the student exists and has one enrollment created
        let stud = client
            .student()
            .find_unique(student::student_number::equals(snum.clone()))
            .exec()
            .await
            .unwrap()
            .unwrap();

        let enrollments = client
            .enrollment()
            .find_many(vec![enrollment::student_id::equals(stud.id)])
            .exec()
            .await
            .unwrap();
        assert_eq!(enrollments.len(), 1);
        assert_eq!(enrollments[0].course_id, course.id);
    }

    #[tokio::test]
    async fn test_filter_sort_paginate_students() {
        let db = setup_test_db().await;
        let client = CausticsClient::new(db.clone());
        // Insert multiple students
        for i in 0..10 {
            let s = client
                .student()
                .create(
                    format!("S{:02}", i),
                    format!("Name{:02}", i),
                    "Test".to_string(),
                    fixed_now(),
                    fixed_now(),
                    true,
                    fixed_now(),
                    fixed_now(),
                    vec![],
                )
                .exec()
                .await
                .unwrap();
            assert_eq!(s.student_number, format!("S{:02}", i));
        }
        // Filter: only students with S01 and S02
        let filtered = client
            .student()
            .find_many(vec![student::student_number::equals("S01".to_string())])
            .order_by(student::student_number::order(caustics::SortOrder::Asc))
            .exec()
            .await
            .unwrap();
        assert_eq!(filtered.len(), 1);
        assert_eq!(filtered[0].student_number, "S01");

        let filtered2 = client
            .student()
            .find_many(vec![student::student_number::equals("S02".to_string())])
            .order_by(student::student_number::order(caustics::SortOrder::Asc))
            .exec()
            .await
            .unwrap();
        assert_eq!(filtered2.len(), 1);
        assert_eq!(filtered2[0].student_number, "S02");

        // Order by enrollments count desc using nested relation sugar
        let _by_count = client
            .student()
            .find_many(vec![])
            .order_by(student::enrollments::order_by(enrollment::id::count(
                caustics::SortOrder::Desc,
            )))
            .take(5)
            .exec()
            .await
            .unwrap();
    }

    #[tokio::test]
    async fn test_select_parity_with_include_belongs_to() {
        let db = setup_test_db().await;
        let client = CausticsClient::new(db.clone());

        let row = client
            .teacher()
            .find_first(vec![])
            .select(teacher::select!(first_name))
            .with(teacher::department::fetch())
            .exec()
            .await
            .unwrap();
        if let Some(t) = row {
            assert!(t.first_name.is_some());
            let _ = &t.department;
        }
    }

    #[tokio::test]
    async fn test_nested_relations_fetching() {
        let db = setup_test_db().await;
        let client = CausticsClient::new(db.clone());
        // Setup department, teacher, semester, course, student, enrollment
        let dept = client
            .department()
            .create(
                "SCI".to_string(),
                "Science".to_string(),
                fixed_now(),
                fixed_now(),
                vec![],
            )
            .exec()
            .await
            .unwrap();
        assert_eq!(dept.code, "SCI");
        let semester = client
            .semester()
            .create(
                "2024S2".to_string(),
                "Summer 2024".to_string(),
                fixed_now(),
                fixed_now(),
                true,
                fixed_now(),
                fixed_now(),
                vec![],
            )
            .exec()
            .await
            .unwrap();
        assert_eq!(semester.code, "2024S2");
        let teacher = client
            .teacher()
            .create(
                "T002".to_string(),
                "Eve".to_string(),
                "Newton".to_string(),
                "eve@school.edu".to_string(),
                fixed_now(),
                true,
                fixed_now(),
                fixed_now(),
                department::id::equals(dept.id),
                vec![],
            )
            .exec()
            .await
            .unwrap();
        assert_eq!(teacher.first_name, "Eve");
        let course = client
            .course()
            .create(
                "SCI101".to_string(),
                "Physics".to_string(),
                5,
                40,
                true,
                fixed_now(),
                fixed_now(),
                teacher::id::equals(teacher.id),
                department::id::equals(dept.id),
                vec![course::semester::connect(semester::id::equals(semester.id))],
            )
            .exec()
            .await
            .unwrap();
        assert_eq!(course.code, "SCI101");
        let student = client
            .student()
            .create(
                "S100".to_string(),
                "Nested".to_string(),
                "Student".to_string(),
                fixed_now(),
                fixed_now(),
                true,
                fixed_now(),
                fixed_now(),
                vec![],
            )
            .exec()
            .await
            .unwrap();
        assert_eq!(student.student_number, "S100");
        let enrollment = client
            .enrollment()
            .create(
                fixed_now(),
                "enrolled".to_string(),
                fixed_now(),
                fixed_now(),
                student::id::equals(student.id),
                course::id::equals(course.id),
                vec![],
            )
            .exec()
            .await
            .unwrap();
        assert_eq!(enrollment.status, "enrolled");
        // Fetch course with teacher, department, and enrollments
        let course_with_rel = client
            .course()
            .find_unique(course::id::equals(course.id))
            .with(course::teacher::fetch())
            .with(course::department::fetch())
            .with(course::enrollments::fetch())
            .exec()
            .await
            .unwrap()
            .unwrap();
        assert_eq!(course_with_rel.teacher.unwrap().first_name, "Eve");
        assert_eq!(course_with_rel.department.unwrap().name, "Science");
        assert_eq!(course_with_rel.enrollments.as_ref().unwrap().len(), 1);
    }

    #[tokio::test]
    async fn test_select_nested_include_student_enrollments_course_teacher() {
        let db = setup_test_db().await;
        let client = CausticsClient::new(db.clone());

        // Create department
        let dept = client
            .department()
            .create(
                "SCI".to_string(),
                "Science".to_string(),
                fixed_now(),
                fixed_now(),
                vec![],
            )
            .exec()
            .await
            .unwrap();

        // Create teacher
        let teacher = client
            .teacher()
            .create(
                "T100".to_string(),
                "Teach".to_string(),
                "Er".to_string(),
                "teach@school.edu".to_string(),
                fixed_now(),
                true,
                fixed_now(),
                fixed_now(),
                department::id::equals(dept.id),
                vec![],
            )
            .exec()
            .await
            .unwrap();

        // Create semester
        let semester = client
            .semester()
            .create(
                "2024FALL".to_string(),
                "Fall 2024".to_string(),
                fixed_now(),
                fixed_now(),
                true,
                fixed_now(),
                fixed_now(),
                vec![],
            )
            .exec()
            .await
            .unwrap();

        // Create course
        let course = client
            .course()
            .create(
                "SCI200".to_string(),
                "Advanced Science".to_string(),
                5,
                25,
                true,
                fixed_now(),
                fixed_now(),
                teacher::id::equals(teacher.id),
                department::id::equals(dept.id),
                vec![course::semester::connect(semester::id::equals(semester.id))],
            )
            .exec()
            .await
            .unwrap();

        // Create student
        let student_row = client
            .student()
            .create(
                "S777".to_string(),
                "Sel".to_string(),
                "Inc".to_string(),
                fixed_now(),
                fixed_now(),
                true,
                fixed_now(),
                fixed_now(),
                vec![],
            )
            .exec()
            .await
            .unwrap();

        // Enroll student in course
        let _enrollment = client
            .enrollment()
            .create(
                fixed_now(),
                "enrolled".to_string(),
                fixed_now(),
                fixed_now(),
                student::id::equals(student_row.id),
                course::id::equals(course.id),
                vec![],
            )
            .exec()
            .await
            .unwrap();

        // Select nested include: student -> enrollments -> course(select) -> teacher(select first_name)
        let selected = client
            .student()
            .find_unique(student::id::equals(student_row.id))
            .select(student::select!(first_name))
            .with(student::enrollments::with(enrollment::course::with(
                course::teacher::include(|rel| rel.select(teacher::select!(first_name))),
            )))
            .exec()
            .await
            .unwrap()
            .unwrap();

        assert!(selected.first_name.is_some());
        let enrollments = selected.enrollments.as_ref().unwrap();
        assert!(!enrollments.is_empty());
        let course_rel = enrollments[0].course.as_ref().unwrap();
        let teacher_rel = course_rel.teacher.as_ref().unwrap();
        // We don't assert specific selected fields deep down; just presence
        let _ = teacher_rel;
    }

    #[tokio::test]
    async fn test_deep_nested_include_with_closure_api() {
        let db = setup_test_db().await;
        let client = CausticsClient::new(db.clone());

        // Seed minimal graph: department -> teacher -> course; student -> enrollment -> course -> teacher
        let dept = client
            .department()
            .create(
                "DNC".to_string(),
                "Deep Nested Dept".to_string(),
                fixed_now(),
                fixed_now(),
                vec![],
            )
            .exec()
            .await
            .unwrap();

        let teacher = client
            .teacher()
            .create(
                "TDNC".to_string(),
                "Deep".to_string(),
                "Teach".to_string(),
                "deep.teach@school.edu".to_string(),
                fixed_now(),
                true,
                fixed_now(),
                fixed_now(),
                department::id::equals(dept.id),
                vec![],
            )
            .exec()
            .await
            .unwrap();

        let course = client
            .course()
            .create(
                "CDNC".to_string(),
                "Deep Course".to_string(),
                3,
                30,
                true,
                fixed_now(),
                fixed_now(),
                teacher::id::equals(teacher.id),
                department::id::equals(dept.id),
                vec![],
            )
            .exec()
            .await
            .unwrap();

        let student_row = client
            .student()
            .create(
                "SDNC".to_string(),
                "Deep".to_string(),
                "Student".to_string(),
                fixed_now(),
                fixed_now(),
                true,
                fixed_now(),
                fixed_now(),
                vec![],
            )
            .exec()
            .await
            .unwrap();

        let _enrollment = client
            .enrollment()
            .create(
                fixed_now(),
                "enrolled".to_string(),
                fixed_now(),
                fixed_now(),
                student::id::equals(student_row.id),
                course::id::equals(course.id),
                vec![],
            )
            .exec()
            .await
            .unwrap();

        // Deep nested include using only include(|rel| ...) closures at every level
        let selected = client
            .student()
            .find_unique(student::id::equals(student_row.id))
            .select(student::select!(first_name, last_name))
            .with(student::enrollments::include(|rel| {
                rel.with(enrollment::course::include(|rel2| {
                    rel2.select(course::select!(name))
                        .with(course::teacher::include(|rel3| {
                            rel3.select(teacher::select!(first_name))
                        }))
                }))
            }))
            .exec()
            .await
            .unwrap()
            .unwrap();

        assert!(selected.first_name.is_some());
        assert!(selected.last_name.is_some());
        let enrollments = selected.enrollments.as_ref().unwrap();
        assert!(!enrollments.is_empty());
        let course_rel = enrollments[0].course.as_ref().unwrap();
        assert_eq!(course_rel.name.as_ref().unwrap(), "Deep Course");
        let teacher_rel = course_rel.teacher.as_ref().unwrap();
        assert_eq!(teacher_rel.first_name.as_ref().unwrap(), "Deep");
    }

    #[tokio::test]
    async fn test_student_create_with_nested_enrollments_create_and_create_many() {
        let db = setup_test_db().await;
        let client = CausticsClient::new(db.clone());

        // Setup department, teacher, semester, and two courses
        let dept = client
            .department()
            .create(
                "NEST".to_string(),
                "Nested Dept".to_string(),
                fixed_now(),
                fixed_now(),
                vec![],
            )
            .exec()
            .await
            .unwrap();

        let teacher = client
            .teacher()
            .create(
                "TN1".to_string(),
                "Teach".to_string(),
                "Nest".to_string(),
                "teach.nest@school.edu".to_string(),
                fixed_now(),
                true,
                fixed_now(),
                fixed_now(),
                department::id::equals(dept.id),
                vec![],
            )
            .exec()
            .await
            .unwrap();

        let course_a = client
            .course()
            .create(
                "NEST101".to_string(),
                "Nested A".to_string(),
                3,
                30,
                true,
                fixed_now(),
                fixed_now(),
                teacher::id::equals(teacher.id),
                department::id::equals(dept.id),
                vec![],
            )
            .exec()
            .await
            .unwrap();

        let course_b = client
            .course()
            .create(
                "NEST102".to_string(),
                "Nested B".to_string(),
                4,
                40,
                true,
                fixed_now(),
                fixed_now(),
                teacher::id::equals(teacher.id),
                department::id::equals(dept.id),
                vec![],
            )
            .exec()
            .await
            .unwrap();

        // Create student with nested has_many enrollment create and createMany
        let student_row = client
            .student()
            .create(
                "SNEST".to_string(),
                "Nested".to_string(),
                "Creator".to_string(),
                fixed_now(),
                fixed_now(),
                true,
                fixed_now(),
                fixed_now(),
                vec![
                    // Single create for course_a
                    student::enrollments::create(vec![enrollment::Create {
                        enrollment_date: fixed_now(),
                        status: "enrolled".to_string(),
                        created_at: fixed_now(),
                        updated_at: fixed_now(),
                        student: student::student_number::equals("SNEST".to_string()),
                        course: course::id::equals(course_a.id),
                        _params: vec![
                            enrollment::withdrawal_date::set(None),
                            enrollment::deleted_at::set(None),
                        ],
                    }]),
                    // CreateMany for course_b (two rows)
                    student::enrollments::create_many(vec![
                        enrollment::Create {
                            enrollment_date: fixed_now(),
                            status: "enrolled".to_string(),
                            created_at: fixed_now(),
                            updated_at: fixed_now(),
                            student: student::student_number::equals("SNEST".to_string()),
                            course: course::id::equals(course_b.id),
                            _params: vec![],
                        },
                        enrollment::Create {
                            enrollment_date: fixed_now(),
                            status: "completed".to_string(),
                            created_at: fixed_now(),
                            updated_at: fixed_now(),
                            student: student::student_number::equals("SNEST".to_string()),
                            course: course::id::equals(course_b.id),
                            _params: vec![],
                        },
                    ]),
                ],
            )
            .exec()
            .await
            .unwrap();

        // Verify three enrollments were created for the student
        let enrollments = client
            .enrollment()
            .find_many(vec![enrollment::student_id::equals(student_row.id)])
            .exec()
            .await
            .unwrap();
        assert_eq!(enrollments.len(), 3);
        assert!(enrollments.iter().any(|e| e.course_id == course_a.id));
        assert!(
            enrollments
                .iter()
                .filter(|e| e.course_id == course_b.id)
                .count()
                == 2
        );
    }

    #[tokio::test]
    async fn test_student_update_with_nested_enrollments_create() {
        let db = setup_test_db().await;
        let client = CausticsClient::new(db.clone());

        // Setup a minimal graph: department, teacher, course, student
        let dept = client
            .department()
            .create(
                "UNEST".to_string(),
                "Upd Nested Dept".to_string(),
                fixed_now(),
                fixed_now(),
                vec![],
            )
            .exec()
            .await
            .unwrap();

        let teacher = client
            .teacher()
            .create(
                "TU1".to_string(),
                "Teach".to_string(),
                "Up".to_string(),
                "teach.up@school.edu".to_string(),
                fixed_now(),
                true,
                fixed_now(),
                fixed_now(),
                department::id::equals(dept.id),
                vec![],
            )
            .exec()
            .await
            .unwrap();

        let course = client
            .course()
            .create(
                "UNEST101".to_string(),
                "Upd Nested".to_string(),
                3,
                30,
                true,
                fixed_now(),
                fixed_now(),
                teacher::id::equals(teacher.id),
                department::id::equals(dept.id),
                vec![],
            )
            .exec()
            .await
            .unwrap();

        let s = client
            .student()
            .create(
                "SUNEST".to_string(),
                "Upd".to_string(),
                "Student".to_string(),
                fixed_now(),
                fixed_now(),
                true,
                fixed_now(),
                fixed_now(),
                vec![],
            )
            .exec()
            .await
            .unwrap();

        // Update student with both nested enrollment create and a scalar field set
        let _updated = client
            .student()
            .update(
                student::id::equals(s.id),
                vec![
                    student::first_name::set("UpdatedName".to_string()),
                    student::enrollments::create(vec![enrollment::Create {
                        enrollment_date: fixed_now(),
                        status: "enrolled".to_string(),
                        created_at: fixed_now(),
                        updated_at: fixed_now(),
                        student: student::id::equals(s.id),
                        course: course::id::equals(course.id),
                        _params: vec![],
                    }]),
                ],
            )
            .exec()
            .await
            .unwrap();

        // Verify one enrollment exists for the student after update
        let enrollments = client
            .enrollment()
            .find_many(vec![enrollment::student_id::equals(s.id)])
            .exec()
            .await
            .unwrap();
        assert_eq!(enrollments.len(), 1);
        assert_eq!(enrollments[0].course_id, course.id);
    }

    #[tokio::test]
    async fn test_nullable_fields_update() {
        let db = setup_test_db().await;
        let client = CausticsClient::new(db.clone());
        let student = client
            .student()
            .create(
                "S200".to_string(),
                "Nullable".to_string(),
                "Fields".to_string(),
                fixed_now(),
                fixed_now(),
                true,
                fixed_now(),
                fixed_now(),
                vec![student::email::set(None)],
            )
            .exec()
            .await
            .unwrap();
        assert_eq!(student.email, None);
        // Update email
        let updated = client
            .student()
            .update(
                student::id::equals(student.id),
                vec![student::email::set(Some("nullable@school.edu".to_string()))],
            )
            .exec()
            .await
            .unwrap();
        assert_eq!(updated.email, Some("nullable@school.edu".to_string()));
        // Set email back to None
        let updated2 = client
            .student()
            .update(
                student::id::equals(student.id),
                vec![student::email::set(None)],
            )
            .exec()
            .await
            .unwrap();
        assert_eq!(updated2.email, None);
    }

    #[tokio::test]
    async fn test_case_insensitive_search_student_first_name() {
        use super::student;
        use caustics::QueryMode;
        let db = setup_test_db().await;
        let client = CausticsClient::new(db.clone());
        let now = chrono::FixedOffset::east_opt(0)
            .unwrap()
            .with_ymd_and_hms(2022, 1, 1, 12, 0, 0)
            .unwrap();
        // Insert a student with mixed-case first_name
        let _student = client
            .student()
            .create(
                "S12345".to_string(), // student_number
                "Alice".to_string(),  // first_name (mixed case)
                "Smith".to_string(),  // last_name
                now,
                now,
                true,
                now,
                now,
                vec![],
            )
            .exec()
            .await
            .expect("insert student");
        // Query with different case and QueryMode::Insensitive
        let found = client
            .student()
            .find_many(vec![
                student::first_name::contains("alice"),
                student::WhereParam::FirstNameMode(QueryMode::Insensitive),
            ])
            .exec()
            .await
            .expect("query student");
        assert!(
            !found.is_empty(),
            "Should find student with case-insensitive search"
        );
        assert_eq!(found[0].first_name, "Alice");
    }

    #[tokio::test]
    async fn test_null_operators_school() {
        let db = setup_test_db().await;
        let client = CausticsClient::new(db.clone());

        // Create students with and without email
        let student_with_email = client
            .student()
            .create(
                "S100".to_string(),
                "John".to_string(),
                "Doe".to_string(),
                fixed_now(),
                fixed_now(),
                true,
                fixed_now(),
                fixed_now(),
                vec![student::email::set(Some("john@school.edu".to_string()))],
            )
            .exec()
            .await
            .unwrap();

        let student_without_email = client
            .student()
            .create(
                "S101".to_string(),
                "Jane".to_string(),
                "Smith".to_string(),
                fixed_now(),
                fixed_now(),
                true,
                fixed_now(),
                fixed_now(),
                vec![student::email::set(None)],
            )
            .exec()
            .await
            .unwrap();

        // Test is_null for email field
        let students_without_email = client
            .student()
            .find_many(vec![student::email::is_null()])
            .exec()
            .await
            .unwrap();
        assert_eq!(students_without_email.len(), 1);
        assert_eq!(students_without_email[0].id, student_without_email.id);

        // Test is_not_null for email field
        let students_with_email = client
            .student()
            .find_many(vec![student::email::is_not_null()])
            .exec()
            .await
            .unwrap();
        assert_eq!(students_with_email.len(), 1);
        assert_eq!(students_with_email[0].id, student_with_email.id);
        assert_eq!(
            students_with_email[0].email,
            Some("john@school.edu".to_string())
        );
    }

    #[tokio::test]
    async fn test_nested_include_child_filters_take_skip_order() {
        use caustics::SortOrder;
        let db = setup_test_db().await;
        let client = CausticsClient::new(db.clone());

        // Seed minimal graph: department, teacher, course, student
        let dept = client
            .department()
            .create(
                "FILT".to_string(),
                "Filters Dept".to_string(),
                fixed_now(),
                fixed_now(),
                vec![],
            )
            .exec()
            .await
            .unwrap();

        let teacher = client
            .teacher()
            .create(
                "TF1".to_string(),
                "Teach".to_string(),
                "Filt".to_string(),
                "teach.filt@school.edu".to_string(),
                fixed_now(),
                true,
                fixed_now(),
                fixed_now(),
                department::id::equals(dept.id),
                vec![],
            )
            .exec()
            .await
            .unwrap();

        let course = client
            .course()
            .create(
                "CFILT".to_string(),
                "Course Filters".to_string(),
                3,
                30,
                true,
                fixed_now(),
                fixed_now(),
                teacher::id::equals(teacher.id),
                department::id::equals(dept.id),
                vec![],
            )
            .exec()
            .await
            .unwrap();

        let stud = client
            .student()
            .create(
                "SFILT".to_string(),
                "Filt".to_string(),
                "Stu".to_string(),
                fixed_now(),
                fixed_now(),
                true,
                fixed_now(),
                fixed_now(),
                vec![],
            )
            .exec()
            .await
            .unwrap();

        // Insert multiple enrollments with varying statuses
        let _e1 = client
            .enrollment()
            .create(
                fixed_now(),
                "enrolled".to_string(),
                fixed_now(),
                fixed_now(),
                student::id::equals(stud.id),
                course::id::equals(course.id),
                vec![],
            )
            .exec()
            .await
            .unwrap();
        let _e2 = client
            .enrollment()
            .create(
                fixed_now(),
                "completed".to_string(),
                fixed_now(),
                fixed_now(),
                student::id::equals(stud.id),
                course::id::equals(course.id),
                vec![],
            )
            .exec()
            .await
            .unwrap();
        let _e3 = client
            .enrollment()
            .create(
                fixed_now(),
                "enrolled".to_string(),
                fixed_now(),
                fixed_now(),
                student::id::equals(stud.id),
                course::id::equals(course.id),
                vec![],
            )
            .exec()
            .await
            .unwrap();

        // Fetch student with enrollments filtered to status == "enrolled", ordered by id desc, take 1
        let with_filtered = client
            .student()
            .find_unique(student::id::equals(stud.id))
            .with(student::enrollments::include(|rel| {
                rel.filter(vec![enrollment::status::equals("enrolled".to_string())])
                    .order_by(vec![enrollment::id::order(SortOrder::Desc)])
                    .take(1)
                    .skip(0)
            }))
            .exec()
            .await
            .unwrap()
            .unwrap();

        let enrollments = with_filtered.enrollments.as_ref().unwrap();
        assert_eq!(enrollments.len(), 1, "should return only take=1 enrollment");
        assert_eq!(enrollments[0].status, "enrolled");

        // Now test contains filter on status
        let with_contains = client
            .student()
            .find_unique(student::id::equals(stud.id))
            .with(student::enrollments::include(|rel| {
                rel.filter(vec![enrollment::status::contains("roll".to_string())])
            }))
            .exec()
            .await
            .unwrap()
            .unwrap();
        let enrollments_contains = with_contains.enrollments.as_ref().unwrap();
        assert!(enrollments_contains
            .iter()
            .all(|e| e.status.contains("roll")));
    }

    #[tokio::test]
    async fn test_belongs_to_include_with_args_filters() {
        let db = setup_test_db().await;
        let client = CausticsClient::new(db.clone());

        // Create department with distinct name and a teacher under it
        let dept = client
            .department()
            .create(
                "BFILT".to_string(),
                "Belongs Filters Dept".to_string(),
                fixed_now(),
                fixed_now(),
                vec![],
            )
            .exec()
            .await
            .unwrap();

        let teacher = client
            .teacher()
            .create(
                "TB1".to_string(),
                "Bel".to_string(),
                "Filt".to_string(),
                "bel.filt@school.edu".to_string(),
                fixed_now(),
                true,
                fixed_now(),
                fixed_now(),
                department::id::equals(dept.id),
                vec![],
            )
            .exec()
            .await
            .unwrap();

        let course = client
            .course()
            .create(
                "CBF1".to_string(),
                "Course BT".to_string(),
                3,
                30,
                true,
                fixed_now(),
                fixed_now(),
                teacher::id::equals(teacher.id),
                department::id::equals(dept.id),
                vec![],
            )
            .exec()
            .await
            .unwrap();

        // Include belongs_to teacher with a filter on last_name (contains)
        let with_teacher = client
            .course()
            .find_unique(course::id::equals(course.id))
            .with(course::teacher::include(|rel| {
                rel.filter(vec![teacher::last_name::contains("ilt".to_string())])
            }))
            .exec()
            .await
            .unwrap()
            .unwrap();

        assert!(with_teacher.teacher.is_some());
        assert!(with_teacher
            .teacher
            .as_ref()
            .unwrap()
            .last_name
            .contains("ilt"));
    }

    #[tokio::test]
    async fn test_nested_include_child_cursor_pagination() {
        use caustics::SortOrder;
        let db = setup_test_db().await;
        let client = CausticsClient::new(db.clone());

        let dept = client
            .department()
            .create(
                "CURSOR".to_string(),
                "Cursor Dept".to_string(),
                fixed_now(),
                fixed_now(),
                vec![],
            )
            .exec()
            .await
            .unwrap();
        let teacher = client
            .teacher()
            .create(
                "TCUR".to_string(),
                "Cur".to_string(),
                "Sor".to_string(),
                "cur.sor@school.edu".to_string(),
                fixed_now(),
                true,
                fixed_now(),
                fixed_now(),
                department::id::equals(dept.id),
                vec![],
            )
            .exec()
            .await
            .unwrap();
        let course = client
            .course()
            .create(
                "CCUR".to_string(),
                "Course Cursor".to_string(),
                3,
                30,
                true,
                fixed_now(),
                fixed_now(),
                teacher::id::equals(teacher.id),
                department::id::equals(dept.id),
                vec![],
            )
            .exec()
            .await
            .unwrap();
        let stud = client
            .student()
            .create(
                "SCUR".to_string(),
                "Cur".to_string(),
                "Sor".to_string(),
                fixed_now(),
                fixed_now(),
                true,
                fixed_now(),
                fixed_now(),
                vec![],
            )
            .exec()
            .await
            .unwrap();

        let e1 = client
            .enrollment()
            .create(
                fixed_now(),
                "x".to_string(),
                fixed_now(),
                fixed_now(),
                student::id::equals(stud.id),
                course::id::equals(course.id),
                vec![],
            )
            .exec()
            .await
            .unwrap();
        let e2 = client
            .enrollment()
            .create(
                fixed_now(),
                "y".to_string(),
                fixed_now(),
                fixed_now(),
                student::id::equals(stud.id),
                course::id::equals(course.id),
                vec![],
            )
            .exec()
            .await
            .unwrap();
        let _e3 = client
            .enrollment()
            .create(
                fixed_now(),
                "z".to_string(),
                fixed_now(),
                fixed_now(),
                student::id::equals(stud.id),
                course::id::equals(course.id),
                vec![],
            )
            .exec()
            .await
            .unwrap();

        let s = client
            .student()
            .find_unique(student::id::equals(stud.id))
            .with(student::enrollments::include(|rel| {
                rel.order_by(vec![enrollment::id::order(SortOrder::Asc)])
                    .cursor(e1.id)
                    .take(1)
            }))
            .exec()
            .await
            .unwrap()
            .unwrap();
        let children = s.enrollments.unwrap();
        assert_eq!(children.len(), 1);
        assert_eq!(children[0].id, e2.id);
    }

    #[tokio::test]
    async fn test_nested_include_child_order_by_non_id() {
        use caustics::SortOrder;
        let db = setup_test_db().await;
        let client = CausticsClient::new(db.clone());

        let dept = client
            .department()
            .create(
                "ORD".to_string(),
                "Order Dept".to_string(),
                fixed_now(),
                fixed_now(),
                vec![],
            )
            .exec()
            .await
            .unwrap();
        let teacher = client
            .teacher()
            .create(
                "TORD".to_string(),
                "Ord".to_string(),
                "Er".to_string(),
                "ord.er@school.edu".to_string(),
                fixed_now(),
                true,
                fixed_now(),
                fixed_now(),
                department::id::equals(dept.id),
                vec![],
            )
            .exec()
            .await
            .unwrap();
        let course = client
            .course()
            .create(
                "CORD".to_string(),
                "Course Order".to_string(),
                3,
                30,
                true,
                fixed_now(),
                fixed_now(),
                teacher::id::equals(teacher.id),
                department::id::equals(dept.id),
                vec![],
            )
            .exec()
            .await
            .unwrap();
        let stud = client
            .student()
            .create(
                "SORD".to_string(),
                "Ord".to_string(),
                "Er".to_string(),
                fixed_now(),
                fixed_now(),
                true,
                fixed_now(),
                fixed_now(),
                vec![],
            )
            .exec()
            .await
            .unwrap();

        let _a = client
            .enrollment()
            .create(
                fixed_now(),
                "bstat".to_string(),
                fixed_now(),
                fixed_now(),
                student::id::equals(stud.id),
                course::id::equals(course.id),
                vec![],
            )
            .exec()
            .await
            .unwrap();
        let _b = client
            .enrollment()
            .create(
                fixed_now(),
                "astat".to_string(),
                fixed_now(),
                fixed_now(),
                student::id::equals(stud.id),
                course::id::equals(course.id),
                vec![],
            )
            .exec()
            .await
            .unwrap();

        let s = client
            .student()
            .find_unique(student::id::equals(stud.id))
            .with(student::enrollments::include(|rel| {
                rel.order_by(vec![enrollment::status::order(SortOrder::Asc)])
            }))
            .exec()
            .await
            .unwrap()
            .unwrap();
        let children = s.enrollments.unwrap();
        assert!(children.len() >= 2);
        let statuses: Vec<String> = children.iter().map(|e| e.status.clone()).collect();
        let mut sorted = statuses.clone();
        sorted.sort();
        assert_eq!(statuses, sorted);
    }

    #[tokio::test]
    async fn test_nested_include_child_distinct() {
        use caustics::SortOrder;
        let db = setup_test_db().await;
        let client = CausticsClient::new(db.clone());

        let dept = client
            .department()
            .create(
                "DD".to_string(),
                "Distinct Dept".to_string(),
                fixed_now(),
                fixed_now(),
                vec![],
            )
            .exec()
            .await
            .unwrap();
        let teacher = client
            .teacher()
            .create(
                "TD".to_string(),
                "Dis".to_string(),
                "Tinct".to_string(),
                "dis.tinct@school.edu".to_string(),
                fixed_now(),
                true,
                fixed_now(),
                fixed_now(),
                department::id::equals(dept.id),
                vec![],
            )
            .exec()
            .await
            .unwrap();
        let course = client
            .course()
            .create(
                "CD".to_string(),
                "Course Distinct".to_string(),
                3,
                30,
                true,
                fixed_now(),
                fixed_now(),
                teacher::id::equals(teacher.id),
                department::id::equals(dept.id),
                vec![],
            )
            .exec()
            .await
            .unwrap();
        let stud = client
            .student()
            .create(
                "SD".to_string(),
                "Dis".to_string(),
                "Tinct".to_string(),
                fixed_now(),
                fixed_now(),
                true,
                fixed_now(),
                fixed_now(),
                vec![],
            )
            .exec()
            .await
            .unwrap();

        let _e1 = client
            .enrollment()
            .create(
                fixed_now(),
                "s1".to_string(),
                fixed_now(),
                fixed_now(),
                student::id::equals(stud.id),
                course::id::equals(course.id),
                vec![],
            )
            .exec()
            .await
            .unwrap();
        let _e2 = client
            .enrollment()
            .create(
                fixed_now(),
                "s1".to_string(),
                fixed_now(),
                fixed_now(),
                student::id::equals(stud.id),
                course::id::equals(course.id),
                vec![],
            )
            .exec()
            .await
            .unwrap();

        let s = client
            .student()
            .find_unique(student::id::equals(stud.id))
            .with(student::enrollments::include(|rel| {
                rel.filter(vec![])
                    .order_by(vec![enrollment::status::order(SortOrder::Asc)])
                    .distinct()
            }))
            .exec()
            .await
            .unwrap()
            .unwrap();

        let children = s.enrollments.unwrap();
        assert!(children.len() >= 2);
        assert!(children.iter().all(|e| e.status == "s1"));
    }
}
