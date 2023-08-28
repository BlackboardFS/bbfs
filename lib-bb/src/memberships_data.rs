use serde::Deserialize;
use time::{OffsetDateTime, PrimitiveDateTime};

#[derive(Deserialize)]
pub struct MembershipsData {
    pub results: Vec<CourseEntry>,
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct CourseEntry {
    /// The one that looks like _1234587_1
    pub course_id: String,
    pub course: Course,
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Course {
    /// The one that looks like ABCD1234S_1234_12345
    #[serde(rename(deserialize = "courseId"))]
    pub short_name: String,
    pub display_name: String,
    pub term: CourseTerm,
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct CourseTerm {
    #[serde(with = "time::serde::rfc3339::option")]
    pub start_date: Option<OffsetDateTime>,
    #[serde(with = "time::serde::rfc3339::option")]
    pub end_date: Option<OffsetDateTime>,
}
