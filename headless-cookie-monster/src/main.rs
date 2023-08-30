use headless_cookie_monster::eat_user_cookies;

use std::io::{stdin, stdout, Write};

fn main() {
    print!("Username: ");
    let _ = stdout().flush();
    let mut username = "".into();
    stdin().read_line(&mut username).expect("expected username");

    print!("Password: ");
    let _ = stdout().flush();
    let mut password = "".into();
    stdin().read_line(&mut password).expect("expected password");

    match eat_user_cookies(&username, &password) {
        Ok(cookies) => println!("{}", cookies),
        Err(err) => {
            eprintln!("{:?}", err);
        }
    }
}
