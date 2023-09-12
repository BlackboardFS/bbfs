use std::collections::HashMap;
use std::path::{Path, PathBuf};
use std::sync::atomic::{self, AtomicU64};
use std::sync::{Mutex, MutexGuard, OnceLock};
use std::time::SystemTime;

use camino::Utf8PathBuf;
use dokan::{CreateFileInfo, FileSystemHandler, FileSystemMounter, MountOptions};
use widestring::UCString;
use winapi::shared::ntdef::NTSTATUS;
use winapi::shared::ntstatus::{
    STATUS_DATA_ERROR, STATUS_FILE_IS_A_DIRECTORY, STATUS_NO_SUCH_FILE,
};
use winapi::um::winnt::{FILE_ATTRIBUTE_DIRECTORY, FILE_ATTRIBUTE_READONLY};

use bbfs_scrape::client::{BbClient, BbError, ItemType};

#[derive(Clone)]
pub struct ItemNode<Item> {
    path: Utf8PathBuf,
    index: u64,
    is_dir: bool,
    item: Item,
    children: OnceLock<Vec<Utf8PathBuf>>,
}

pub struct Bbfs<Client: BbClient> {
    client: Client,
    // Not a Utf8PathBuf because we cannot ensure it and don't really need to
    mount_point: OnceLock<PathBuf>,
    paths: Mutex<HashMap<Utf8PathBuf, ItemNode<Client::Item>>>,
    next_index: AtomicU64,
}

impl<Client: BbClient> Bbfs<Client> {
    pub fn new(client: Client) -> Result<Self, BbError> {
        Ok(Bbfs {
            client,
            mount_point: OnceLock::new(),
            paths: Default::default(),
            next_index: AtomicU64::new(0),
        })
    }

    pub fn mount(&self, mount_point: impl AsRef<Path>) -> anyhow::Result<()> {
        dokan::init();
        let mount_point_ucstr = UCString::<u16>::from_os_str(mount_point.as_ref()).unwrap();
        // TODO: Consider timeouts when moving auth to API client
        let mount_options = MountOptions::default();
        let mut mounter = FileSystemMounter::new(self, &mount_point_ucstr, &mount_options);
        mounter.mount().unwrap();
        Ok(())
    }
}

impl<Client: BbClient> Bbfs<Client> {
    fn item_ancestors<'a>(
        &self,
        lock: &'a MutexGuard<'_, HashMap<Utf8PathBuf, ItemNode<Client::Item>>>,
        node: &ItemNode<Client::Item>,
    ) -> Vec<&'a Client::Item> {
        let mut ancestors = node
            .path
            .ancestors()
            .map(|path| &lock.get(path).unwrap().item)
            .collect::<Vec<_>>();
        ancestors.reverse();
        ancestors
    }

    fn normalize_path(&self, path: &widestring::U16CStr) -> Utf8PathBuf {
        let path = PathBuf::from(path.to_os_string());
        Utf8PathBuf::from_path_buf(
            path.strip_prefix(self.mount_point.get().unwrap())
                .map(ToOwned::to_owned)
                .unwrap_or(path),
        )
        .expect("internal paths should be valid UTF-8")
    }

    fn next_index(&self) -> u64 {
        self.next_index.fetch_add(1, atomic::Ordering::SeqCst)
    }

    fn sanitize_name(&self, item: String) -> String {
        String::from_utf8(
            item.into_bytes()
                .into_iter()
                .map(|b| {
                    if matches!(b, b'<' | b'>' | b':' | b'\\' | b'|' | b'?' | b'*') {
                        b'-'
                    } else {
                        b
                    }
                })
                .collect::<Vec<_>>(),
        )
        .expect("transformed string should remain valid utf-8")
    }
}

impl<'c, 'h: 'c, Client: BbClient + 'h> FileSystemHandler<'c, 'h> for Bbfs<Client> {
    type Context = ItemNode<Client::Item>;

    fn mounted(
        &'h self,
        mount_point: &widestring::U16CStr,
        _info: &dokan::OperationInfo<'c, 'h, Self>,
    ) -> dokan::OperationResult<()> {
        // Create the root ItemNode
        // TODO: Move this to fs creation, for consistency?
        let mut lock = self.paths.lock().unwrap();

        let item = self.client.get_root().unwrap();
        let node = ItemNode {
            path: Utf8PathBuf::from(r"\"),
            index: self.next_index(),
            is_dir: true,
            item,
            children: OnceLock::new(),
        };

        lock.insert(Utf8PathBuf::from(r"\"), node);

        // Remember the mount point (because I don't want to pass `info` everywhere)
        self.mount_point
            .set(PathBuf::from(mount_point.to_os_string()))
            .expect("`mounted` should only be called once");
        Ok(())
    }

    fn create_file(
        &'h self,
        file_name: &widestring::U16CStr,
        _security_context: &dokan::IO_SECURITY_CONTEXT,
        _desired_access: winapi::um::winnt::ACCESS_MASK,
        _file_attributes: u32,
        _share_access: u32,
        _create_disposition: u32,
        _create_options: u32,
        _info: &mut dokan::OperationInfo<'c, 'h, Self>,
    ) -> dokan::OperationResult<dokan::CreateFileInfo<Self::Context>> {
        let lock = self.paths.lock().unwrap();
        let path = Utf8PathBuf::from(self.normalize_path(file_name));
        println!("create_file {path}");
        match lock.get(&path) {
            Some(item) => Ok(CreateFileInfo {
                is_dir: item.is_dir,
                context: item.clone(),
                new_file_created: false,
            }),
            None => Err(STATUS_NO_SUCH_FILE),
        }
    }

    fn read_file(
        &'h self,
        _file_name: &widestring::U16CStr,
        offset: i64,
        buffer: &mut [u8],
        _info: &dokan::OperationInfo<'c, 'h, Self>,
        node: &'c Self::Context,
    ) -> dokan::OperationResult<u32> {
        println!(
            "read_file {} offset: {offset} size: {}",
            node.path,
            buffer.len()
        );

        if self.client.get_type(&node.item) != ItemType::File {
            eprintln!("attempted to read a directory");
            return Err(STATUS_FILE_IS_A_DIRECTORY);
        }

        let offset = offset as usize;
        let end_offset = offset + buffer.len();
        let contents = self
            .client
            .get_contents(&node.item)
            .map_err(Into::<NTSTATUS>::into)
            .and_then(|contents| {
                contents
                    .get(offset..end_offset.min(contents.len()))
                    .ok_or(STATUS_DATA_ERROR)
                    .map(|contents| contents.to_vec())
                    .map_err(|err| {
                        eprintln!(
                            "invalid offset ({offset}) for read of file of size {}",
                            contents.len()
                        );
                        err
                    })
            })?;
        buffer[..contents.len()].copy_from_slice(&contents);
        Ok(contents.len() as _)
    }

    fn get_file_information(
        &'h self,
        _file_name: &widestring::U16CStr,
        _info: &dokan::OperationInfo<'c, 'h, Self>,
        node: &'c Self::Context,
    ) -> dokan::OperationResult<dokan::FileInfo> {
        Ok(dokan::FileInfo {
            // ? Should FILE_ATTRIBUTE_OFFLINE be set here as well?
            attributes: FILE_ATTRIBUTE_READONLY
                | if node.is_dir {
                    FILE_ATTRIBUTE_DIRECTORY
                } else {
                    0
                },
            creation_time: SystemTime::UNIX_EPOCH,
            last_access_time: SystemTime::UNIX_EPOCH,
            last_write_time: SystemTime::UNIX_EPOCH,
            file_size: if node.is_dir {
                0
            } else {
                self.client.get_size(&node.item)? as _
            },
            number_of_links: 1,
            file_index: node.index,
        })
    }

    fn find_files(
        &'h self,
        _file_name: &widestring::U16CStr,
        mut fill_find_data: impl FnMut(&dokan::FindData) -> dokan::FillDataResult,
        _info: &dokan::OperationInfo<'c, 'h, Self>,
        node: &'c Self::Context,
    ) -> dokan::OperationResult<()> {
        let mut lock = self.paths.lock().unwrap();
        println!("find_files {}", node.path);

        let children = if let Some(children) = node.children.get() {
            children
        } else {
            let items = self.client.get_children(self.item_ancestors(&lock, node))?;

            let mut paths = vec![];
            for item in items {
                let item_name = self.sanitize_name(self.client.get_name(&item)?);
                let child_node = ItemNode {
                    path: node.path.join(item_name),
                    index: self.next_index(),
                    is_dir: matches!(self.client.get_type(&item), ItemType::Directory),
                    item,
                    children: OnceLock::new(),
                };
                paths.push(child_node.path.clone());
                lock.insert(child_node.path.clone(), child_node);
            }
            node.children
                .set(paths)
                .expect("only one thread should be resolving children at a time");
            node.children.get().unwrap()
        };

        for child in children {
            let child = &lock[child];
            fill_find_data(&dokan::FindData {
                // ? Should FILE_ATTRIBUTE_OFFLINE be set here as well?
                attributes: FILE_ATTRIBUTE_READONLY
                    | if child.is_dir {
                        FILE_ATTRIBUTE_DIRECTORY
                    } else {
                        0
                    },
                creation_time: SystemTime::UNIX_EPOCH,
                last_access_time: SystemTime::UNIX_EPOCH,
                last_write_time: SystemTime::UNIX_EPOCH,
                file_size: if child.is_dir {
                    0
                } else {
                    self.client.get_size(&child.item)? as _
                },
                file_name: UCString::<u16>::from_str(
                    child.path.file_name().expect("paths should all have names"),
                )
                .expect("file paths should not contain NULs"),
            })
            .unwrap();
        }
        Ok(())
    }
}
