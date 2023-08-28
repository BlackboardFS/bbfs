use crate::LINK_FILE_EXT;
use crate::{
    create_link_file, memberships_data::CourseMemberships, Course, CourseItem, CourseItemContent,
    User,
};
use nix::errno::Errno;
use pct_str::PctStr;
use std::cell::RefCell;
use std::error::Error;
use std::fmt::Debug;
use std::fmt::Display;
use std::num::ParseIntError;
use std::{collections::HashMap, time::Duration};
use time::OffsetDateTime;
use ureq::{Agent, AgentBuilder};

const BB_BASE_URL: &str = "https://learn.uq.edu.au";

pub trait BBClient {
    type Item: Clone;
    type Error: Into<Errno> + Debug;

    fn get_root(&self) -> Result<Self::Item, Self::Error>;
    fn get_children(&self, path: Vec<&Self::Item>) -> Result<Option<Vec<Self::Item>>, Self::Error>;
    fn get_size(&self, item: &Self::Item) -> Result<usize, Self::Error>;
    fn get_contents(&self, item: &Self::Item) -> Result<Vec<u8>, Self::Error>;
    fn get_type(&self, item: &Self::Item) -> ItemType;
    fn get_name(&self, item: &Self::Item) -> Result<String, Self::Error>;
}

#[derive(PartialEq)]
pub enum ItemType {
    File,
    Directory,
}

#[derive(Clone, Debug)]
pub enum BBPage {
    Me,
    CourseList { user_id: String },
    Course { id: String },
    Folder { url: String },
}

impl BBPage {
    fn url(&self) -> String {
        let path = match self {
            Self::Me => "/learn/api/v1/users/me?expand=systemRoles,insRoles".into(),
            Self::CourseList { user_id } => format!("/learn/api/v1/users/{user_id}/memberships?expand=course.effectiveAvailability,course.permissions,courseRole&includeCount=true&limit=10000"),
            Self::Course { id } => {
                format!("/webapps/blackboard/execute/announcement?method=search&course_id={id}")
            }
            Self::Folder { url } => {
                url.clone()
            }
        };
        format!("{BB_BASE_URL}{path}")
    }
}

pub struct BBAPIClient {
    cookies: String,
    agent: Agent,
    all_courses: bool,
    cache: RefCell<HashMap<CourseItem, Vec<u8>>>,
}

impl BBAPIClient {
    pub fn new(cookies: String, all_courses: bool) -> Self {
        let agent: Agent = AgentBuilder::new()
            .timeout_read(Duration::from_secs(5))
            .timeout_write(Duration::from_secs(5))
            .build();
        Self {
            cookies,
            agent,
            all_courses,
            cache: RefCell::new(HashMap::new()),
        }
    }

    pub fn get_page(&self, page: BBPage) -> Result<String, BBError> {
        self.agent
            .get(&page.url())
            .set("Cookie", &self.cookies)
            .call()
            .map_err(|err| BBError::FailedToGetPage(page.clone(), err))?
            .into_string()
            .map_err(|err| BBError::FailedToReadPageContents(page, err))
    }

    pub fn get_me(&self) -> Result<User, BBError> {
        let json = self.get_page(BBPage::Me)?;
        serde_json::from_str(&json).map_err(BBError::FailedToParseMe)
    }

    pub fn get_download_file_name(&self, url: &str) -> Result<String, BBError> {
        let url = &format!("{}{}", BB_BASE_URL, url);
        let response = self
            .agent
            .head(url)
            .set("Cookie", &self.cookies)
            .call()
            .map_err(BBError::FailedToGetHeaders)?;

        let last_component: String = response.get_url().split('/').last().unwrap().into();
        let file_name = last_component.split('?').next().unwrap();
        Ok(PctStr::new(file_name)
            .map(PctStr::decode)
            .unwrap_or(file_name.to_owned()))
    }

    fn get_courses(&self) -> Result<Vec<Course>, BBError> {
        let user_id = self.get_me()?.id;
        let json = self.get_page(BBPage::CourseList { user_id })?;
        let memberships_data: CourseMemberships =
            serde_json::from_str(&json).map_err(BBError::FailedToParseMemberships)?;
        Ok(memberships_data
            .results
            .into_iter()
            .filter(|course_entry| {
                self.all_courses || {
                    let now = OffsetDateTime::now_utc();
                    if let (Some(start), Some(end)) = (
                        course_entry.course.term.start_date,
                        course_entry.course.term.end_date,
                    ) {
                        start <= now && now <= end
                    } else {
                        false
                    }
                }
            })
            .map(|course_entry| course_entry.into())
            .collect())
    }

    fn get_course_contents(&self, course: &Course) -> Result<Vec<CourseItem>, BBError> {
        let html = self.get_page(BBPage::Course {
            id: course.id.clone(),
        })?;
        Ok(Self::parse_course_sidebar(&html)
            .unwrap_or_default()
            .into_iter()
            .map(|entry| entry.into())
            .collect())
    }

    /// url should be from a CourseItemContent::Folder
    fn get_directory_contents(&self, url: String) -> Result<Vec<CourseItem>, BBError> {
        let html = self.get_page(BBPage::Folder { url })?;
        Ok(Self::parse_folder_contents(&html)?
            .into_iter()
            .map(|entry| entry.into())
            .collect())
    }

    fn get_attachment_directory(&self, item: &CourseItem) -> Result<Vec<CourseItem>, BBError> {
        item.attachments
            .iter()
            .map(|url| {
                let name = self.get_download_file_name(url)?;

                Ok(CourseItem {
                    name,
                    content: Some(CourseItemContent::FileUrl(url.to_string())),
                    description: None,
                    attachments: vec![],
                })
            })
            .collect()
    }

    fn get_course_item_size(&self, item: &CourseItem) -> Result<usize, BBError> {
        Ok(match &item.content {
            Some(content) => match content {
                CourseItemContent::FileUrl(url) => {
                    let url = &format!("{}{}", BB_BASE_URL, url);
                    let response = self
                        .agent
                        .head(url)
                        .set("Cookie", &self.cookies)
                        .call()
                        .map_err(|err| BBError::FailedToGetHeaders(err))?;
                    response
                        .header("Content-Length")
                        .ok_or(BBError::MissingContentLengthHeader)?
                        .parse()
                        .map_err(|err| BBError::InvalidContentLengthHeader(err))?
                }
                CourseItemContent::FolderUrl(_) => unreachable!(),
                CourseItemContent::Link(url) => create_link_file(url).len(),
            },
            None => match &item.description {
                Some(desc) => desc.len(),
                None => 0,
            },
        })
    }

    fn get_course_item_contents(&self, item: &CourseItem) -> Result<Vec<u8>, BBError> {
        let mut cache = self.cache.borrow_mut();
        if cache.contains_key(item) {
            return Ok(cache[item].clone());
        }
        let bytes = match &item.content {
            Some(content) => match content {
                CourseItemContent::FileUrl(url) => {
                    let url = &format!("{}{}", BB_BASE_URL, url);
                    let response = self
                        .agent
                        .get(url)
                        .set("Cookie", &self.cookies)
                        .call()
                        .map_err(|_| BBError::FailedToGetContents(item.clone(), None))?; // TODO: Reach inside and check the error type
                    let mut bytes = Vec::new();
                    response
                        .into_reader()
                        .read_to_end(&mut bytes)
                        .map_err(|e| {
                            BBError::FailedToGetContents(
                                item.clone(),
                                e.raw_os_error().map(Errno::from_i32),
                            )
                        })?;
                    bytes
                }
                //CourseItemContent::FolderUrl(_) => unreachable!(),
                CourseItemContent::FolderUrl(_) => vec![],
                CourseItemContent::Link(url) => create_link_file(url).bytes().collect(),
            },
            None => match &item.description {
                Some(desc) => desc.bytes().collect(),
                None => vec![],
            },
        };
        cache.insert(item.clone(), bytes.clone());
        Ok(bytes)
    }
}

#[derive(Clone, Debug)]
pub struct SynthesizedFile {
    pub name: String,
    pub contents: String,
}

#[derive(Clone, Debug)]
pub struct SynthesizedDirectory {
    pub name: String,
    pub contents: Vec<Item>,
}

#[derive(Clone, Debug)]
pub enum Item {
    Course(Course),
    CourseItem(CourseItem),
    SynthesizedFile(SynthesizedFile),
    SynthesizedDirectory(SynthesizedDirectory),
}

#[derive(Debug)]
pub enum BBError {
    FailedToGetPage(BBPage, ureq::Error),
    FailedToReadPageContents(BBPage, std::io::Error),
    FailedToGetContents(CourseItem, Option<Errno>),
    FailedToGetHeaders(ureq::Error),
    MissingContentLengthHeader,
    InvalidContentLengthHeader(ParseIntError),
    FailedToWebScrapeFolder(anyhow::Error),
    FailedToParseMemberships(serde_json::Error),
    FailedToParseMe(serde_json::Error),
    NotAFile(Item),
}

impl Into<Errno> for BBError {
    fn into(self) -> Errno {
        eprintln!("{:?}", self);
        // TODO: Choose errnos more carefully
        match self {
            Self::FailedToGetPage(_, _)
            | Self::FailedToGetContents(_, _)
            | Self::FailedToGetHeaders(_) => Errno::ENETRESET,
            Self::FailedToReadPageContents(_, _)
            | Self::MissingContentLengthHeader
            | Self::InvalidContentLengthHeader(_)
            | Self::FailedToWebScrapeFolder(_)
            | Self::FailedToParseMemberships(_)
            | Self::FailedToParseMe(_)
            | Self::NotAFile(_) => Errno::EIO,
        }
    }
}

impl Display for BBError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        // TODO: Possibly create nicer descriptions
        f.write_fmt(format_args!("{:?}", self))
    }
}

impl Error for BBError {}

impl BBClient for BBAPIClient {
    type Item = Item;
    type Error = BBError;

    fn get_root(&self) -> Result<Self::Item, Self::Error> {
        Ok(Item::SynthesizedDirectory(SynthesizedDirectory {
            name: "root".into(),
            contents: self.get_courses()?.into_iter().map(Item::Course).collect(),
        }))
    }

    fn get_children(&self, path: Vec<&Item>) -> Result<Option<Vec<Item>>, BBError> {
        if let Some(item) = path.last() {
            match item {
                Item::Course(course) => Ok(Some(
                    self.get_course_contents(course)?
                        .into_iter()
                        .map(Item::CourseItem)
                        .collect(),
                )),
                Item::CourseItem(course_item) => match &course_item.content {
                    Some(CourseItemContent::Link(_)) | Some(CourseItemContent::FileUrl(_)) => {
                        Ok(None)
                    }
                    Some(CourseItemContent::FolderUrl(url)) => Ok(Some(
                        self.get_directory_contents(url.clone())?
                            .into_iter()
                            .map(Item::CourseItem)
                            .collect(),
                    )),
                    // TODO: attachments
                    None => Ok(None),
                },
                Item::SynthesizedDirectory(directory) => Ok(Some(directory.contents.clone())),
                Item::SynthesizedFile(_) => Ok(None),
            }
        } else {
            Ok(Some(
                self.get_courses()?.into_iter().map(Item::Course).collect(),
            ))
        }
    }

    fn get_size(&self, item: &Item) -> Result<usize, BBError> {
        match item {
            Item::Course(_) | Item::SynthesizedDirectory(_) => Err(BBError::NotAFile(item.clone())),
            Item::SynthesizedFile(file) => Ok(file.contents.len()),
            Item::CourseItem(course_item) => self.get_course_item_size(course_item),
        }
    }

    fn get_contents(&self, item: &Item) -> Result<Vec<u8>, BBError> {
        match item {
            Item::Course(_) | Item::SynthesizedDirectory(_) => Err(BBError::NotAFile(item.clone())),
            Item::SynthesizedFile(file) => Ok(file.contents.as_bytes().to_vec()),
            Item::CourseItem(course_item) => self.get_course_item_contents(course_item),
        }
    }

    fn get_type(&self, item: &Item) -> ItemType {
        match item {
            Item::Course(_) | Item::SynthesizedDirectory(_) => ItemType::Directory,
            Item::SynthesizedFile(_) => ItemType::File,
            Item::CourseItem(course_item) => match course_item.content {
                Some(CourseItemContent::FileUrl(_)) | Some(CourseItemContent::Link(_)) => {
                    ItemType::File
                }
                Some(CourseItemContent::FolderUrl(_)) => ItemType::Directory,
                None => ItemType::File,
            },
        }
    }

    fn get_name(&self, item: &Item) -> Result<String, BBError> {
        Ok(match item {
            Item::Course(course) => course.short_name.clone(),
            Item::SynthesizedDirectory(directory) => directory.name.clone(),
            Item::SynthesizedFile(file) => file.name.clone(),
            Item::CourseItem(course_item) => match &course_item.content {
                Some(CourseItemContent::FileUrl(file)) => self.get_download_file_name(file)?,
                Some(CourseItemContent::FolderUrl(_)) => course_item.name.clone(),
                Some(CourseItemContent::Link(_)) => {
                    format!("{}.{}", course_item.name, LINK_FILE_EXT)
                }
                // TODO: txt file extension when necessary
                None => course_item.name.clone(),
            },
        })
    }
}

#[cfg(test)]
mod tests {
    use dotenv::dotenv;
    use std::env;

    use super::BBAPIClient;

    fn get_client() -> BBAPIClient {
        dotenv().ok();
        let cookies = env::var("BBCOOKIE").unwrap();
        BBAPIClient::new(cookies, true)
    }

    #[test]
    fn test_req() {
        let client = get_client();
        client.get_page(super::BBPage::Me).unwrap();
    }

    #[test]
    fn test_get_me() {
        let client = get_client();
        client.get_me().unwrap();
    }
}
