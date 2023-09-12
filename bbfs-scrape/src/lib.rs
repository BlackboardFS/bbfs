use regex::Regex;
use serde::Deserialize;
pub mod client;
mod course_main_data;
mod list_content_data;
mod memberships_data;

// TODO: Update all hardcoded URLs to use this.
pub const BB_BASE_URL: &str = "https://learn.uq.edu.au";

#[derive(Clone, Debug)]
pub struct Course {
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

impl CourseItem {
    fn attachments_as_items(&self) -> Vec<Item> {
        self.attachments
            .iter()
            .map(|attachment| {
                Item::CourseItem(CourseItem {
                    name: "".into(),
                    content: Some(CourseItemContent::FileUrl(attachment.clone())),
                    description: None,
                    attachments: vec![],
                })
            })
            .collect()
    }

    fn maybe_new_description_file(&self) -> Option<Item> {
        self.description.as_ref().map(|description| {
            Item::SynthesizedFile(SynthesizedFile {
                name: self.name.clone(),
                contents: description.clone(),
            })
        })
    }

    fn maybe_new_link_file(&self) -> Option<Item> {
        match &self.content {
            Some(CourseItemContent::Link(link)) if !self.attachments.is_empty() => {
                Some(Item::SynthesizedFile(SynthesizedFile {
                    name: format!("{}.{}", self.name, LINK_FILE_EXT),
                    contents: create_link_file(link),
                }))
            }
            _ => None,
        }
    }

    fn get_blackboard_link(&self, parent: &Item) -> String {
        match &self.content {
            Some(CourseItemContent::FolderUrl(url)) => url.clone(),
            Some(CourseItemContent::Link(_) | CourseItemContent::FileUrl(_)) | None => match parent
            {
                Item::Course(ref course) => format!(
                    "https://learn.uq.edu.au/ultra/courses/{}/cl/outline",
                    course.id
                ),
                Item::CourseItem(ref item) => match &item.content {
                    Some(CourseItemContent::FolderUrl(url)) => url.clone(),
                    Some(CourseItemContent::FileUrl(_))
                    | Some(CourseItemContent::Link(_))
                    | None => unreachable!(),
                },
                Item::SynthesizedDirectory(_) | Item::SynthesizedFile(_) => {
                    unreachable!()
                }
            },
        }
    }
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

#[derive(Clone, Debug)]
pub struct SynthesizedFile {
    name: String,
    contents: String,
}

#[derive(Clone, Debug)]
pub struct SynthesizedDirectory {
    name: String,
    contents: Vec<Item>,
}

#[derive(Clone, Debug)]
pub enum Item {
    Course(Course),
    CourseItem(CourseItem),
    SynthesizedFile(SynthesizedFile),
    SynthesizedDirectory(SynthesizedDirectory),
}

impl Item {
    fn make_link_file(name: &str, link: &str) -> Item {
        Item::SynthesizedFile(SynthesizedFile {
            name: format!("{name}.{LINK_FILE_EXT}"),
            contents: create_link_file(link),
        })
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
URL={BB_BASE_URL}{hyperlink}
Icon=text-html
",
    )
}

#[cfg(target_os = "linux")]
pub const LINK_FILE_EXT: &str = "desktop";

#[cfg(target_os = "macos")]
pub fn create_link_file(hyperlink: &str) -> String {
    format!("{{ URL = \"{BB_BASE_URL}{hyperlink}\"; }}")
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
