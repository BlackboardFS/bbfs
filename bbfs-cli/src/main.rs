use std::fs::File;
use std::io::Write;
use std::path::{Path, PathBuf};

use argh::FromArgs;
use daemonize_me::Daemon;
use etcetera::BaseStrategy;
use fuser::MountOption;
use url::Url;

use bbfs_fuse::Bbfs;
use bbfs_scrape::client::BbApiClient;

#[derive(FromArgs)]
/// A CLI tool to authenticate to and mount BlackboardFS
struct BbfsCli {
    /// show all enrolled courses
    #[argh(switch, short = 'a')]
    all: bool,
    /// runs fs service in foreground
    #[argh(switch, short = 'm')]
    monitor: bool,
    /// the path to mount the Blackboard filesystem at
    #[argh(positional)]
    mount_point: PathBuf,
}

fn main() -> anyhow::Result<()> {
    let args: BbfsCli = argh::from_env();
    let mount_point = args.mount_point.canonicalize().unwrap();

    let strategy = etcetera::choose_base_strategy().unwrap();
    let data_dir = {
        let mut data_dir = strategy.data_dir();
        data_dir.push("blackboardfs");
        std::fs::create_dir_all(&data_dir).unwrap();
        data_dir
    };
    let wry_data_dir = data_dir.join("wry");

    let cookie_file = data_dir.join("cookie");
    let stdout =
        File::create(data_dir.join("stdout.log")).expect("failed to create stdout log file");
    let stderr =
        File::create(data_dir.join("stderr.log")).expect("failed to create stderr log file");

    let bb_url = Url::parse("https://learn.uq.edu.au/").unwrap();

    let cookies = find_cookies(&cookie_file, wry_data_dir, &bb_url).unwrap();

    if !args.monitor {
        Daemon::new().stdout(stdout).stderr(stderr).start().unwrap();
    }

    let client = BbApiClient::new(cookies, args.all);
    fuser::mount2(
        Bbfs::new(client).expect("failed to initialize Blackboard client"),
        mount_point,
        &[MountOption::AutoUnmount, MountOption::RO],
    )
    .unwrap();
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

fn find_cookies(cookie_file: &Path, wry_data_dir: PathBuf, bb_url: &Url) -> Option<String> {
    std::fs::read_to_string(cookie_file)
        .ok()
        .and_then(|cookie| cookie_valid(&cookie, bb_url).then_some(cookie))
        .or_else(|| {
            let cookie = cookie_monster::eat_user_cookies(wry_data_dir, cookie_file)
                .expect("cookie monster failed");

            cookie_valid(&cookie, bb_url).then(move || {
                if let Ok(mut file) = File::create(cookie_file) {
                    if file.write_all(cookie.as_bytes()).is_err() {
                        eprintln!("Failed to write cookie");
                    }
                } else {
                    eprintln!("Failed to open cookie file");
                }
                cookie
            })
        })
}

// TODO(theonlymrcat): This will panic if your internet is down
fn cookie_valid(cookie: &str, bb_url: &Url) -> bool {
    redirecting_agent()
        .request_url("GET", bb_url)
        .set("cookie", cookie)
        .call()
        .map(|response| response.get_url().starts_with("https://learn.uq.edu.au/"))
        .unwrap()
}
