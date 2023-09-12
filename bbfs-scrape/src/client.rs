use std::collections::HashMap;
use std::error::Error;
use std::fmt::Debug;
use std::fmt::Display;
use std::num::ParseIntError;
use std::sync::Mutex;
use std::time::Duration;

use pct_str::PctStr;
use time::OffsetDateTime;
use ureq::{Agent, AgentBuilder};

use crate::{
    create_link_file, memberships_data::CourseMemberships, Course, CourseItem, CourseItemContent,
    Item, SynthesizedDirectory, User, LINK_FILE_EXT,
};

const BB_BASE_URL: &str = "https://learn.uq.edu.au";

pub trait BbClient: Sync {
    type Item: Clone + Send + Sync;

    fn get_root(&self) -> Result<Self::Item, BbError>;
    fn get_children(&self, path: Vec<&Self::Item>) -> Result<Vec<Self::Item>, BbError>;
    fn get_size(&self, item: &Self::Item) -> Result<usize, BbError>;
    fn get_contents(&self, item: &Self::Item) -> Result<Vec<u8>, BbError>;
    fn get_type(&self, item: &Self::Item) -> ItemType;
    fn get_name(&self, item: &Self::Item) -> Result<String, BbError>;
}

#[derive(PartialEq)]
pub enum ItemType {
    File,
    Directory,
}

#[derive(Clone, Debug)]
pub enum BbPage {
    Me,
    CourseList { user_id: String },
    Course { id: String },
    Folder { url: String },
}

impl BbPage {
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

pub struct BbApiClient {
    cookies: String,
    agent: Agent,
    all_courses: bool,
    // TODO: Consider a dashmap or similar
    cache: Mutex<HashMap<CourseItem, Vec<u8>>>,
}

impl BbApiClient {
    pub fn new(cookies: String, all_courses: bool) -> Self {
        let agent: Agent = AgentBuilder::new()
            .timeout_read(Duration::from_secs(5))
            .timeout_write(Duration::from_secs(5))
            .build();
        Self {
            cookies,
            agent,
            all_courses,
            cache: Mutex::new(HashMap::new()),
        }
    }

    fn get_page(&self, page: BbPage) -> Result<String, BbError> {
        self.agent
            .get(&page.url())
            .set("Cookie", &self.cookies)
            .call()
            .map_err(|err| BbError::FailedToGetPage(page.clone(), Box::new(err)))?
            .into_string()
            .map_err(|err| BbError::FailedToReadPageContents(page, err))
    }

    fn get_me(&self) -> Result<User, BbError> {
        let json = self.get_page(BbPage::Me)?;
        serde_json::from_str(&json).map_err(BbError::FailedToParseMe)
    }

    fn get_download_file_name(&self, url: &str) -> Result<String, BbError> {
        let url = &format!("{}{}", BB_BASE_URL, url);
        let response = self
            .agent
            .head(url)
            .set("Cookie", &self.cookies)
            .call()
            .map_err(|e| BbError::FailedToGetHeaders(Box::new(e)))?;

        let last_component: String = response.get_url().split('/').last().unwrap().into();
        let file_name = last_component.split('?').next().unwrap();
        Ok(PctStr::new(file_name)
            .map(PctStr::decode)
            .unwrap_or(file_name.to_owned()))
    }

    fn get_courses(&self) -> Result<Vec<Course>, BbError> {
        let user_id = self.get_me()?.id;
        let json = self.get_page(BbPage::CourseList { user_id })?;
        let memberships_data: CourseMemberships =
            serde_json::from_str(&json).map_err(BbError::FailedToParseMemberships)?;
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

    fn get_course_contents(&self, course: &Course) -> Result<Vec<CourseItem>, BbError> {
        let html = self.get_page(BbPage::Course {
            id: course.id.clone(),
        })?;
        Ok(Self::parse_course_sidebar(&html)
            .unwrap_or_default()
            .into_iter()
            .collect())
    }

    /// url should be from a CourseItemContent::Folder
    fn get_directory_contents(&self, url: String) -> Result<Vec<CourseItem>, BbError> {
        let html = self.get_page(BbPage::Folder { url })?;
        Ok(Self::parse_folder_contents(&html)?.into_iter().collect())
    }

    fn get_course_item_size(&self, item: &CourseItem) -> Result<usize, BbError> {
        Ok(match &item.content {
            Some(content) => match content {
                CourseItemContent::FileUrl(url) => {
                    let url = &format!("{}{}", BB_BASE_URL, url);
                    let response = self
                        .agent
                        .head(url)
                        .set("Cookie", &self.cookies)
                        .call()
                        .map_err(|err| BbError::FailedToGetHeaders(Box::new(err)))?;
                    response
                        .header("Content-Length")
                        .ok_or(BbError::MissingContentLengthHeader)?
                        .parse()
                        .map_err(BbError::InvalidContentLengthHeader)?
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

    fn get_course_item_contents(&self, item: &CourseItem) -> Result<Vec<u8>, BbError> {
        let mut cache = self.cache.lock().unwrap();
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
                        .map_err(|_| BbError::FailedToGetContents(item.clone(), None))?; // TODO: Reach inside and check the error type
                    let mut bytes = Vec::new();
                    response
                        .into_reader()
                        .read_to_end(&mut bytes)
                        .map_err(|e| BbError::FailedToGetContents(item.clone(), Some(e)))?;
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

#[derive(Debug)]
pub enum BbError {
    FailedToGetPage(BbPage, Box<ureq::Error>),
    FailedToReadPageContents(BbPage, std::io::Error),
    FailedToGetContents(CourseItem, Option<std::io::Error>),
    FailedToGetHeaders(Box<ureq::Error>),
    MissingContentLengthHeader,
    InvalidContentLengthHeader(ParseIntError),
    FailedToWebScrapeFolder(anyhow::Error),
    FailedToParseMemberships(serde_json::Error),
    FailedToParseMe(serde_json::Error),
    NotAFile(Item),
}

#[cfg(unix)]
impl From<BbError> for nix::errno::Errno {
    fn from(error: BbError) -> nix::errno::Errno {
        eprintln!("{:?}", error);
        // TODO: Choose errnos more carefully
        match error {
            BbError::FailedToGetPage(_, _)
            | BbError::FailedToGetContents(_, _)
            | BbError::FailedToGetHeaders(_) => nix::errno::Errno::ENETRESET,
            BbError::FailedToReadPageContents(_, _)
            | BbError::MissingContentLengthHeader
            | BbError::InvalidContentLengthHeader(_)
            | BbError::FailedToWebScrapeFolder(_)
            | BbError::FailedToParseMemberships(_)
            | BbError::FailedToParseMe(_) => nix::errno::Errno::EIO,
            BbError::NotAFile(_) => nix::errno::Errno::EISDIR,
        }
    }
}

#[cfg(windows)]
impl From<BbError> for winapi::shared::ntdef::NTSTATUS {
    fn from(error: BbError) -> winapi::shared::ntdef::NTSTATUS {
        use winapi::shared::ntstatus;
        eprintln!("{:?}", error);
        match error {
            BbError::FailedToGetPage(_, _)
            | BbError::FailedToGetContents(_, _)
            | BbError::FailedToGetHeaders(_) => ntstatus::STATUS_UNEXPECTED_NETWORK_ERROR,
            BbError::FailedToReadPageContents(_, _)
            | BbError::MissingContentLengthHeader
            | BbError::InvalidContentLengthHeader(_)
            | BbError::FailedToWebScrapeFolder(_)
            | BbError::FailedToParseMemberships(_)
            | BbError::FailedToParseMe(_) => ntstatus::STATUS_FILE_NOT_AVAILABLE,
            BbError::NotAFile(_) => ntstatus::STATUS_FILE_IS_A_DIRECTORY,
        }
    }
}

impl Display for BbError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        // TODO: Possibly create nicer descriptions
        f.write_fmt(format_args!("{:?}", self))
    }
}

impl Error for BbError {}

impl BbClient for BbApiClient {
    type Item = Item;

    fn get_root(&self) -> Result<Self::Item, BbError> {
        Ok(Item::SynthesizedDirectory(SynthesizedDirectory {
            name: "root".into(),
            contents: self.get_courses()?.into_iter().map(Item::Course).collect(),
        }))
    }

    fn get_children(&self, path: Vec<&Item>) -> Result<Vec<Item>, BbError> {
        if let Some(item) = path.last() {
            if self.get_type(item) != ItemType::Directory {
                unreachable!();
            }

            match item {
                Item::Course(course) => {
                    let link = format!("/ultra/courses/{}/cl/outline", course.id);

                    let mut items: Vec<_> = self
                        .get_course_contents(course)?
                        .into_iter()
                        .map(Item::CourseItem)
                        .collect();

                    items.push(Item::make_link_file("Blackboard", &link));

                    Ok(items)
                }
                Item::CourseItem(course_item) => {
                    let mut items: Vec<Item> = match &course_item.content {
                        Some(CourseItemContent::Link(link)) => {
                            vec![Item::make_link_file(&course_item.name, link)]
                        }
                        Some(CourseItemContent::FileUrl(url)) => {
                            vec![Item::CourseItem(CourseItem {
                                name: course_item.name.clone(),
                                content: Some(CourseItemContent::FileUrl(url.clone())),
                                description: None,
                                attachments: vec![],
                            })]
                        }
                        Some(CourseItemContent::FolderUrl(url)) => self
                            .get_directory_contents(url.clone())?
                            .into_iter()
                            .map(Item::CourseItem)
                            .collect(),
                        None => vec![],
                    };

                    items.append(&mut course_item.attachments_as_items());

                    items.extend(course_item.maybe_new_description_file());

                    items.extend(course_item.maybe_new_link_file());

                    let link = course_item.get_blackboard_link(path[path.len() - 2]);

                    items.push(Item::make_link_file("Blackboard", &link));

                    Ok(items)
                }
                Item::SynthesizedDirectory(directory) => Ok(directory.contents.clone()),
                Item::SynthesizedFile(_) => unreachable!(),
            }
        } else {
            Ok(self.get_courses()?.into_iter().map(Item::Course).collect())
        }
    }

    fn get_size(&self, item: &Item) -> Result<usize, BbError> {
        match item {
            Item::Course(_) | Item::SynthesizedDirectory(_) => Err(BbError::NotAFile(item.clone())),
            Item::SynthesizedFile(file) => Ok(file.contents.len()),
            Item::CourseItem(course_item) => self.get_course_item_size(course_item),
        }
    }

    fn get_contents(&self, item: &Item) -> Result<Vec<u8>, BbError> {
        match item {
            Item::Course(_) | Item::SynthesizedDirectory(_) => Err(BbError::NotAFile(item.clone())),
            Item::SynthesizedFile(file) => Ok(file.contents.as_bytes().to_vec()),
            Item::CourseItem(course_item) => self.get_course_item_contents(course_item),
        }
    }

    fn get_type(&self, item: &Item) -> ItemType {
        match item {
            Item::Course(_) | Item::SynthesizedDirectory(_) => ItemType::Directory,
            Item::SynthesizedFile(_) => ItemType::File,
            Item::CourseItem(course_item) => {
                if !course_item.attachments.is_empty()
                    || (course_item.description.is_some() && course_item.content.is_some())
                {
                    ItemType::Directory
                } else {
                    match course_item.content {
                        Some(CourseItemContent::FileUrl(_)) | Some(CourseItemContent::Link(_)) => {
                            ItemType::File
                        }
                        Some(CourseItemContent::FolderUrl(_)) => ItemType::Directory,
                        None => ItemType::File,
                    }
                }
            }
        }
    }

    fn get_name(&self, item: &Item) -> Result<String, BbError> {
        Ok(match item {
            Item::Course(course) => course.short_name.clone(),
            Item::SynthesizedDirectory(directory) => directory.name.clone(),
            Item::SynthesizedFile(file) => file.name.clone(),
            Item::CourseItem(course_item) => {
                if self.get_type(item) == ItemType::Directory {
                    course_item.name.clone()
                } else {
                    match &course_item.content {
                        Some(CourseItemContent::FileUrl(file)) => {
                            self.get_download_file_name(file)?
                        }
                        Some(CourseItemContent::FolderUrl(_)) => course_item.name.clone(),
                        Some(CourseItemContent::Link(_)) => {
                            format!("{}.{}", course_item.name, LINK_FILE_EXT)
                        }
                        None => {
                            if course_item.description.is_some() {
                                format!("{}.txt", course_item.name)
                            } else {
                                course_item.name.clone()
                            }
                        }
                    }
                }
            }
        })
    }
}
