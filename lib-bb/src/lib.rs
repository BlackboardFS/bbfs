// https://learn.uq.edu.au/learn/api/v1/users/{user_id}/memberships for course names
//
// process:
//
// Authenticate with blackboard
// Get list of courses
// When in a course
//  Get side bar
//      Get items under sidebar

#![allow(dead_code)]

use serde::Deserialize;

pub mod client;
mod course_main_data;
mod list_content_data;
pub mod memberships_data;
mod ultra_data;

#[derive(Clone)]
pub struct Course {
    pub short_name: String,
    pub full_name: String,
    pub id: String,
}

impl From<memberships_data::CourseEntry> for Course {
    fn from(value: memberships_data::CourseEntry) -> Self {
        Course {
            short_name: value.course_id,
            full_name: value.course.display_name,
            id: value.course.short_name,
        }
    }
}

#[derive(Clone)]
pub struct CourseItem {
    pub name: String,
    pub url: Option<String>,
    pub ty: CourseItemType,
}

#[derive(Clone)]
pub enum CourseItemType {
    Link,
    File,
    Folder,
}

impl From<list_content_data::Content> for CourseItem {
    fn from(value: list_content_data::Content) -> Self {
        let name = value.title;
        let url = value.link;
        let ty = CourseItemType::File;

        CourseItem { name, url, ty }
    }
}

#[derive(Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct User {
    pub id: String,
    pub given_name: String,
    pub family_name: String,
    pub user_name: String,
}

// Sidebar entry
// enum TabEntry {
//     Link(String),
//     Directory(Vec<FileEntry>),
// }

// Content under a directory
// i have no idea how these data structures are formatted
// blackboard is such a mess
// enum FileEntry {
//     Directory(Vec<FileEntry>),
//     File {
//         title: String,
//         link: String,
//         attachments: Vec<()>,
//         content_id: String,
//         course_id: String,
//         // https://learn.uq.edu.au/webapps/blackboard/execute/content/file?cmd=view&content_id={content_id}&course_id={course_id}
//         // is the url of the file
//     },
// }
