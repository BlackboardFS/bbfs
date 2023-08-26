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

use regex::Regex;
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
            short_name: value.course.short_name[..8].into(),
            full_name: value.course.display_name,
            id: value.course_id,
        }
    }
}

#[derive(Clone, Debug)]
pub struct CourseItem {
    pub name: String,
    pub url: Option<String>,
    pub ty: CourseItemType,
}

#[derive(Clone, Debug)]
pub enum CourseItemType {
    Link,
    File,
    Folder,
    Text,
}

impl From<list_content_data::Content> for CourseItem {
    fn from(value: list_content_data::Content) -> Self {
        let name = value.title;
        let url = value.link;
        let ty = match value.icon.as_str() {
            "/images/ci/sets/set12/folder_on.svg" => CourseItemType::Folder,
            _ => {
                let re = Regex::new(
                    r"/webapps/blackboard/content/listContent.jsp\?course_id=.*&content_id=.*",
                )
                .unwrap();
                match url {
                    Some(ref url) => match re.is_match(url) {
                        true => CourseItemType::File,
                        false => CourseItemType::Link,
                    },
                    None => CourseItemType::Text,
                }
            }
        };

        CourseItem { name, url, ty }
    }
}

impl From<course_main_data::SidebarEntry> for CourseItem {
    fn from(value: course_main_data::SidebarEntry) -> Self {
        match value.link {
            course_main_data::SidebarLink::Directory(url) => {
                CourseItem {
                    name: value.name,
                    url: Some(url),
                    ty: CourseItemType::Folder,
                }
            },
            course_main_data::SidebarLink::Link(url) => {
                CourseItem {
                    name: value.name,
                    url: Some(url),
                    ty: CourseItemType::Link,
                }
            },
        }
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
