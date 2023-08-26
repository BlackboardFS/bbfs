use dotenv::dotenv;
use fs::BBFS;
use fuser::MountOption;
use lib_bb::client::BBAPIClient;
use std::env;
use std::path::Path;

// `umount ./tmp; cargo run` (otherwise you'll get an error most of the time)

fn main() {
    let path = "./tmp";
    let mountpoint = Path::new(path);
    dotenv().ok();
    let cookies = env::var("BBCOOKIE").unwrap();
    let client = BBAPIClient::new(cookies);
    fuser::mount2(BBFS::new(client), &mountpoint, &[MountOption::AutoUnmount]).unwrap();
}
