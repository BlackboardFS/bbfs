use regex::Regex;
use serde::Deserialize;
pub mod client;
mod course_main_data;
mod list_content_data;
mod memberships_data;

#[derive(Clone, Debug)]
pub struct Course {
    // TODO make it so that we don't just take 8 from short_names (doesn't feel generic at all!)
    // maybe this isn't needed though but it just feels weird
    short_name: String,
    id: String,
}

#[derive(Clone, Debug, PartialEq, Eq, Hash)]
pub struct CourseItem {
    name: String,
    content: Option<CourseItemContent>,
    description: Option<String>,
    attachments: Vec<String>,
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
struct User {
    id: String,
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
