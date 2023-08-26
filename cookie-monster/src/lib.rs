use std::{
    path::PathBuf,
    sync::{Arc, Mutex},
};

use anyhow::anyhow;

use wry::{
    application::{
        dpi::LogicalSize,
        event::{Event, StartCause, WindowEvent},
        event_loop::{ControlFlow, EventLoopBuilder},
        platform::run_return::EventLoopExtRunReturn,
        window::WindowBuilder,
    },
    webview::{WebContext, WebViewBuilder},
};

#[derive(Debug)]
enum UserEvent {
    Navigation(String),
    GotCookies(String),
}

pub fn eat_user_cookies() -> anyhow::Result<Vec<String>> {
    let mut event_loop = EventLoopBuilder::<UserEvent>::with_user_event().build();
    let proxy = event_loop.create_proxy();
    let window = WindowBuilder::new()
        .with_title("Blackboard Authentication")
        .with_inner_size(LogicalSize::new(400, 600))
        .build(&event_loop)?;

    let mut context = WebContext::new(Some(PathBuf::from("./data")));
    let webview = WebViewBuilder::new(window)?
        .with_web_context(&mut context)
        .with_url("https://learn.uq.edu.au/")?
        .with_navigation_handler(move |uri: String| {
            let submitted = proxy.send_event(UserEvent::Navigation(uri.clone())).is_ok();
            submitted
        })
        .build()?;

    event_loop.run_return(move |event, _, control_flow| {
        *control_flow = ControlFlow::Wait;

        match event {
            Event::NewEvents(StartCause::Init) => println!("Wry has started!"),
            Event::WindowEvent {
                event: WindowEvent::CloseRequested,
                ..
            } => *control_flow = ControlFlow::Exit,
            Event::UserEvent(UserEvent::Navigation(url)) => {
                println!("{url}");
                if url == "https://learn.uq.edu.au/ultra" {
                    // let cookie_proxy = cookie_proxy.clone();
                    // println!("Running script");
                    webview
                        .evaluate_script_with_callback("document.cookie", move |cookies| {
                            println!("{cookies}");
                            // println!("Script callback");
                            // cookie_proxy.send_event(UserEvent::GotCookies(cookies))
                        })
                        .unwrap();
                }
            }
            Event::UserEvent(UserEvent::GotCookies(cookies)) => {
                println!("{cookies}");
                *control_flow = ControlFlow::Exit;
            }
            _ => (),
        }
    });

    Err(anyhow!("nah"))

    // gtk::init().unwrap();
    // let window = Window::new(WindowType::Toplevel);
    // let context = WebContext::default().unwrap();

    // let webview = WebView::with_context(&context);
    // webview.load_uri("https://learn.uq.edu.au/");
    // window.add(&webview);

    // let settings = WebViewExt::settings(&webview).unwrap();
    // settings.set_enable_developer_extras(true);

    // let cookies = Arc::new(Mutex::new(Vec::new()));
    // let cookies_send = cookies.clone();

    // webview.connect_load_changed(move |webview, load_event| {
    //     if webview
    //         .uri()
    //         .is_some_and(|uri| uri.as_str().starts_with("https://learn.uq.edu.au"))
    //         && matches!(load_event, LoadEvent::Committed)
    //     {
    //         // We're loading the final blackboard page, so we ave successfully authenticated and
    //         // no longer need our window
    //         webview.parent_window().unwrap().hide();

    //         let cookies_send_internal = cookies_send.clone();

    //         let cookie_manager = webview.web_context().unwrap().cookie_manager().unwrap();
    //         cookie_manager.cookies(
    //             "https://learn.uq.edu.au/",
    //             None::<&gio::Cancellable>,
    //             move |cookies| {
    //                 let mut stored_cookies = cookies_send_internal.lock().unwrap();
    //                 for mut cookie in cookies.unwrap() {
    //                     stored_cookies.push(cookie.to_cookie_header().unwrap().as_str().to_owned());
    //                 }
    //                 gtk::main_quit();
    //             },
    //         )
    //     }
    // });

    // window.show_all();
    // window.connect_delete_event(|_, _| {
    //     gtk::main_quit();
    //     Inhibit(false)
    // });
    // gtk::main();

    // let cookies = cookies.lock().unwrap();
    // cookies.as_slice().to_owned()
}
