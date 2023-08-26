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

#[derive(Clone, Debug, PartialEq, Eq, Hash)]
pub struct CourseItem {
    pub name: String,
    pub file_name: Option<String>,
    pub content: Option<CourseItemContent>,
    pub description: Option<String>,
    pub attachments: Vec<String>,
}

#[derive(Clone, Debug, PartialEq, Eq, Hash)]
pub enum CourseItemContent {
    FileUrl(String),
    FolderUrl(String),
    Link(String),
}

impl CourseItemContent {
    // not tested!
    fn from_url(url: String) -> Self {
        let file = Regex::new(r".*/bbcswebdav/.*").unwrap();
        let folder = Regex::new(r".*/listContent\.jsp.*").unwrap();

        if file.is_match(&url) {
            CourseItemContent::FileUrl(url)
        } else if folder.is_match(&url) {
            CourseItemContent::FolderUrl(url)
        } else {
            CourseItemContent::Link(url)
        }
    }
}

impl From<list_content_data::Content> for CourseItem {
    fn from(value: list_content_data::Content) -> Self {
        let name = value.title;
        let content = value.link.clone().map(CourseItemContent::from_url);
        let file_name = match content {
            Some(CourseItemContent::Link(_)) => Some(format!(
                "{}.{LINK_FILE_EXT}",
                value.file_name.unwrap_or_else(|| name.clone())
            )),
            _ => value.file_name,
        };
        let description = value.description;
        let attachments = value.attachments;

        CourseItem {
            name,
            file_name,
            content,
            description,
            attachments,
        }
    }
}

impl From<course_main_data::SidebarEntry> for CourseItem {
    fn from(value: course_main_data::SidebarEntry) -> Self {
        let name = value.name;
        let (file_name, content) = match value.link {
            course_main_data::SidebarLink::Directory(url) => {
                (None, Some(CourseItemContent::FolderUrl(url)))
            }
            course_main_data::SidebarLink::Link(url) => (
                Some(format!("{name}.{LINK_FILE_EXT}")),
                Some(CourseItemContent::Link(url)),
            ),
        };
        CourseItem {
            name,
            file_name,
            content,
            description: None,
            attachments: vec![],
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

#[cfg(target_os = "linux")]
pub fn create_link_file(hyperlink: &str) -> String {
    format!(
        "\
[Desktop Entry]
Encoding=UTF-8
Type=Link
URL=https://learn.uq.edu.au{hyperlink}
Icon=text-html
"
    )
}

#[cfg(target_os = "linux")]
pub const LINK_FILE_EXT: &str = "desktop";
#[cfg(target_os = "linux")]
pub const LINK_FILE_EXTRA_SIZE: usize = "\
[Desktop Entry]
Encoding=UTF-8
Type=Link
URL=https://learn.uq.edu.au
Icon=text-html
"
.len();

#[cfg(target_os = "macos")]
pub fn create_link_file(hyperlink: &str) -> String {
    format!(r#"{ URL = "https://learn.uq.edu.au{hyperlink}"; }"#)
}

#[cfg(target_os = "macos")]
pub const LINK_FILE_EXT: &str = "webloc";
#[cfg(target_os = "macos")]
pub const LINK_FILE_EXTRA_SIZE: usize = r#"{ URL = "https://learn.uq.edu.au"; }"#.len();

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
