use std::sync::{Arc, Mutex};

use gtk::prelude::*;
use gtk::{Inhibit, Window, WindowType};
use webkit2gtk::traits::{CookieManagerExt, SettingsExt, WebContextExt, WebViewExt};
use webkit2gtk::{LoadEvent, WebContext, WebView};

pub fn eat_user_cookies() -> Vec<String> {
    gtk::init().unwrap();
    let window = Window::new(WindowType::Toplevel);
    let context = WebContext::default().unwrap();

    let webview = WebView::with_context(&context);
    webview.load_uri("https://learn.uq.edu.au/");
    window.add(&webview);

    let settings = WebViewExt::settings(&webview).unwrap();
    settings.set_enable_developer_extras(true);

    let cookies = Arc::new(Mutex::new(Vec::new()));
    let cookies_send = cookies.clone();

    webview.connect_load_changed(move |webview, load_event| {
        if webview
            .uri()
            .is_some_and(|uri| uri.as_str().starts_with("https://learn.uq.edu.au"))
            && matches!(load_event, LoadEvent::Committed)
        {
            // We're loading the final blackboard page, so we ave successfully authenticated and
            // no longer need our window
            webview.parent_window().unwrap().hide();

            let cookies_send_internal = cookies_send.clone();

            let cookie_manager = webview.web_context().unwrap().cookie_manager().unwrap();
            cookie_manager.cookies(
                "https://learn.uq.edu.au/",
                None::<&gio::Cancellable>,
                move |cookies| {
                    let mut stored_cookies = cookies_send_internal.lock().unwrap();
                    for mut cookie in cookies.unwrap() {
                        stored_cookies.push(cookie.to_cookie_header().unwrap().as_str().to_owned());
                    }
                    gtk::main_quit();
                },
            )
        }
    });

    window.show_all();
    window.connect_delete_event(|_, _| {
        gtk::main_quit();
        Inhibit(false)
    });
    gtk::main();

    let cookies = cookies.lock().unwrap();
    cookies.as_slice().to_owned()
}
