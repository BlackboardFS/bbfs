#[cfg(target_os = "macos")]
#[macro_use]
extern crate objc;

use std::path::Path;

use anyhow::anyhow;
use wry::webview::Url;

pub mod headless;
pub mod webview;

pub use headless::HeadlessCookieMonster;
pub use webview::WebViewCookieMonster;

// TODO: If bbfs-scrape starts depending directly on cookie-monster, it should use this constant
//   instead of its own.
const BB_BASE_URL: &str = "https://learn.uq.edu.au";

pub fn is_cookie_valid(cookie: &str) -> anyhow::Result<bool> {
    ureq::AgentBuilder::new()
        .redirects(32)
        .build()
        .request_url("GET", &Url::parse(BB_BASE_URL).unwrap())
        .set("cookie", cookie)
        .call()
        .map(|response| response.get_url().starts_with(&format!("{BB_BASE_URL}/")))
        .map_err(|err| anyhow!("failed to check cookie validity: {}", err))
}

pub trait CookieMonster {
    fn authenticate(&self, data_dir: &Path) -> anyhow::Result<String>;
}
