use std::ops::Index;
use std::path::PathBuf;
use std::process::Command;
use rfd::MessageDialogResult::No;
use thiserror::Error;
use crate::fleen_app::FleenError::{RootDirNonexistenceError, RootDirPopulatedError};
use crate::fleen_app::TreeEntry::{CloseDir, Dir};

#[derive(Error, Debug, Clone)]
pub enum FleenError {
    #[error("Can't reach root dir {0}")]
    RootDirNonexistenceError(PathBuf),
    #[error("Root dir is nonempty, you probably don't want to create an app here: {0}")]
    RootDirPopulatedError(PathBuf)
}

#[derive(Clone, Debug)]
pub enum TreeEntry {
    File(PathBuf),
    Dir(PathBuf),
    CloseDir
}

#[derive(Clone)]
pub struct FleenApp {
    root: PathBuf,
    files_cache: Option<Vec<TreeEntry>>
}

impl FleenApp {
    pub fn open(root: PathBuf) -> Result<Self, FleenError> {
        match root.try_exists() {
            Ok(true) => Ok(Self { root, files_cache: None }),
            _ => Err(RootDirNonexistenceError(root))
        }
    }

    pub fn create(root: PathBuf) -> Result<Self, FleenError> {
        match root.read_dir() {
            Ok(mut iter) => {
                if iter.next().is_some() {
                    Err(RootDirPopulatedError(root))
                } else {
                    Ok(Self { root, files_cache: None })
                }
            }
            Err(_) => {
                Err(RootDirNonexistenceError(root))
            }
        }
    }

    pub fn file_tree_entries(&mut self) -> impl IntoIterator<Item=TreeEntry> {
        if self.files_cache.is_none() {
            let mut entries = vec![];

            fn visit_dir(dir: &PathBuf, entries: &mut Vec<TreeEntry>) {
                for entry in dir.read_dir().unwrap() {
                    let path = entry.unwrap().path();
                    if path.is_file() {
                        entries.push(TreeEntry::File(path))
                    } else if path.is_dir() {
                        entries.push(Dir(path.clone()));
                        visit_dir(&path, entries);
                        entries.push(CloseDir)
                    }
                }
            }

            visit_dir(&self.root, &mut entries);
            self.files_cache = Some(entries)
        }
        self.files_cache.as_ref().unwrap().clone()
    }

    pub fn open_file_at_index(&self, index: usize) {
        if let TreeEntry::File(path) = &self.files_cache.as_ref().unwrap()[index] {
            Command::new("open").arg(path).spawn();
        }
    }
}
