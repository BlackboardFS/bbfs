use anyhow::anyhow;
use soup::prelude::*;

// https://learn.uq.edu.au/webapps/blackboard/execute/courseMain?course_id={course_id}
// gives html that has the sidebar

#[derive(Debug)]
struct SidebarEntry {
    link: String,
    name: String,
}

#[allow(unreachable_code)]
fn get_course_sidebar(_course_id: String) -> anyhow::Result<()> {
    // get the html somehow
    let _html =
        todo!("This was an include_str!() to a download of the html that we will actually use");
    let soup = Soup::new(_html);

    let side_bar = soup
        .tag("ul")
        .attr("class", "courseMenu")
        .find()
        .ok_or(anyhow!("Couldn't find courseMenu class item"))?
        .tag("a")
        .find_all()
        .map(|a| {
            let link = a
                .get("href")
                .ok_or(anyhow!("Some sidebar <a> tag didnt have a href?!?!?!"))?;
            let name = a.text();

            Ok(SidebarEntry { link, name })
        })
        .collect::<anyhow::Result<Vec<_>>>()?;
    println!("{:?}", side_bar);

    Ok(())
}

#[test]
fn test_it() {
    get_course_sidebar(String::new()).expect("broke");

    panic!("Everything is fine actually");
}
