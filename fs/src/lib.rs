use fuser::{
    FileAttr, FileType, Filesystem, ReplyAttr, ReplyData, ReplyDirectory, ReplyEntry, Request,
};
use lib_bb::Course;
use libc::ENOENT;
use std::collections::HashMap;
use std::ffi::OsStr;
use std::time::{Duration, UNIX_EPOCH};

use lib_bb::client::{BBClient, BBMockClient};

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
    client: BBMockClient,
    next_free_inode: u64,
    courses: HashMap<u64, Course>,
}

impl BBFS {
    pub fn new() -> BBFS {
        let mut bbfs = BBFS {
            client: BBMockClient,
            next_free_inode: 2,
            courses: HashMap::new(),
        };
        bbfs.populate_courses();
        bbfs
    }

    fn get_free_inode(&mut self) -> u64 {
        let inode = self.next_free_inode;
        self.next_free_inode += 1;
        inode
    }

    fn populate_courses(&mut self) {
        for course in self.client.get_courses().into_iter() {
            let inode = self.get_free_inode();
            self.courses.insert(inode, course);
        }
    }

    fn course(&self, inode: u64) -> Option<&Course> {
        self.courses.get(&inode)
    }

    fn course_dir_name(course: &Course) -> String {
        course.short_name.clone()
    }
}

impl Filesystem for BBFS {
    fn lookup(&mut self, _req: &Request, parent: u64, name: &OsStr, reply: ReplyEntry) {
        println!("lookup");
        if parent == 1 {
            let name = name.to_str().unwrap();
            for (inode, course) in self.courses.iter() {
                if name == BBFS::course_dir_name(course) {
                    reply.entry(&TTL, &dirattr(*inode), 0);
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
                if let Some(_) = self.course(ino) {
                    reply.attr(&TTL, &dirattr(ino));
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

        // reply.data(&file.contents.as_bytes()[offset as usize..]);

        reply.error(ENOENT);
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

        for (inode, course) in self.courses.iter() {
            entries.push((*inode, FileType::RegularFile, BBFS::course_dir_name(course)))
        }

        for (i, entry) in entries.into_iter().enumerate().skip(offset as usize) {
            if reply.add(entry.0, (i + 1) as i64, entry.1, entry.2) {
                break;
            }
        }
        reply.ok();
    }
}
