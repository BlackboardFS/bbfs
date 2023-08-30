use anyhow::anyhow;
use regex::Regex;
use soup::prelude::*;

use crate::{client::BbApiClient, CourseItem, CourseItemContent};

impl BbApiClient {
    pub fn parse_course_sidebar(html: &str) -> anyhow::Result<Vec<CourseItem>> {
        Soup::new(html)
            .attr("class", "courseMenu")
            .find()
            .ok_or(anyhow!("Couldn't find courseMenu class item"))?
            .tag("a")
            .find_all()
            .map(|a| {
                let url = a
                    .get("href")
                    .ok_or(anyhow!("Some sidebar <a> tag didnt have a href?!?!?!"))?;

                let re = Regex::new(
                    r"/webapps/blackboard/content/listContent.jsp\?course_id=.*&content_id=.*",
                )
                .unwrap();

                let content = match re.is_match(&url) {
                    true => CourseItemContent::FolderUrl(url),
                    false => CourseItemContent::Link(url),
                };
                let name = a.text();

                Ok(CourseItem {
                    name,
                    content: Some(content),
                    description: None,
                    attachments: vec![],
                })
            })
            .collect::<anyhow::Result<Vec<_>>>()
    }
}
