use fuser::{
    FileAttr, FileType, Filesystem, ReplyAttr, ReplyData, ReplyDirectory, ReplyEntry, Request,
};
use libc::ENOENT;
use std::ffi::OsStr;
use std::time::{Duration, UNIX_EPOCH};

// TODO: Figure out the best TTL (if any)
const TTL: Duration = Duration::from_secs(1);

const BLOCK_SIZE: u32 = 512;

fn attr(inode: u64, nlink: u32, kind: FileType, size: u64, perm: u16) -> FileAttr {
    FileAttr {
        ino: inode,
        size,
        blocks: (size + (BLOCK_SIZE as u64) - 1) / (BLOCK_SIZE as u64),
        atime: UNIX_EPOCH,
        mtime: UNIX_EPOCH,
        ctime: UNIX_EPOCH,
        crtime: UNIX_EPOCH,
        kind,
        perm,
        nlink,
        uid: 501,
        gid: 20,
        rdev: 0,
        flags: 0,
        blksize: BLOCK_SIZE,
    }
}

fn dirattr(inode: u64) -> FileAttr {
    // TODO: Correctly calculate nlink for directory
    attr(inode, 2, FileType::Directory, 0, 0o755)
}

fn fileattr(inode: u64, size: u64) -> FileAttr {
    // We don't handle symlinks so nlink can just be hardcoded to 1 for files
    attr(inode, 1, FileType::RegularFile, size, 0o644)
}

struct Inode {
    inode: u64,
    filename: String,
    contents: String,
}

pub struct BBFS {
    files: Vec<Inode>,
}

impl BBFS {
    pub fn new() -> BBFS {
        BBFS {
            files: vec![
                Inode {
                    inode: 2,
                    filename: "hello.txt".into(),
                    contents: "Hello, world!".into(),
                },
                Inode {
                    inode: 3,
                    filename: "foobar.txt".into(),
                    contents: "foo bar baz".into(),
                },
                Inode {
                    inode: 4,
                    filename: "joke.txt".into(),
                    contents: "Why did the chicken cro... (upgrade to premium)".into(),
                },
            ],
        }
    }

    fn file(&self, inode: u64) -> Option<&Inode> {
        self.files.iter().find(|x| x.inode == inode)
    }
}

impl Filesystem for BBFS {
    fn lookup(&mut self, _req: &Request, parent: u64, name: &OsStr, reply: ReplyEntry) {
        println!("lookup");
        if parent == 1 {
            let name = name.to_str().unwrap();
            for file in self.files.iter() {
                if file.filename == name {
                    reply.entry(&TTL, &fileattr(file.inode, file.contents.len() as u64), 0);
                    return;
                }
            }
            reply.error(ENOENT);
        } else {
            reply.error(ENOENT);
        }
    }

    fn getattr(&mut self, _req: &Request, ino: u64, reply: ReplyAttr) {
        println!("getattr(ino={ino})");
        match ino {
            1 => reply.attr(&TTL, &dirattr(1)),
            _ => {
                if let Some(file) = self.file(ino) {
                    reply.attr(&TTL, &fileattr(file.inode, file.contents.len() as u64));
                } else {
                    reply.error(ENOENT);
                }
            }
        }
    }

    fn read(
        &mut self,
        _req: &Request,
        ino: u64,
        _fh: u64,
        offset: i64,
        _size: u32,
        _flags: i32,
        _lock: Option<u64>,
        reply: ReplyData,
    ) {
        println!("read(ino={ino}, offset={offset})");

        if let Some(file) = self.file(ino) {
            reply.data(&file.contents.as_bytes()[offset as usize..]);
        } else {
            reply.error(ENOENT);
        }
    }

    fn readdir(
        &mut self,
        _req: &Request,
        ino: u64,
        _fh: u64,
        offset: i64,
        mut reply: ReplyDirectory,
    ) {
        println!("readdir(ino={ino}, offset={offset})");

        if ino != 1 {
            reply.error(ENOENT);
            return;
        }

        let mut entries = vec![
            (1, FileType::Directory, ".".into()),
            (1, FileType::Directory, "..".into()),
        ];

        for file in self.files.iter() {
            entries.push((file.inode, FileType::RegularFile, file.filename.clone()))
        }

        for (i, entry) in entries.into_iter().enumerate().skip(offset as usize) {
            // i + 1 means the index of the next entry
            if reply.add(entry.0, (i + 1) as i64, entry.1, entry.2) {
                break;
            }
        }
        reply.ok();
    }
}
