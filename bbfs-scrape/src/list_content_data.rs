use crate::{
    client::{BbApiClient, BbError},
    CourseItem, CourseItemContent,
};
use anyhow::anyhow;
use regex::Regex;
use soup::prelude::*;

impl BbApiClient {
    pub fn parse_folder_contents(html: &str) -> Result<Vec<CourseItem>, BbError> {
        Soup::new(html)
            .tag("ul")
            .attr("class", "contentList")
            .find()
            .ok_or(BbError::FailedToWebScrapeFolder(anyhow!(
                "There was no contentList"
            )))?
            .children()
            .map(|elem| {
                let attachments: Vec<_> = elem
                    .attr("class", "attachments")
                    .find_all()
                    .flat_map(|elem| -> Vec<String> {
                        elem.tag("li")
                            .find_all()
                            .filter_map(|elem| {
                                elem.tag("a").find().and_then(|link| link.get("href"))
                            })
                            .collect()
                    })
                    .filter(|url| !url.starts_with('#'))
                    .collect();

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
                    .attr("class", "vtbegenerated")
                    .find()
                    .map(|elem| {
                        // terrible code warning!
                        let html = elem.display();
                        let re = Regex::new("(?s)<script.*?>.*?</script>").unwrap();
                        let html = re.replace_all(&html, "");
                        let re = Regex::new("<br></br>").unwrap();
                        let html = re.replace_all(&html, "\n");
                        let re = Regex::new("<br>").unwrap();
                        let html = re.replace_all(&html, "\n");
                        Soup::new(&html).text().trim().into()
                    })
                    .filter(|s: &String| !s.is_empty());

                /*
                let icon = elem
                    .tag("img")
                    .attr("class", "item_icon")
                    .find()
                    .ok_or(anyhow!("Item had no icon"))?
                    .get("src")
                    .ok_or(anyhow!("Icon had no src tag"))?;
                    */

                // Replace / with - so that the fs doesnt explode
                let re = Regex::new("/").unwrap();
                let title = re.replace_all(&title, r"-").into();

                Ok(CourseItem {
                    name: title,
                    content: if attachments.len() == 1 && link.is_none() {
                        Some(attachments[0].clone())
                    } else {
                        link
                    }
                    .map(CourseItemContent::from_url),
                    description,
                    attachments,
                })
            })
            .filter(|r| r.is_ok())
            .collect::<anyhow::Result<Vec<_>>>()
            .map_err(BbError::FailedToWebScrapeFolder)
    }
}
