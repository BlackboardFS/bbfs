use anyhow::anyhow;
use regex::Regex;
use soup::prelude::*;

// https://learn.uq.edu.au/webapps/blackboard/execute/courseMain?course_id={course_id}
// gives html that has the sidebar

#[derive(Debug)]
struct SidebarEntry {
    link: SidebarLink,
    name: String,
}

#[derive(Debug)]
enum SidebarLink {
    Directory(String),
    Link(String),
}

impl SidebarLink {
    fn from_url(url: String) -> Self {
        let re =
            Regex::new(r"/webapps/blackboard/content/listContent.jsp\?course_id=.*&content_id=.*")
                .unwrap();
        match re.is_match(&url) {
            true => SidebarLink::Directory(url),
            false => SidebarLink::Link(url),
        }
    }
}

#[allow(unreachable_code)]
fn get_course_sidebar(html: &str) -> anyhow::Result<()> {
    let soup = Soup::new(html);

    let side_bar = soup
        .tag("ul")
        .attr("class", "courseMenu")
        .find()
        .ok_or(anyhow!("Couldn't find courseMenu class item"))?
        .tag("a")
        .find_all()
        .map(|a| {
            let link = SidebarLink::from_url(
                a.get("href")
                    .ok_or(anyhow!("Some sidebar <a> tag didnt have a href?!?!?!"))?,
            );
            let name = a.text();

            Ok(SidebarEntry { link, name })
        })
        .collect::<anyhow::Result<Vec<_>>>()?;
    println!("{:?}", side_bar);

    Ok(())
}
