use fs::BBFS;
use fuser::MountOption;
use std::path::Path;

// `umount ./tmp; cargo run` (otherwise you'll get an error most of the time)

fn main() {
    let path = "./tmp";
    let mountpoint = Path::new(path);
    fuser::mount2(BBFS, &mountpoint, &[MountOption::AutoUnmount]).unwrap();
}
