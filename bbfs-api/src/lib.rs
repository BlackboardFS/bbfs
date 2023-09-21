use std::error::Error;

pub trait BbClient: Sync {
    type Item: Clone + Send + Sync;

    #[cfg(unix)]
    type Error: Error + Into<nix::errno::Errno> + Send + Sync;
    #[cfg(windows)]
    type Error: Error + Into<winapi::shared::ntdef::NTSTATUS> + Send + Sync;

    fn get_root(&self) -> Result<Self::Item, Self::Error>;
    fn get_children(&self, path: Vec<&Self::Item>) -> Result<Vec<Self::Item>, Self::Error>;
    fn get_size(&self, item: &Self::Item) -> Result<usize, Self::Error>;
    fn get_contents(&self, item: &Self::Item) -> Result<Vec<u8>, Self::Error>;
    fn get_type(&self, item: &Self::Item) -> ItemType;
    fn get_name(&self, item: &Self::Item) -> Result<String, Self::Error>;
}

#[derive(PartialEq)]
pub enum ItemType {
    File,
    Directory,
}
