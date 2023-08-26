use crate::client::BBAPIClient;
use anyhow::anyhow;
use regex::Regex;
use soup::prelude::*;

#[derive(Debug)]
pub struct Content {
    pub title: String,
    pub link: Option<String>,
    pub description: Option<String>,
    pub file_name: Option<String>,
    // url
    pub icon: String,
    // vec of urls
    pub attachments: Vec<String>,
}

impl BBAPIClient {
    pub fn get_folder_contents(&self, html: &str) -> anyhow::Result<Vec<Content>> {
        // https://learn.uq.edu.au/webapps/blackboard/content/listContent.jsp?course_id={course_id}&content_id={content_id}&mode=reset
        let file = Regex::new(r".*/bbcswebdav/.*").unwrap();
        let soup = Soup::new(html);

        let contents = soup
            .tag("ul")
            .attr("class", "contentList")
            .find()
            .ok_or(anyhow!("There was no contentList"))?
            .children()
            .map(|elem| {
                let title_elem = elem
                    .tag("h3")
                    .find()
                    .ok_or(anyhow!("item didnt have a header"))?
                    .children()
                    .nth(3)
                    .ok_or(anyhow!("header had nothing in it"))?;
                let title = title_elem.text();
                let link = title_elem.get("href");

                let description = elem
                    .tag("div")
                    .attr("class", "vtbegenerated_div")
                    .find()
                    .map(|elem| elem.text());

                let file_name = link
                    .as_ref()
                    .and_then(|l| {
                        file.is_match(l)
                            .then(|| self.get_download_file_name(l).ok())
                    })
                    .flatten();

                println!("{:?}", file_name);

                let icon = elem
                    .tag("img")
                    .attr("class", "item_icon")
                    .find()
                    .ok_or(anyhow!("Item had no icon"))?
                    .get("src")
                    .ok_or(anyhow!("Icon had no src tag"))?;

                let attachments = elem
                    .tag("span")
                    .attr("class", "contextMenuContainer")
                    .find_all()
                    .filter_map(|elem| elem.get("bb:menuGeneratorUrl"))
                    .collect();

                Ok(Content {
                    title,
                    link,
                    description,
                    file_name,
                    icon,
                    attachments,
                })
            })
            .filter(|r| r.is_ok())
            .collect::<anyhow::Result<Vec<_>>>();

        contents
    }
}
