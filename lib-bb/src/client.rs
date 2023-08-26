use crate::{memberships_data::MembershipsData, Course, CourseItem, CourseItemType, User};
use std::time::Duration;
use ureq::{Agent, AgentBuilder};

const BB_BASE_URL: &str = "https://learn.uq.edu.au";

pub trait BBClient {
    fn get_courses(&self) -> Vec<Course>;
    fn get_course_contents(&self, course: &Course) -> Vec<CourseItem>;

    fn get_item_size(&self, item: &CourseItem) -> usize;
    fn get_item_contents(&self, item: &CourseItem) -> Vec<u8>;
}

pub struct BBMockClient;

impl BBClient for BBMockClient {
    fn get_courses(&self) -> Vec<Course> {
        vec![
            Course {
                short_name: "MATH2401".into(),
                full_name: "[MATH2401] Real analysis".into(),
                id: "".into(),
            },
            Course {
                short_name: "MATH1072".into(),
                full_name: "[MATH1072] Multivariate calculus".into(),
                id: "".into(),
            },
            Course {
                short_name: "STAT1301".into(),
                full_name: "[STAT1301] Advanced analysis of scientific data".into(),
                id: "".into(),
            },
            Course {
                short_name: "COMP3506".into(),
                full_name: "[COMP3506] Algorithms & data structures".into(),
                id: "".into(),
            },
        ]
    }

    fn get_course_contents(&self, course: &Course) -> Vec<CourseItem> {
        if course.short_name == "MATH2401" {
            vec![CourseItem {
                name: "Assignment 1".into(),
                url: Some("https://learn.uq.edu.au/bbcswebdav/pid-9222876-dt-content-rid-56218459_1/xid-56218459_1".into()),
                ty: CourseItemType::File,
            }]
        } else {
            vec![]
        }
    }

    fn get_item_size(&self, _item: &CourseItem) -> usize {
        10
    }

    fn get_item_contents(&self, _item: &CourseItem) -> Vec<u8> {
        "hellohello".into()
    }
}

pub enum BBPage {
    Me,
    CourseList { user_id: String },
    Course { id: String },
}

impl BBPage {
    fn url(&self) -> String {
        let path = match self {
            Self::Me => "/learn/api/v1/users/me?expand=systemRoles,insRoles".into(),
            Self::CourseList { user_id } => format!("learn/api/v1/users/{user_id}/memberships"),
            Self::Course { id } => {
                format!("/ultra/courses/{id}/cl/outline")
            }
        };
        format!("{BB_BASE_URL}{path}")
    }
}

pub struct BBAPIClient {
    cookies: String,
    agent: Agent,
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
        Self { cookies, agent }
    }

    pub fn get_page(&self, page: BBPage) -> Result<String, BBClientError> {
        Ok(self
            .agent
            .get(&page.url())
            .set("Cookie", &self.cookies)
            .call()?
            .into_string()?)
    }

    pub fn get_me(&self) -> Result<User, BBClientError> {
        let json = self.get_page(BBPage::Me)?;
        Ok(serde_json::from_str(&json)?)
    }
}

impl BBClient for BBAPIClient {
    fn get_courses(&self) -> Vec<Course> {
        let user_id = self.get_me().unwrap().id;
        let json = self.get_page(BBPage::CourseList { user_id }).unwrap();
        let memberships_data: MembershipsData = serde_json::from_str(&json).unwrap();
        memberships_data
            .results
            .into_iter()
            .map(|course_entry| course_entry.into())
            .collect()
    }

    fn get_course_contents(&self, course: &Course) -> Vec<CourseItem> {
        let _html = self.get_page(BBPage::Course {
            id: course.id.clone(),
        });
        todo!("Parse html");
    }

    fn get_item_size(&self, item: &CourseItem) -> usize {
        // TODO remove unwraps
        let response = self
            .agent
            .head(&item.url.as_ref().unwrap())
            .set("Cookie", &self.cookies)
            .call()
            .unwrap();
        response.header("Content-Length").unwrap().parse().unwrap()
    }

    fn get_item_contents(&self, item: &CourseItem) -> Vec<u8> {
        // TODO remove unwraps
        let response = self
            .agent
            .head(&item.url.as_ref().unwrap())
            .set("Cookie", &self.cookies)
            .call()
            .unwrap();
        let mut bytes = Vec::new();
        response.into_reader().read_to_end(&mut bytes).unwrap();
        bytes
    }
}

#[cfg(test)]
mod tests {
    use dotenv::dotenv;
    use std::env;

    use super::BBAPIClient;

    fn get_client() -> BBAPIClient {
        dotenv().unwrap();
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
