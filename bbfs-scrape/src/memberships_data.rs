use serde::Deserialize;
use time::OffsetDateTime;

use crate::Course;

#[derive(Deserialize)]
pub struct CourseMemberships {
    pub results: Vec<CourseMembership>,
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct CourseMembership {
    /// The one that looks like _1234587_1
    pub course_id: String,
    pub course: CourseMembershipDetails,
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct CourseMembershipDetails {
    /// The one that looks like CSSE2310S_1234_12345
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

impl From<CourseMembership> for Course {
    fn from(value: CourseMembership) -> Self {
        Course {
            short_name: value.course.short_name[..8].into(),
            id: value.course_id,
        }
    }
}
