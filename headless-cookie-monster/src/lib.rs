use anyhow::anyhow;
use fantoccini::{elements::Element, Client, ClientBuilder, Locator};
use futures::{future::FutureExt, pin_mut, select};
use url::Url;

async fn wait_for_completion(client: &Client) -> anyhow::Result<()> {
    Ok(client
        .wait()
        .forever()
        .for_url(Url::parse("https://learn.uq.edu.au/ultra")?)
        .await?)
}

async fn wait_for_duo_code(client: &Client) -> anyhow::Result<Element> {
    Ok(client
        .wait()
        .forever()
        .for_element(Locator::Css(".verification-code"))
        .await?)
}

async fn wait_for_error_alert(client: &Client) -> anyhow::Result<Element> {
    Ok(client
        .wait()
        .forever()
        .for_element(Locator::Css(".sign-on__form-error"))
        .await?)
}

async fn complete_auth(client: &Client) -> anyhow::Result<()> {
    let duo_task = wait_for_duo_code(client).fuse();
    let completion_task = wait_for_completion(client).fuse();
    let failure_task = wait_for_error_alert(client).fuse();

    pin_mut!(duo_task, completion_task, failure_task);

    select! {
        duo_code_element = duo_task => {
            let duo_code = duo_code_element?
                .text()
                .await?;

            println!("Your duo code is {}", duo_code);
            wait_for_completion(client).await
        }
        _ = completion_task => Ok(()),
        _ = failure_task => {
            return Err(anyhow!("Incorrect username or password"));
        }
    }
}

pub fn eat_user_cookies(username: &str, password: &str) -> anyhow::Result<String> {
    tokio::runtime::Builder::new_multi_thread()
        .enable_all()
        .build()
        .unwrap()
        .block_on(async {
            let c = ClientBuilder::native()
                .connect("http://localhost:4444")
                .await
                .expect("failed to connect to WebDriver");

            // first, go to the Wikipedia page for Foobar
            c.goto("https://learn.uq.edu.au").await?;

            let username_field = c
                .wait()
                .forever()
                .for_element(Locator::Id("username"))
                .await?;
            let password_field = c
                .wait()
                .forever()
                .for_element(Locator::Id("password"))
                .await?;
            let submit_button = c
                .wait()
                .forever()
                .for_element(Locator::Css("#loginuserpass .button"))
                .await?;

            username_field.send_keys(username).await?;
            password_field.send_keys(password).await?;
            submit_button.click().await?;

            match complete_auth(&c).await {
                Ok(()) => {}
                Err(err) => {
                    c.close().await?;
                    return Err(err);
                }
            }

            let megacookie = c
                .get_all_cookies()
                .await?
                .into_iter()
                .map(|cookie| format!("{}={}", cookie.name(), cookie.value()))
                .reduce(|mut megacookie, cookie| {
                    megacookie.push(';');
                    megacookie.push_str(&cookie);
                    megacookie
                })
                .unwrap_or_default();

            c.close().await?;

            Ok(megacookie)
        })
}
