pub enum BBPage {
    Me,
    CourseList,
    Course { id: String },
}

impl BBPage {
    fn url(&self) -> String {
        let path = match self {
            Self::Me => "/learn/api/v1/users/me?expand=systemRoles,insRoles".into(),
            Self::CourseList => "/ultra/course".into(),
            Self::Course { id } => {
                format!("/ultra/courses/{id}/cl/outline")
            }
        };
        format!("{BB_BASE_URL}{path}")
    }
}

const BB_BASE_URL: &str = "https://learn.uq.edu.au";

pub struct BBHTTPClient {
    cookies: String,
    agent: Agent,
}

use std::time::Duration;
use ureq::{Agent, AgentBuilder};

use crate::User;

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

impl BBHTTPClient {
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

#[cfg(test)]
mod tests {
    use dotenv::dotenv;
    use std::env;

    use super::BBHTTPClient;

    fn get_client() -> BBHTTPClient {
        dotenv().unwrap();
        let cookies = env::var("BBCOOKIE").unwrap();
        BBHTTPClient::new(cookies)
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
