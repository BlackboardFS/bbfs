use std::fs::File;
use std::io::BufReader;
use std::path::{Path, PathBuf};

use argh::FromArgs;
use cookie_store::{Cookie, CookieStore};
use etcetera::BaseStrategy;
use url::Url;

// #[derive(FromArgs)]
// struct BbfsCli {
//     mount_point: PathBuf,
// }

fn main() -> anyhow::Result<()> {
    let strategy = etcetera::choose_base_strategy().unwrap();
    let data_dir = {
        let mut data_dir = strategy.data_dir();
        data_dir.push("blackboardfs");
        std::fs::create_dir_all(&data_dir).unwrap();
        data_dir
    };

    let cookie_file = data_dir.join("cookies.json");

    let bb_url = Url::parse("https://learn.uq.edu.au/").unwrap();

    find_cookies(&cookie_file, &bb_url).unwrap();

    Ok(())
}

fn redirecting_agent() -> ureq::Agent {
    ureq::AgentBuilder::new().redirects(32).build()
}

trait RequestExt {
    fn with_cookies(self, cookies: &[String]) -> Self;
}

impl RequestExt for ureq::Request {
    fn with_cookies(self, cookies: &[String]) -> Self {
        self.set(
            "cookie",
            &cookies
                .iter()
                .cloned() // TODO(theonlymrcat): I couldn't be bothered
                .reduce(|mut megacookie, cookie| {
                    megacookie.push(';');
                    megacookie.push_str(&cookie);
                    megacookie
                })
                .unwrap_or_default(),
        )
    }
}

fn find_cookies(cookie_file: &Path, bb_url: &Url) -> Option<Vec<String>> {
    File::open(cookie_file)
        .ok()
        .and_then(|file| serde_json::from_reader::<_, Vec<String>>(BufReader::new(file)).ok())
        .and_then(|cookies| cookies_valid(&cookies, bb_url).then_some(cookies))
        .or_else(|| {
            let cookies = cookie_monster::eat_user_cookies();
            cookies_valid(&cookies, bb_url).then(move || {
                if let Ok(mut file) = File::create(cookie_file) {
                    if serde_json::to_writer_pretty(&mut file, &cookies).is_err() {
                        eprintln!("Failed to save cookies to json");
                    }
                } else {
                    eprintln!("Failed to write to cookie file");
                }
                cookies
            })
        })
}

// TODO(theonlymrcat): This will panic if your internet is down
fn cookies_valid(cookies: &[String], bb_url: &Url) -> bool {
    redirecting_agent()
        .request_url("GET", bb_url)
        .with_cookies(cookies)
        .call()
        .map(|response| response.get_url().starts_with("https://learn.uq.edu.au/"))
        .unwrap()
}

// --- The cookie store isn't passing cookies properly, so this code isn't working ---
fn create_valid_agent(cookie_file: &Path, bb_url: &Url) -> Option<ureq::Agent> {
    File::open(cookie_file)
        .ok()
        .and_then(|file| CookieStore::load_json(BufReader::new(file)).ok())
        .and_then(|cookie_store| {
            let agent = ureq::AgentBuilder::new().cookie_store(cookie_store).build();
            agent_cookies_valid(&agent, bb_url).then_some(agent)
        })
        .or_else(|| {
            Some(
                CookieStore::from_cookies(
                    cookie_monster::eat_user_cookies()
                        .into_iter()
                        .map(|s| dbg!(Cookie::parse(s, bb_url))),
                    true,
                )
                .unwrap(),
            )
            .and_then(|cookie_store| {
                let agent = ureq::AgentBuilder::new().cookie_store(cookie_store).build();
                if let Ok(mut file) = File::create(cookie_file) {
                    if agent.cookie_store().save_json(&mut file).is_err() {
                        eprintln!("Failed to save cookies to json");
                    }
                } else {
                    eprintln!("Failed to write to cookie file");
                }
                agent_cookies_valid(&agent, bb_url).then(move || agent)
            })
        })
}

// TODO(theonlymrcat): This will panic if your internet is down
fn agent_cookies_valid(agent: &ureq::Agent, bb_url: &Url) -> bool {
    agent
        .request_url("GET", bb_url)
        .call()
        .map(|response| response.get_url().starts_with("https://learn.uq.edu.au/"))
        .unwrap()
}
