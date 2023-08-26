use fuser::{
    FileAttr, FileType, Filesystem, ReplyAttr, ReplyData, ReplyDirectory, ReplyEntry, Request,
};
use lib_bb::{Course, CourseItem, CourseItemContent};
use libc::ENOENT;
use nix::errno::Errno;
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

    fn populate_courses(&mut self) -> Result<(), Errno> {
        for course in self.client.get_courses()?.into_iter() {
            let inode = self.get_free_inode();
            self.courses.insert(
                inode,
                CourseInode {
                    course,
                    items: None,
                },
            );
        }
        Ok(())
    }

    fn course(&self, inode: u64) -> Option<&CourseInode> {
        self.courses.get(&inode)
    }

    #[cfg(target_os = "linux")]
    fn create_hyperlink_item(&mut self, hyperlink: String) -> u64 {
        let inode = self.get_free_inode();
        self.items.insert(
            inode,
            CourseItemInode {
                item: CourseItem {
                    name: "Blackboard.desktop".to_owned(),
                    file_name: None,
                    content: None,
                    description: Some(format!(
                        "\
[Desktop Entry]
Encoding=UTF-8
Type=Link
URL=https://learn.uq.edu.au{hyperlink}
Icon=text-html
"
                    )),
                    attachments: vec![],
                },
                children: Some(vec![]),
            },
        );
        inode
    }

    #[cfg(target_os = "macos")]
    fn create_hyperlink_item(&mut self, hyperlink: String) -> u64 {
        let inode = self.get_free_inode();
        self.items.insert(
            inode,
            CourseItemInode {
                item: CourseItem {
                    name: "Blackboard.webloc".to_owned(),
                    file_name: None,
                    content: None,
                    description: Some(format!(
                        r#"{ URL = "https://learn.uq.edu.au{hyperlink}"; }"#
                    )),
                    attachments: vec![],
                },
                children: Some(vec![]),
            },
        );
        inode
    }

    fn course_items(&mut self, course_inode: u64) -> Result<Option<Vec<(u64, CourseItem)>>, Errno> {
        let course = match self.course(course_inode) {
            Some(course) => course,
            None => return Ok(None),
        };

        let inodes = if let Some(items) = &course.items {
            items.clone()
        } else {
            let mut inodes = Vec::new();
            let course_id = course.course.id.clone(); // TODO: Shouldn't really be clone but lifetimes

            for item in self.client.get_course_contents(&course.course)?.into_iter() {
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

            inodes.push(
                self.create_hyperlink_item(format!("/ultra/courses/{}/cl/outline", course_id)),
            );

            self.courses.get_mut(&course_inode).unwrap().items = Some(inodes.clone());

            inodes
        };

        Ok(Some(
            inodes
                .iter()
                .map(|inode| (*inode, self.items.get(inode).unwrap().item.clone()))
                .collect(),
        ))
    }

    fn course_item(&self, inode: u64) -> Option<&CourseItem> {
        self.items.get(&inode).map(|item| &item.item)
    }

    fn course_item_children(
        &mut self,
        inode: u64,
    ) -> Result<Option<Vec<(u64, CourseItem)>>, Errno> {
        let item = match self.items.get(&inode) {
            Some(item) => item,
            None => return Ok(None),
        };

        // If the children have already been fetched return them
        let inodes = if let Some(children) = &item.children {
            children.clone()
        } else {
            let (url, contents) = match item.item.content {
                Some(CourseItemContent::FolderUrl(ref url)) => (
                    url.clone(),
                    self.client.get_directory_contents(url.clone())?,
                ),
                _ => return Ok(None),
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

            inodes.push(self.create_hyperlink_item(url));

            self.items.get_mut(&inode).unwrap().children = Some(inodes.clone());

            inodes
        };

        Ok(Some(
            inodes
                .iter()
                .map(|inode| (*inode, self.items.get(inode).unwrap().item.clone()))
                .collect(),
        ))
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

        let items = match self.course_items(parent) {
            Ok(Some(items)) => items,
            Ok(None) => match self.course_item_children(parent) {
                Ok(Some(items)) => items,
                Ok(None) => {
                    reply.error(ENOENT);
                    return;
                }
                Err(errno) => {
                    reply.error(errno as _);
                    return;
                }
            },
            Err(errno) => {
                reply.error(errno as _);
                return;
            }
        };

        for (inode, item) in items.iter() {
            if name == item.name {
                match Self::file_type(item) {
                    FileType::RegularFile => match self.client.get_item_size(item) {
                        Ok(size) => reply.entry(&TTL, &fileattr(*inode, size as u64), 0),
                        Err(errno) => {
                            reply.error(errno as _);
                            return;
                        }
                    },
                    FileType::Directory => reply.entry(&TTL, &dirattr(*inode), 0),
                    _ => unreachable!(),
                };
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
                        FileType::RegularFile => match self.client.get_item_size(item) {
                            Ok(size) => reply.attr(&TTL, &fileattr(ino, size as u64)),
                            Err(errno) => {
                                reply.error(errno as _);
                                return;
                            }
                        },
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
            match self
                .client
                .get_item_contents(&item.clone())
                .and_then(|contents| {
                    contents
                        .get(offset..end_offset.min(contents.len()))
                        .ok_or(Errno::EIO)
                }) {
                Ok(contents) => reply.data(contents),
                Err(errno) => {
                    reply.error(errno as _);
                    return;
                }
            }
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
        if offset == 0 {
            entries.push((ino, FileType::Directory, ".".into()));
        }
        if ino == 1 {
            entries.push((1, FileType::Directory, "..".into()));

            for (inode, course) in self.courses.iter() {
                entries.push((
                    *inode,
                    FileType::RegularFile,
                    Self::course_dir_name(&course.course),
                ))
            }
        } else {
            match self.course_items(ino) {
                Ok(Some(items)) => {
                    entries.push((1, FileType::Directory, "..".into()));

                    for (inode, item) in items {
                        entries.push((inode, Self::file_type(&item), item.name.clone()));
                    }
                }
                Ok(None) => match self.course_item_children(ino) {
                    Ok(Some(children)) => {
                        for (inode, child) in children {
                            entries.push((inode, Self::file_type(&child), child.name.clone()));
                        }
                    }
                    Ok(None) => {
                        reply.error(ENOENT);
                        return;
                    }
                    Err(errno) => {
                        reply.error(errno as _);
                        return;
                    }
                },
                Err(errno) => {
                    reply.error(errno as _);
                    return;
                }
            }
        }

        for (i, entry) in entries.into_iter().enumerate().skip(offset as usize) {
            if reply.add(entry.0, (i + 1) as i64, entry.1, entry.2) {
                break;
            }
        }
        reply.ok();
    }
}
