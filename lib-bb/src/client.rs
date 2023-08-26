use crate::{
    course_main_data::get_course_sidebar, memberships_data::MembershipsData, Course, CourseItem,
    CourseItemContent, User,
};
use nix::errno::Errno;
use std::{collections::HashMap, time::Duration};
use ureq::{Agent, AgentBuilder};

const BB_BASE_URL: &str = "https://learn.uq.edu.au";

pub trait BBClient {
    fn get_courses(&self) -> Result<Vec<Course>, Errno>;
    fn get_course_contents(&self, course: &Course) -> Result<Vec<CourseItem>, Errno>;

    fn get_directory_contents(&self, url: String) -> Result<Vec<CourseItem>, Errno>;

    fn get_item_size(&self, item: &CourseItem) -> Result<usize, Errno>;
    fn get_item_contents(&mut self, item: &CourseItem) -> Result<&[u8], Errno>;
}

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
    cache: HashMap<CourseItem, Vec<u8>>,
}

#[derive(Debug)]
pub enum BBClientError {
    IO(std::io::Error),
    UReq(ureq::Error),
    Serde(serde_json::Error),
}

impl From<std::io::Error> for BBClientError {
    fn from(value: std::io::Error) -> Self {
        Self::IO(value)
    }
}

impl From<ureq::Error> for BBClientError {
    fn from(value: ureq::Error) -> Self {
        Self::UReq(value)
    }
}

impl From<serde_json::Error> for BBClientError {
    fn from(value: serde_json::Error) -> Self {
        Self::Serde(value)
    }
}

impl BBAPIClient {
    pub fn new(cookies: String) -> Self {
        let agent: Agent = AgentBuilder::new()
            .timeout_read(Duration::from_secs(5))
            .timeout_write(Duration::from_secs(5))
            .build();
        Self {
            cookies,
            agent,
            cache: HashMap::new(),
        }
    }

    pub fn get_page(&self, page: BBPage) -> Result<String, Errno> {
        Ok(self
            .agent
            .get(&page.url())
            .set("Cookie", &self.cookies)
            .call()
            .map_err(|_| Errno::ENETRESET)? // TODO: Reach inside and check the error type
            .into_string()
            .map_err(|_| Errno::EIO)?)
    }

    pub fn get_me(&self) -> Result<User, Errno> {
        let json = self.get_page(BBPage::Me)?;
        Ok(serde_json::from_str(&json).map_err(|_| Errno::EIO)?)
    }

    pub fn get_download_file_name(&self, url: &str) -> anyhow::Result<String> {
        let url = &format!("{}{}", BB_BASE_URL, url);
        let response = self
            .agent
            .head(url)
            .set("Cookie", &self.cookies)
            .call()?;
        Ok(response.get_url().split('/').last().unwrap().into())
    }
}

impl BBClient for BBAPIClient {
    fn get_courses(&self) -> Result<Vec<Course>, Errno> {
        let user_id = self.get_me()?.id;
        let json = self.get_page(BBPage::CourseList { user_id })?;
        let memberships_data: MembershipsData =
            serde_json::from_str(&json).map_err(|_| Errno::EIO)?;
        Ok(memberships_data
            .results
            .into_iter()
            .map(|course_entry| course_entry.into())
            .collect())
    }

    fn get_course_contents(&self, course: &Course) -> Result<Vec<CourseItem>, Errno> {
        let html = self.get_page(BBPage::Course {
            id: course.id.clone(),
        })?;
        println!("{:?}", course.id);
        Ok(get_course_sidebar(&html)
            .unwrap_or_default()
            .into_iter()
            .map(|entry| entry.into())
            .collect())
    }

    /// url should be from a CourseItemContent::Folder
    fn get_directory_contents(&self, url: String) -> Result<Vec<CourseItem>, Errno> {
        let html = self.get_page(BBPage::Folder { url })?;
        Ok(self
            .get_folder_contents(&html)
            .map_err(|_| Errno::EIO)?
            .into_iter()
            .map(|entry| entry.into())
            .collect())
    }

    fn get_item_size(&self, item: &CourseItem) -> Result<usize, Errno> {
        Ok(match &item.content {
            Some(content) => match content {
                CourseItemContent::FileUrl(url) => {
                    let url = &format!("{}{}", BB_BASE_URL, url);
                    let response = self
                        .agent
                        .head(url)
                        .set("Cookie", &self.cookies)
                        .call()
                        .map_err(|_| Errno::ENETRESET)?;
                    response
                        .header("Content-Length")
                        .ok_or(Errno::EIO)?
                        .parse()
                        .map_err(|_| Errno::EIO)?
                }
                //CourseItemContent::FolderUrl(_) => unreachable!(),
                CourseItemContent::FolderUrl(_) => 0,
                CourseItemContent::Link(url) => url.len(),
            },
            None => match &item.description {
                Some(desc) => desc.len(),
                None => 0,
            },
        })
    }

    fn get_item_contents(&mut self, item: &CourseItem) -> Result<&[u8], Errno> {
        if self.cache.contains_key(item) {
            return Ok(&self.cache[item]);
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
                        .map_err(|_| Errno::ENETRESET)?; // TODO: Reach inside and check the error type
                    let mut bytes = Vec::new();
                    response
                        .into_reader()
                        .read_to_end(&mut bytes)
                        .map_err(|e| e.raw_os_error().map(Errno::from_i32).unwrap_or(Errno::EIO))?;
                    bytes
                }
                //CourseItemContent::FolderUrl(_) => unreachable!(),
                CourseItemContent::FolderUrl(_) => vec![],
                CourseItemContent::Link(url) => url.bytes().collect(),
            },
            None => match &item.description {
                Some(desc) => desc.bytes().collect(),
                None => vec![],
            },
        };
        self.cache.insert(item.clone(), bytes);
        Ok(self.cache[item].as_slice())
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
        BBAPIClient::new(cookies)
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
