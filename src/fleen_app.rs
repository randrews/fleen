use std::cell::RefCell;
use std::path::PathBuf;
use std::process::Command;
use thiserror::Error;
use crate::fleen_app::FleenError::{RootDirNonexistenceError, RootDirPopulatedError};
use crate::fleen_app::TreeEntry::{CloseDir, Dir};

#[derive(Error, Debug, Clone)]
pub enum FleenError {
    #[error("Can't reach root dir {0}")]
    RootDirNonexistenceError(PathBuf),
    #[error("Root dir is nonempty, you probably don't want to create an app here: {0}")]
    RootDirPopulatedError(PathBuf),
    #[error("Failed to open {0}: {1}")]
    FileOpenError(String, String)
}

#[derive(Clone, Debug)]
pub enum TreeEntry {
    File(PathBuf),
    Dir(PathBuf),
    CloseDir
}

pub struct FleenApp {
    root: PathBuf,
    files_cache: RefCell<Option<Vec<TreeEntry>>>
}

impl FleenApp {
    pub fn open(root: PathBuf) -> Result<Self, FleenError> {
        match root.try_exists() {
            Ok(true) => Ok(Self { root, files_cache: RefCell::new(None) }),
            _ => Err(RootDirNonexistenceError(root))
        }
    }

    pub fn create(root: PathBuf) -> Result<Self, FleenError> {
        match root.read_dir() {
            Ok(mut iter) => {
                if iter.next().is_some() {
                    Err(RootDirPopulatedError(root))
                } else {
                    Ok(Self { root, files_cache: RefCell::new(None) })
                }
            }
            Err(_) => {
                Err(RootDirNonexistenceError(root))
            }
        }
    }

    // TODO: This is panicky as hell, make it return a Result
    fn refresh_file_cache(&self, force: bool) {
        if self.files_cache.borrow().is_none() || force {
            let mut entries = vec![];

            fn visit_dir(dir: &PathBuf, entries: &mut Vec<TreeEntry>) {
                for entry in dir.read_dir().unwrap() {
                    let path = entry.unwrap().path();
                    if path.is_file() && !path.file_name().unwrap().to_str().unwrap().starts_with('.') {
                        entries.push(TreeEntry::File(path))
                    } else if path.is_dir() {
                        entries.push(Dir(path.clone()));
                        visit_dir(&path, entries);
                        entries.push(CloseDir)
                    }
                }
            }

            entries.push(Dir(self.root.clone()));
            visit_dir(&self.root, &mut entries);
            entries.push(CloseDir);

            self.files_cache.replace(Some(entries));
        }
    }

    pub fn file_tree_entries(&self) -> impl IntoIterator<Item=TreeEntry> {
        self.refresh_file_cache(false);
        self.files_cache.borrow().clone().expect("Can't happen because we just refreshed the cache")
    }

    pub fn open_filename(&self, filename: &String) -> Result<(), FleenError> {
        Command::new("open").arg(filename.clone()).spawn().map_err(|err| {
            FleenError::FileOpenError(filename.clone(), err.to_string())
        })?;
        Ok(())
    }
}
