use std::{
    path::PathBuf,
    sync::mpsc,
    time::{Duration, Instant},
};

use wry::{
    application::{
        dpi::LogicalSize,
        event::{Event, StartCause, WindowEvent},
        event_loop::{ControlFlow, EventLoopBuilder, EventLoopProxy},
        platform::run_return::EventLoopExtRunReturn,
        window::WindowBuilder,
    },
    webview::{WebContext, WebView, WebViewBuilder},
};

#[derive(Debug)]
enum UserEvent {
    Navigation(String),
    GotCookie(String),
}

pub fn eat_user_cookies(context_data_dir: PathBuf) -> anyhow::Result<String> {
    let mut event_loop = EventLoopBuilder::<UserEvent>::with_user_event().build();
    let proxy = event_loop.create_proxy();
    let cookie_proxy = event_loop.create_proxy();
    let window = WindowBuilder::new()
        .with_title("Blackboard Authentication")
        .with_inner_size(LogicalSize::new(400, 600))
        .build(&event_loop)?;

    let mut context = WebContext::new(Some(context_data_dir));
    let mut webview = Some(
        WebViewBuilder::new(window)?
            .with_web_context(&mut context)
            .with_url("https://learn.uq.edu.au/")?
            .with_navigation_handler(move |uri: String| {
                proxy
                    .send_event(UserEvent::Navigation(uri.clone()))
                    .expect("event loop should be open");
                true
            })
            .build()?,
    );

    let (cookie_send, cookie_recv) = mpsc::channel();
    let mut finish_time = None;

    event_loop.run_return(move |event, _, control_flow| {
        *control_flow = match finish_time {
            Some(time) if Instant::now() > time => ControlFlow::Exit,
            Some(_) => ControlFlow::Poll,
            None => ControlFlow::Wait,
        };

        match event {
            Event::NewEvents(StartCause::Init) => println!("Wry has started!"),
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
                println!("Window destroyed");
            }
            Event::UserEvent(UserEvent::Navigation(url)) => {
                println!("{url}");
                if url == "https://learn.uq.edu.au/ultra" {
                    extract_cookies_from_webview(
                        webview
                            .as_ref()
                            .expect("WebView should still be alive if we're navigating in it"),
                        cookie_proxy.clone(),
                    );
                }
            }
            Event::UserEvent(UserEvent::GotCookie(got_cookie)) => {
                println!("{got_cookie}");
                cookie_send
                    .send(got_cookie)
                    .expect("channel should not be closed");
                drop(
                    webview
                        .take()
                        .expect("WebView should only be dropped once?"),
                );
            }
            _ => (),
        }
    });

    println!("Escaped");
    cookie_recv.try_recv().map_err(Into::into)
}

#[cfg(target_os = "linux")]
fn extract_cookies_from_webview(webview: &WebView, cookie_proxy: EventLoopProxy<UserEvent>) {
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
                        .map(|mut cookie| cookie.to_cookie_header().unwrap().as_str().to_owned())
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
async fn extract_cookies_from_webview(webview: &WebView, cookie_proxy: EventLoopProxy<UserEvent>) {
    use wry::webview::WebviewExtMacOS;

    let wk_webview = webview.webview();

    // Some Obj-C magic to get wk_webview.configuration.websiteDataStore.httpCookieStore, run
    // getAllCookies: on that and filter for the NSHTTPCookies that have the relavant domain
    todo!()
}
