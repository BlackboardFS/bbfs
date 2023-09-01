use std::{
    fs::File,
    io::Write,
    path::Path,
    time::{Duration, Instant},
};

use anyhow::anyhow;

use wry::{
    application::{
        dpi::LogicalSize,
        event::{Event, WindowEvent},
        event_loop::{ControlFlow, EventLoopBuilder, EventLoopProxy},
        platform::run_return::EventLoopExtRunReturn,
        window::WindowBuilder,
    },
    webview::{WebView, WebViewBuilder},
};

use crate::CookieMonster;

#[derive(Debug)]
enum UserEvent {
    PageLoad(String),
    Navigation(String),
    #[allow(dead_code)]
    GotCookie(String),
}

pub struct WebViewCookieMonster;

impl WebViewCookieMonster {
    #[cfg(target_os = "linux")]
    fn extract_cookies_from_webview(
        webview: &WebView,
        cookie_proxy: EventLoopProxy<UserEvent>,
        _cookie_file: &Path,
    ) {
        use webkit2gtk::{CookieManagerExt, WebContextExt, WebViewExt};
        use wry::webview::WebviewExtUnix;

        let gtk_webview = webview.webview();

        let cookie_manager = gtk_webview.web_context().unwrap().cookie_manager().unwrap();
        cookie_manager.cookies(
            "https://learn.uq.edu.au/",
            None::<&gio::Cancellable>,
            move |cookies| {
                cookie_proxy
                    .send_event(UserEvent::GotCookie(
                        cookies
                            .unwrap()
                            .into_iter()
                            .map(|mut cookie| {
                                cookie.to_cookie_header().unwrap().as_str().to_owned()
                            })
                            .reduce(|mut megacookie, cookie| {
                                megacookie.push(';');
                                megacookie.push_str(&cookie);
                                megacookie
                            })
                            .unwrap_or_default(),
                    ))
                    .expect("event loop should be open");
            },
        );
    }

    #[cfg(target_os = "macos")]
    fn extract_cookies_from_webview(
        webview: &WebView,
        _cookie_proxy: EventLoopProxy<UserEvent>,
        cookie_file: &Path,
    ) {
        use block::ConcreteBlock;
        use objc::runtime::Object;
        use std::os::raw::c_char;
        use std::str;

        unsafe fn object_to_string(object: *mut Object) -> String {
            let bytes: *const c_char = msg_send![object, UTF8String];
            let bytes = bytes as *const u8;

            let len = msg_send![object, lengthOfBytesUsingEncoding:4];

            let mut aligned_bytes = vec![];
            for i in 0..len {
                aligned_bytes.push(bytes.offset(i).read_unaligned());
            }

            str::from_utf8(&aligned_bytes).unwrap().into()
        }

        unsafe {
            let website_data_store: *mut Object =
                msg_send![class!(WKWebsiteDataStore), defaultDataStore];
            // TODO: Undo ugly fn_once stuff, not necessary anymore
            let block = ConcreteBlock::new(move |cookies: *mut Object| {
                let count: usize = msg_send![cookies, count];
                let mut cookie_pairs: Vec<(String, String)> = vec![];
                for i in 0..count {
                    let cookie: *mut Object = msg_send![cookies, objectAtIndex:i];
                    let key: *mut Object = msg_send![cookie, name];
                    let value: *mut Object = msg_send![cookie, value];
                    let domain: *mut Object = msg_send![cookie, domain];
                    let path: *mut Object = msg_send![cookie, path];
                    let key = object_to_string(key);
                    let value = object_to_string(value);
                    let domain = object_to_string(domain);
                    let path = object_to_string(path);
                    if path == "/" && domain == "learn.uq.edu.au" {
                        cookie_pairs.push((key, value))
                    }
                }

                let megacookie = cookie_pairs
                    .iter()
                    .map(|(key, value)| format!("{}={}", key, value))
                    .collect::<Vec<String>>()
                    .join("; ");

                let res = File::create(cookie_file);
                if let Ok(mut file) = res {
                    if file.write_all(megacookie.as_bytes()).is_err() {
                        eprintln!("Failed to write cookie");
                    }
                } else {
                    eprintln!("Failed to create file: {:?}", res.err().unwrap());
                }

                webview.load_url("https://macos-done");
            });
            let http_cookie_store: *mut Object = msg_send![website_data_store, httpCookieStore];
            let _: () = msg_send![http_cookie_store, getAllCookies:block];
        }
    }
}

impl CookieMonster for WebViewCookieMonster {
    fn authenticate(&self, data_dir: &Path) -> anyhow::Result<String> {
        let cookie_file_buf = data_dir.join("tmp_cookie");
        let cookie_file = cookie_file_buf.as_path();

        let mut event_loop = EventLoopBuilder::<UserEvent>::with_user_event().build();
        let proxy = event_loop.create_proxy();
        let load_proxy = event_loop.create_proxy();
        let cookie_proxy = event_loop.create_proxy();
        let window = WindowBuilder::new()
            .with_title("Blackboard Authentication")
            .with_inner_size(LogicalSize::new(400, 600))
            .build(&event_loop)?;

        let mut webview = Some(
            WebViewBuilder::new(window)?
                .with_url("https://learn.uq.edu.au/")?
                .with_navigation_handler(move |uri: String| {
                    proxy
                        .send_event(UserEvent::Navigation(uri.clone()))
                        .expect("event loop should be open");
                    true
                })
                .with_on_page_load_handler(move |event, uri| {
                    if matches!(event, wry::webview::PageLoadEvent::Finished) {
                        load_proxy
                            .send_event(UserEvent::PageLoad(uri.clone()))
                            .expect("event loop should be open");
                    }
                })
                .build()?,
        );

        let mut finish_time = None;

        event_loop.run_return(move |event, _, control_flow| {
            *control_flow = match finish_time {
                Some(time) if Instant::now() > time => ControlFlow::Exit,
                Some(_) => ControlFlow::Poll,
                None => ControlFlow::Wait,
            };

            match event {
                Event::WindowEvent {
                    event: WindowEvent::CloseRequested,
                    ..
                } => {
                    drop(
                        webview
                            .take()
                            .expect("WebView should only be dropped once?"),
                    );
                }
                Event::WindowEvent {
                    event: WindowEvent::Destroyed,
                    ..
                } => {
                    finish_time = Some(Instant::now() + Duration::from_secs(2));
                }
                Event::UserEvent(UserEvent::PageLoad(url)) => {
                    if url.starts_with("https://auth.uq.edu.au/idp/module.php/core/loginuserpass.php?AuthState") {
                        webview.as_ref()
                            .expect("WebView should still be alive if we're navigating in it")
                            .evaluate_script(r#"if (document.getElementsByClassName("sign-on__form-error").length == 0) { document.getElementsByClassName("sign-on__content")[0].children[0].innerHTML = "<span style=\"background-color: red; color: white; width: 100%; font-weight: bold; padding: 10px; display: block; text-align: center\">We just injected custom JavaScript into this web browser. We could steal your credentials. Make sure you have read and understand our code.</span>" }"#)
                            .unwrap();
                    }
                }
                Event::UserEvent(UserEvent::Navigation(url)) => {
                    if url == "https://learn.uq.edu.au/ultra" {
                        finish_time = Some(Instant::now() + Duration::from_secs(2));
                        Self::extract_cookies_from_webview(
                            webview
                                .as_ref()
                                .expect("WebView should still be alive if we're navigating in it"),
                            cookie_proxy.clone(),
                            cookie_file,
                        );
                    } else if url == "https://macos-done/" {
                        drop(webview.take().expect("WebView should only be dropped once"))
                    }
                }
                Event::UserEvent(UserEvent::GotCookie(cookie)) => {
                    // Only Linux gets here
                    if let Ok(mut file) = File::create(cookie_file) {
                        if file.write_all(cookie.as_bytes()).is_err() {
                            eprintln!("failed to write cookie");
                        }
                    }
                    drop(webview.take().expect("WebView should only be dropped once"));
                }
                _ => (),
            }
        });

        std::fs::read_to_string(cookie_file).map_err(|_| anyhow!("failed to retrieve cookie"))
    }
}
