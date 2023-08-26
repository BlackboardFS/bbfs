use anyhow::anyhow;
use soup::prelude::*;

#[derive(Debug)]
struct Content {
    title: String,
    link: Option<String>,
    description: Option<String>,
    // vec of urls
    attachments: Vec<String>,
}

fn get_folder_contents(_url: String) -> anyhow::Result<Vec<Content>> {
    // https://learn.uq.edu.au/webapps/blackboard/content/listContent.jsp?course_id={course_id}&content_id={content_id}&mode=reset
    let _html = include_str!("/home/benjamin/doc/bb_listContent_eg.html");
    let soup = Soup::new(_html);

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
                attachments,
            })
        })
        .filter(|r| r.is_ok())
        .collect::<anyhow::Result<Vec<_>>>();

    Ok(contents)
}
