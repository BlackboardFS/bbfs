use fuser::{
    FileAttr, FileType, Filesystem, MountOption, ReplyAttr, ReplyData, ReplyDirectory, ReplyEntry,
    Request,
};
use libc::{EIO, ENOENT};
use nix::errno::Errno;
use std::cell::RefCell;
use std::collections::HashMap;
use std::ffi::OsStr;
use std::path::PathBuf;
use std::time::{Duration, UNIX_EPOCH};

use bbfs_scrape::client::{BbClient, BbError, ItemType};

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
        uid: nix::unistd::Uid::current().as_raw(),
        gid: nix::unistd::Gid::current().as_raw(),
        rdev: 0,
        flags: 0,
        blksize: BLOCK_SIZE,
    }
}

fn dirattr(inode: u64) -> FileAttr {
    // TODO: Correctly calculate nlink for directory
    attr(inode, 2, FileType::Directory, 0, 0o500)
}

fn fileattr(inode: u64, size: u64) -> FileAttr {
    // We don't handle symlinks so nlink can just be hardcoded to 1 for files
    attr(inode, 1, FileType::RegularFile, size, 0o400)
}

#[derive(Clone)]
struct ItemInode<Item> {
    parent: Option<u64>,
    ino: u64,
    ty: FileType,
    name: String,
    item: Item,
    /// If None, the children haven't been loaded from the client yet
    children: Option<Vec<u64>>,
}

pub struct Bbfs<Client: BbClient> {
    client: Client,
    next_free_inode: RefCell<u64>,
    inodes: HashMap<u64, ItemInode<Client::Item>>,
}

impl<Client: BbClient> Bbfs<Client> {
    pub fn new(client: Client) -> Result<Bbfs<Client>, BbError> {
        let mut inodes = HashMap::new();
        inodes.insert(
            1,
            ItemInode {
                parent: None,
                ino: 1,
                ty: FileType::Directory,
                name: "root".into(),
                item: client.get_root()?,
                children: None,
            },
        );
        Ok(Bbfs {
            client,
            next_free_inode: RefCell::new(2),
            inodes,
        })
    }

    pub fn mount(self, mount_point: &PathBuf) -> anyhow::Result<()> {
        fuser::mount2(self, mount_point, &[MountOption::RO]).map_err(|err| err.into())
    }

    fn get_free_inode(&self) -> u64 {
        let mut inode = self.next_free_inode.borrow_mut();
        let free_inode = *inode;
        *inode += 1;
        free_inode
    }

    fn attr(&self, inode: &ItemInode<Client::Item>) -> Result<FileAttr, BbError> {
        Ok(match self.client.get_type(&inode.item) {
            ItemType::File => fileattr(inode.ino, self.client.get_size(&inode.item)? as u64),
            ItemType::Directory => dirattr(inode.ino),
        })
    }

    fn path(&self, inode: &ItemInode<Client::Item>) -> Vec<&Client::Item> {
        match inode.parent.and_then(|ino| self.inodes.get(&ino)) {
            Some(parent) => {
                let mut path = self.path(parent);
                path.push(&self.inodes[&inode.ino].item);
                path
            }
            None => vec![&self.inodes[&inode.ino].item],
        }
    }

    fn cached_children(
        &self,
        inode: &ItemInode<Client::Item>,
    ) -> Option<Vec<ItemInode<Client::Item>>> {
        inode.children.as_ref().map(|children| {
            children
                .iter()
                .map(|ino| self.inodes[ino].clone())
                .collect()
        })
    }
}

impl<Client: BbClient> Filesystem for Bbfs<Client> {
    fn lookup(&mut self, _req: &Request, parent: u64, name: &OsStr, reply: ReplyEntry) {
        let name = name.to_str().unwrap();
        println!("lookup(name={name})");

        match self
            .inodes
            .values()
            .find(|inode| inode.parent == Some(parent) && inode.name == name)
        {
            Some(inode) => {
                let attr = match self.attr(inode) {
                    Ok(attr) => attr,
                    Err(err) => return reply.error(Errno::from(err) as _),
                };
                reply.entry(&TTL, &attr, 0)
            }
            None => reply.error(ENOENT),
        }
    }

    fn getattr(&mut self, _req: &Request, ino: u64, reply: ReplyAttr) {
        println!("getattr(ino={ino})");

        let inode = match self.inodes.get(&ino) {
            Some(inode) => inode,
            None => return reply.error(ENOENT),
        };

        match self.attr(inode) {
            Ok(attr) => reply.attr(&TTL, &attr),
            Err(err) => reply.error(Errno::from(err) as _),
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

        let inode = match self.inodes.get(&ino) {
            Some(inode) => inode,
            None => {
                eprintln!("attempted to read from non-existent inode (ino={ino})");
                return reply.error(ENOENT);
            }
        };

        if self.client.get_type(&inode.item) != ItemType::File {
            eprintln!("attempted to read a directory");
            return reply.error(EIO);
        }

        let offset = offset as usize;
        let end_offset = offset + (size as usize);
        match self
            .client
            .get_contents(&inode.item)
            .map_err(Into::<Errno>::into)
            .and_then(|contents| {
                contents
                    .get(offset..end_offset.min(contents.len()))
                    .ok_or(Errno::EIO)
                    .map(|contents| contents.to_vec())
                    .map_err(|err| {
                        eprintln!(
                            "invalid offset ({offset}) for read of file of size {}",
                            contents.len()
                        );
                        err
                    })
            }) {
            Ok(contents) => reply.data(&contents),
            Err(err) => reply.error(err as _),
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
        entries.push((ino, FileType::Directory, "."));

        let children = if let Some(inode) = self.inodes.get(&ino) {
            if self.client.get_type(&inode.item) != ItemType::Directory {
                eprintln!("attempted to read directory contents of a file (ino={ino})");
                return reply.error(EIO);
            }

            entries.push((inode.parent.unwrap_or(1), FileType::Directory, ".."));
            self.cached_children(inode)
        } else {
            eprintln!("attempted to read directory contents of non-existent inode (ino={ino})");
            return reply.error(ENOENT);
        };

        let children = if let Some(children) = children {
            children
        } else {
            let items = match self
                .client
                .get_children(self.path(&self.inodes[&ino]))
                .map_err(Into::<Errno>::into)
            {
                Ok(items) => items,
                Err(err) => return reply.error(err as _),
            };

            let mut inodes = vec![];
            for item in items {
                let child_ino = self.get_free_inode();
                let child_inode = ItemInode {
                    parent: Some(ino),
                    ino: child_ino,
                    ty: match self.client.get_type(&item) {
                        ItemType::File => FileType::RegularFile,
                        ItemType::Directory => FileType::Directory,
                    },
                    name: match self.client.get_name(&item) {
                        Ok(name) => name,
                        Err(err) => return reply.error(Errno::from(err) as _),
                    },
                    item,
                    children: None,
                };
                inodes.push(child_inode.clone());
                self.inodes.insert(child_ino, child_inode);
            }
            self.inodes.get_mut(&ino).unwrap().children =
                Some(inodes.iter().map(|inode| inode.ino).collect());
            self.cached_children(&self.inodes[&ino]).unwrap()
        };

        children
            .iter()
            .for_each(|child| entries.push((child.ino, child.ty, &child.name)));

        for (i, entry) in entries.into_iter().enumerate().skip(offset as usize) {
            if reply.add(entry.0, (i + 1) as i64, entry.1, entry.2) {
                break;
            }
        }
        reply.ok();
    }
}
