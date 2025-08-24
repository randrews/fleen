use std::cell::RefCell;
use std::fs;
use std::io::Error;
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
    FileIoError(String, String),
    #[error("Can't create {0} because it already exists")]
    FileExistsError(PathBuf),
    #[error("Can't create {0}: {1}")]
    FileCreateError(PathBuf, String)
}

#[derive(Clone, Debug)]
pub enum TreeEntry {
    File(PathBuf),
    Dir(PathBuf),
    CloseDir
}

#[derive(Copy, Clone, Debug, PartialEq)]
pub enum FileType {
    File, Dir
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
            FleenError::FileIoError(filename.clone(), err.to_string())
        })?;
        Ok(())
    }

    pub fn create_page(&self, file_type: FileType, name: &String, parent: Option<&String>) -> Result<(), FleenError> {
        let mut target = match parent {
            Some(s) => PathBuf::from(s),
            None => self.root.clone()
        };
        target.push(name.clone());
        if target.exists() {
            return Err(FleenError::FileExistsError(target))
        }

        match file_type {
            FileType::File => std::fs::write(target.clone(), []),
            FileType::Dir => std::fs::create_dir(target.clone())
        }.map_err(|err| FleenError::FileCreateError(target.clone(), err.to_string()))?;

        self.refresh_file_cache(true);
        if file_type == FileType::File {
            self.open_filename(&target.to_string_lossy().to_string())?
        }
        Ok(())
    }

    pub fn delete_page(&self, path: &String) -> Result<(), FleenError> {
        let target = PathBuf::from(path);
        if target.is_dir() {
            fs::remove_dir_all(target)
        } else {
            fs::remove_file(target)
        }.map_err(|err| FleenError::FileIoError(path.clone(), err.to_string()))?;
        self.refresh_file_cache(true);
        Ok(())
    }
}
