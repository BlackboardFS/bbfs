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

        soup.tag("ul")
            .attr("class", "contentList")
            .find()
            .ok_or(anyhow!("There was no contentList"))?
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
                    .filter(|url| !url.starts_with("#"))
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
                        let re = Regex::new("(?s)<script.*?>.*?</script>").unwrap();
                        let html = elem.display();
                        let html = re.replace_all(&html, "");
                        let re = Regex::new("<br></br>").unwrap();
                        let html = re.replace_all(&html, "\n");
                        let re = Regex::new("<br>").unwrap();
                        let html = re.replace_all(&html, "\n");
                        Soup::new(&html).text().trim().into()
                    });

                let icon = elem
                    .tag("img")
                    .attr("class", "item_icon")
                    .find()
                    .ok_or(anyhow!("Item had no icon"))?
                    .get("src")
                    .ok_or(anyhow!("Icon had no src tag"))?;

                let file_name = if link.clone().is_some_and(|l| file.is_match(&l)) {
                    self.get_download_file_name(link.as_ref().unwrap()).ok()
                } else if attachments.is_empty() && link.is_none() {
                    Some(format!("{title}.txt"))
                } else {
                    None
                };

                if attachments.len() == 1 && link == None {
                    let file_name = self.get_download_file_name(&attachments[0])?;
                    Ok(Content {
                        title: file_name.clone(),
                        link: Some(attachments[0].clone()),
                        description,
                        file_name: Some(file_name),
                        icon,
                        attachments: vec![],
                    })
                } else {
                    Ok(Content {
                        title,
                        link,
                        description,
                        file_name,
                        icon,
                        attachments,
                    })
                }
            })
            .filter(|r| r.is_ok())
            .collect::<anyhow::Result<Vec<_>>>()
    }
}
