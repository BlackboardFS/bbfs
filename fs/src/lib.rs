use fuser::{
    FileAttr, FileType, Filesystem, ReplyAttr, ReplyData, ReplyDirectory, ReplyEntry, Request,
};
use lib_bb::{Course, CourseItem, CourseItemContent};
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

#[derive(Clone)]
struct CourseItemInode {
    item: CourseItem,
    children: Option<Vec<u64>>,
}

struct CourseInode {
    course: Course,
    items: Option<Vec<u64>>,
}

pub struct BBFS<Client: BBClient> {
    client: Client,
    next_free_inode: u64,
    courses: HashMap<u64, CourseInode>,
    items: HashMap<u64, CourseItemInode>,
}

impl<Client: BBClient> BBFS<Client> {
    pub fn new(client: Client) -> BBFS<Client> {
        let mut bbfs = BBFS {
            client,
            next_free_inode: 2,
            courses: HashMap::new(),
            items: HashMap::new(),
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

    fn course_items(&mut self, course_inode: u64) -> Option<Vec<(u64, CourseItem)>> {
        let course = match self.course(course_inode) {
            Some(course) => course,
            None => return None,
        };

        let inodes = if let Some(items) = &course.items {
            items.clone()
        } else {
            let mut inodes = Vec::new();
            for item in self.client.get_course_contents(&course.course).into_iter() {
                let inode = self.get_free_inode();
                self.items.insert(
                    inode,
                    CourseItemInode {
                        item,
                        children: None,
                    },
                );
                inodes.push(inode);
            }

            self.courses.get_mut(&course_inode).unwrap().items = Some(inodes.clone());

            inodes
        };

        Some(
            inodes
                .iter()
                .map(|inode| (*inode, self.items.get(inode).unwrap().item.clone()))
                .collect(),
        )
    }

    fn course_item(&self, inode: u64) -> Option<&CourseItem> {
        self.items.get(&inode).map(|item| &item.item)
    }

    fn course_item_children(&mut self, inode: u64) -> Option<Vec<(u64, CourseItem)>> {
        let item = match self.items.get(&inode) {
            Some(item) => item,
            None => return None,
        };

        // If the children have already been fetched return them
        let inodes = if let Some(children) = &item.children {
            children.clone()
        } else {
            let contents = match item.item.content.clone() {
                Some(CourseItemContent::FolderUrl(url)) => self.client.get_directory_contents(url),
                _ => return None,
            };

            let mut inodes = Vec::new();
            for child in contents {
                let inode = self.get_free_inode();
                inodes.push(inode);

                self.items.insert(
                    inode,
                    CourseItemInode {
                        item: child,
                        children: None,
                    },
                );
            }

            self.items.get_mut(&inode).unwrap().children = Some(inodes.clone());

            inodes
        };

        Some(
            inodes
                .iter()
                .map(|inode| (*inode, self.items.get(inode).unwrap().item.clone()))
                .collect(),
        )
    }

    fn course_dir_name(course: &Course) -> String {
        course.short_name.clone()
    }

    fn file_type(item: &CourseItem) -> FileType {
        match item.content {
            Some(CourseItemContent::FileUrl(_)) => FileType::RegularFile,
            Some(CourseItemContent::Link(_)) => FileType::RegularFile,
            Some(CourseItemContent::FolderUrl(_)) => FileType::Directory,
            None => FileType::RegularFile,
        }
    }
}

impl<Client: BBClient> Filesystem for BBFS<Client> {
    fn lookup(&mut self, _req: &Request, parent: u64, name: &OsStr, reply: ReplyEntry) {
        let name = name.to_str().unwrap();
        println!("lookup(name={name})");

        if parent == 1 {
            for (inode, course) in self.courses.iter() {
                if name == Self::course_dir_name(&course.course) {
                    reply.entry(&TTL, &dirattr(*inode), 0);
                    return;
                }
            }
            reply.error(ENOENT);
            return;
        }

        let items = if let Some(items) = self.course_items(parent) {
            items
        } else if let Some(items) = self.course_item_children(parent) {
            items
        } else {
            reply.error(ENOENT);
            return;
        };

        for (inode, item) in items.iter() {
            if name == item.name {
                reply.entry(
                    &TTL,
                    &match Self::file_type(item) {
                        FileType::RegularFile => {
                            fileattr(*inode, self.client.get_item_size(item) as u64)
                        }
                        FileType::Directory => dirattr(*inode),
                        _ => unreachable!(),
                    },
                    0,
                );
                return;
            }
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
                    match Self::file_type(item) {
                        FileType::RegularFile => {
                            reply.attr(&TTL, &fileattr(ino, self.client.get_item_size(item) as u64))
                        }
                        FileType::Directory => reply.attr(&TTL, &dirattr(ino)),
                        _ => unreachable!(),
                    }
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
        size: u32,
        _flags: i32,
        _lock: Option<u64>,
        reply: ReplyData,
    ) {
        println!("read(ino={ino}, offset={offset}, size={size})");

        if let Some(item) = self.course_item(ino) {
            // TODO: Cache item contents
            let offset = offset as usize;
            let end_offset = offset + (size as usize);
            reply.data(&self.client.get_item_contents(&item.clone())[offset as usize..end_offset])
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
                entries.push((inode, Self::file_type(&item), item.name.clone()));
            }
        } else if let Some(children) = self.course_item_children(ino) {
            for (inode, child) in children {
                entries.push((inode, Self::file_type(&child), child.name.clone()));
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
