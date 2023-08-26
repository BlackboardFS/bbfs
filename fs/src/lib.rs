use fuser::{
    FileAttr, FileType, Filesystem, ReplyAttr, ReplyData, ReplyDirectory, ReplyEntry, Request,
};
use lib_bb::{Course, CourseItem};
use libc::ENOENT;
use std::collections::HashMap;
use std::ffi::OsStr;
use std::time::{Duration, UNIX_EPOCH};

use lib_bb::client::BBClient;

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

struct CourseInode {
    course: Course,
    items: Option<HashMap<u64, CourseItem>>,
}

pub struct BBFS<Client: BBClient> {
    client: Client,
    next_free_inode: u64,
    courses: HashMap<u64, CourseInode>,
}

impl<Client: BBClient> BBFS<Client> {
    pub fn new(client: Client) -> BBFS<Client> {
        let mut bbfs = BBFS {
            client,
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
            self.courses.insert(
                inode,
                CourseInode {
                    course,
                    items: None,
                },
            );
        }
    }

    fn course(&self, inode: u64) -> Option<&CourseInode> {
        self.courses.get(&inode)
    }

    fn course_items(&mut self, course_inode: u64) -> Option<HashMap<u64, CourseItem>> {
        let course = match self.course(course_inode) {
            Some(course) => course,
            None => return None,
        };

        if let Some(items) = course.items.clone() {
            Some(items)
        } else {
            let mut items = HashMap::new();
            for item in self.client.get_course_contents(&course.course).into_iter() {
                items.insert(self.get_free_inode(), item);
            }
            self.courses.get_mut(&course_inode).unwrap().items = Some(items.clone());
            Some(items)
        }
    }

    fn course_item(&self, inode: u64) -> Option<&CourseItem> {
        for course in self.courses.values() {
            let items = match &course.items {
                Some(items) => items,
                None => continue,
            };
            for (item_inode, item) in items.iter() {
                if inode == *item_inode {
                    return Some(item);
                }
            }
        }
        None
    }

    fn course_dir_name(course: &Course) -> String {
        course.short_name.clone()
    }
}

impl<Client: BBClient> Filesystem for BBFS<Client> {
    fn lookup(&mut self, _req: &Request, parent: u64, name: &OsStr, reply: ReplyEntry) {
        println!("lookup");
        let name = name.to_str().unwrap();
        if parent == 1 {
            for (inode, course) in self.courses.iter() {
                if name == Self::course_dir_name(&course.course) {
                    reply.entry(&TTL, &dirattr(*inode), 0);
                    return;
                }
            }
            reply.error(ENOENT);
        } else if let Some(items) = self.course_items(parent) {
            for (inode, item) in items.iter() {
                if name == item.name {
                    reply.entry(
                        &TTL,
                        &fileattr(*inode, self.client.get_item_size(item) as u64),
                        0,
                    );
                    return;
                }
            }
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
                } else if let Some(item) = self.course_item(ino) {
                    reply.attr(&TTL, &fileattr(ino, self.client.get_item_size(item) as u64))
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

        if let Some(item) = self.course_item(ino) {
            // TODO: Cache item contents
            reply.data(&self.client.get_item_contents(item)[offset as usize..])
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

        let mut entries = vec![];
        entries.push((ino, FileType::Directory, ".".into()));
        if ino == 1 {
            entries.push((1, FileType::Directory, "..".into()));

            for (inode, course) in self.courses.iter() {
                entries.push((
                    *inode,
                    FileType::RegularFile,
                    Self::course_dir_name(&course.course),
                ))
            }
        } else if let Some(items) = self.course_items(ino) {
            entries.push((1, FileType::Directory, "..".into()));

            for (inode, item) in items {
                entries.push((inode, FileType::RegularFile, item.name));
            }
        } else {
            reply.error(ENOENT);
            return;
        }

        for (i, entry) in entries.into_iter().enumerate().skip(offset as usize) {
            if reply.add(entry.0, (i + 1) as i64, entry.1, entry.2) {
                break;
            }
        }
        reply.ok();
    }
}
