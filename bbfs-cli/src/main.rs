use std::io::Write;
use std::path::PathBuf;
use std::{fs::File, path::Path};

use anyhow::anyhow;
use argh::FromArgs;
use cookie_monster::{is_cookie_valid, CookieMonster, HeadlessCookieMonster, WebViewCookieMonster};
use etcetera::BaseStrategy;
use url::Url;

#[cfg(windows)]
use bbfs_dokan::Bbfs;
#[cfg(unix)]
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
    /// uses headless auth flow
    #[argh(switch)]
    headless: bool,
    /// the path to mount the Blackboard filesystem at
    #[argh(positional)]
    mount_point: PathBuf,
}

impl BbfsCli {
    fn normalized_mount_point(&self) -> PathBuf {
        self.mount_point.canonicalize().unwrap()
    }
}

fn main() -> anyhow::Result<()> {
    let args: BbfsCli = argh::from_env();
    let mount_point = args.normalized_mount_point();
    let data_dir = get_data_dir();

    let cookies = if args.headless {
        authenticate(HeadlessCookieMonster, &data_dir)
    } else {
        authenticate(WebViewCookieMonster, &data_dir)
    }
    .map_err(|err| anyhow!("failed to authenticate {err}"))?;

    if !args.monitor {
        daemonize(&data_dir);
    }

    let client = BbApiClient::new(cookies, args.all);
    let fs = Bbfs::new(client).map_err(|_| anyhow!("failed to initialize Blackboard fs driver"))?;
    fs.mount(&mount_point)?;

    Ok(())
}

fn get_data_dir() -> PathBuf {
    let strategy = etcetera::choose_base_strategy().unwrap();
    let mut data_dir = strategy.data_dir();
    data_dir.push("blackboardfs");
    std::fs::create_dir_all(&data_dir).unwrap();
    data_dir
}

#[cfg(unix)]
fn daemonize(data_dir: &Path) {
    use daemonize_me::Daemon;
    let stdout =
        File::create(data_dir.join("stdout.log")).expect("failed to create stdout log file");
    let stderr =
        File::create(data_dir.join("stderr.log")).expect("failed to create stderr log file");
    Daemon::new().stdout(stdout).stderr(stderr).start().unwrap();
}

#[cfg(not(unix))]
fn daemonize(data_dir: &Path) {
    // TODO
}

fn authenticate<Monster: CookieMonster>(
    cookie_monster: Monster,
    data_dir: &Path,
) -> anyhow::Result<String> {
    // Check for cached cookie
    let cookie_cache_file = data_dir.join("cookie");
    match std::fs::read_to_string(&cookie_cache_file) {
        Ok(cookie) if is_cookie_valid(&cookie)? => return Ok(cookie),
        _ => {}
    }

    let cookie = cookie_monster.authenticate(data_dir)?;

    // Attempt to cache the cookie and warn if that fails
    File::create(&cookie_cache_file)
        .and_then(|mut file| file.write_all(cookie.as_bytes()))
        .map_err(|_| eprintln!("failed to cache cookie"))
        .ok();

    Ok(cookie)
}
