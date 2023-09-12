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

#[derive(Clone, Debug)]
pub struct Course {
    pub short_name: String,
    pub full_name: String,
    pub id: String,
}

#[derive(Clone, Debug, PartialEq, Eq, Hash)]
pub struct CourseItem {
    pub name: String,
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

#[cfg(target_os = "macos")]
pub fn create_link_file(hyperlink: &str) -> String {
    format!("{{ URL = \"https://learn.uq.edu.au{hyperlink}\"; }}")
}

#[cfg(target_os = "macos")]
pub const LINK_FILE_EXT: &str = "webloc";

#[cfg(target_os = "windows")]
pub fn create_link_file(hyperlink: &str) -> String {
    format!(
        "\
[InternetShortcut]
URL=https://learn.uq.edu.au{hyperlink}
"
    )
}

#[cfg(target_os = "windows")]
pub const LINK_FILE_EXT: &str = "url";
