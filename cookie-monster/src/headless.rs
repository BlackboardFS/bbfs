use crate::CookieMonster;
use anyhow::anyhow;
use etcetera::{choose_base_strategy, BaseStrategy};
use fantoccini::{elements::Element, Client, ClientBuilder, Locator};
use futures::{future::FutureExt, pin_mut, select};
use rpassword::read_password;
use std::io::{prelude::*, stdin, stdout};
use std::path::Path;
use std::process::Command;
use url::Url;

pub struct HeadlessCookieMonster;

impl HeadlessCookieMonster {
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

    async fn complete_auth<DuoF: Fn(&str)>(
        client: &Client,
        handle_duo_code: DuoF,
    ) -> anyhow::Result<()> {
        let duo_task = Self::wait_for_duo_code(client).fuse();
        let completion_task = Self::wait_for_completion(client).fuse();
        let failure_task = Self::wait_for_error_alert(client).fuse();

        pin_mut!(duo_task, completion_task, failure_task);

        select! {
            duo_code_element = duo_task => {
                let duo_code = duo_code_element?
                    .text()
                    .await?;

                handle_duo_code(&duo_code);
                Self::wait_for_completion(client).await
            }
            _ = completion_task => Ok(()),
            _ = failure_task => {
                Err(anyhow!("Incorrect username or password"))
            }
        }
    }

    fn eat_user_cookies<DuoF: Fn(&str)>(
        username: &str,
        password: &str,
        handle_duo_code: DuoF,
    ) -> anyhow::Result<String> {
        // Ensure that webdriver is installed
        let strategy = choose_base_strategy().unwrap();
        let data_dir = {
            let mut data_dir = strategy.data_dir();
            data_dir.push("blackboardfs");
            std::fs::create_dir_all(&data_dir).expect("failed to create data dir");
            data_dir
        };

        webdriver_install::Driver::Gecko
            .install_into(data_dir.clone())
            .map_err(|err| anyhow!(format!("failed to install gecko webdriver: {:?}", err)))?;

        let mut driver_path = data_dir;
        driver_path.push("geckodriver");

        // Run the webdriver
        let mut driver = Command::new(driver_path)
            .stdout(std::process::Stdio::piped())
            .stderr(std::process::Stdio::piped())
            .spawn()
            .expect("failed to run geckodriver");

        let driver_stdout = driver.stdout.as_mut().unwrap();
        let reader = std::io::BufReader::new(driver_stdout);
        reader
            .lines()
            .next()
            .expect("expected first line of webdriver output")
            .expect("expected first line of webdriver output");

        let result = tokio::runtime::Builder::new_multi_thread()
            .enable_all()
            .build()
            .unwrap()
            .block_on(async {
                let mut firefox_options = serde_json::Map::<String, serde_json::Value>::new();
                firefox_options.insert("args".into(), vec!["-headless"].into());
                let mut capabilities = serde_json::Map::<String, serde_json::Value>::new();
                capabilities.insert(
                    "moz:firefoxOptions".into(),
                    serde_json::Value::Object(firefox_options),
                );

                let c = ClientBuilder::native()
                    .capabilities(capabilities)
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

                match Self::complete_auth(&c, handle_duo_code).await {
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
            });

        driver.kill().expect("failed to kill webdriver");

        result
    }
}

impl CookieMonster for HeadlessCookieMonster {
    fn authenticate(&self, _data_dir: &Path) -> anyhow::Result<String> {
        print!("Username: ");
        let _ = stdout().flush();
        let mut username = "".into();
        stdin().read_line(&mut username).expect("expected username");

        print!("Password: ");
        let _ = stdout().flush();
        let password = read_password().expect("expected password");

        Self::eat_user_cookies(&username, &password, |duo_code| {
            println!("Your duo code is {duo_code}")
        })
    }
}
