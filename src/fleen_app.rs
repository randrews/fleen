use std::cell::RefCell;
use std::{fs, io};
use std::path::{Path, PathBuf};
use std::process::Command;
use thiserror::Error;
use crate::fleen_app::FleenError::{RootDirNonexistence, RootDirPopulated};
use crate::fleen_app::TreeEntry::{CloseDir, Dir};

#[derive(Error, Debug, Clone)]
pub enum FleenError {
    #[error("Can't reach root dir {0}")]
    RootDirNonexistence(PathBuf),
    #[error("Root dir is nonempty, you probably don't want to create an app here: {0}")]
    RootDirPopulated(PathBuf),
    #[error("IO error on {0}: {1}")]
    FileIo(String, String),
    #[error("Can't create {0} because it already exists")]
    FileExists(PathBuf),
    #[error("Can't create {0}: {1}")]
    FileCreate(PathBuf, String)
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
            _ => Err(RootDirNonexistence(root))
        }
    }

    pub fn create(root: PathBuf) -> Result<Self, FleenError> {
        match root.read_dir() {
            Ok(mut iter) => {
                if iter.next().is_some() {
                    Err(RootDirPopulated(root))
                } else {
                    Self::initialize_site(root.clone()).map_err(|e| FleenError::FileIo(String::from("creating site"), e.to_string()))?;
                    Ok(Self { root, files_cache: RefCell::new(None) })
                }
            }
            Err(_) => {
                Err(RootDirNonexistence(root))
            }
        }
    }

    fn initialize_site(root: PathBuf) -> Result<(), io::Error> {
        fs::create_dir(root.join("_layouts"))?;
        fs::create_dir(root.join("assets"))?;
        fs::create_dir(root.join("images"))?;
        fs::write(root.join("_layouts/default.html"), include_str!("../templates/default_layout.html"))?;
        fs::write(root.join("assets/.keep"), "")?;
        fs::write(root.join("images/.keep"), "")?;
        Ok(())
    }

    // TODO: This is panicky as hell, make it return a Result
    fn refresh_file_cache(&self, force: bool) {
        if self.files_cache.borrow().is_none() || force {
            let mut entries = vec![];

            fn visit_dir(dir: &Path, entries: &mut Vec<TreeEntry>) {
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

    pub fn open_filename(&self, filename: &str) -> Result<(), FleenError> {
        // TODO this doesn't work for html files. We really want to open things in a platform-dependent way
        // - md, html, all other text, should open in gedit on linux or the user's preferred editor on mac
        // - images should open in an image viewer preferably
        // - dirs should open in a file browser
        // - on mac we can open -t to force a text editor
        // - on linux we should maybe allow a FLEEN_EDITOR env var, defaulting to EDITOR if missing?
        Command::new("open").arg(filename).spawn().map_err(|err| {
            FleenError::FileIo(filename.to_owned(), err.to_string())
        })?;
        Ok(())
    }

    pub fn open_server(&self, port: &str) {
        // If this doesn't work, not like I can do much about it.
        let _ = Command::new("open").arg(format!("http://localhost:{}", port)).spawn();
    }

    pub fn create_page(&self, file_type: FileType, name: &str, parent: Option<&String>) -> Result<(), FleenError> {
        let mut target = match parent {
            Some(s) => PathBuf::from(s),
            None => self.root.clone()
        };
        target.push(name);
        if target.exists() {
            return Err(FleenError::FileExists(target))
        }

        let contents = if name.ends_with(".md") {
            include_str!("../templates/markdown_template.md")
        } else if name.ends_with(".html") {
            include_str!("../templates/default_layout.html")
        } else {
            ""
        };

        match file_type {
            FileType::File => std::fs::write(target.clone(), contents),
            FileType::Dir => std::fs::create_dir(target.clone())
        }.map_err(|err| FleenError::FileCreate(target.clone(), err.to_string()))?;

        self.refresh_file_cache(true);
        if file_type == FileType::File {
            self.open_filename(target.to_string_lossy().as_ref())?
        }
        Ok(())
    }

    pub fn delete_page(&self, path: &String) -> Result<(), FleenError> {
        let target = PathBuf::from(path);
        if target.is_dir() {
            fs::remove_dir_all(target)
        } else {
            fs::remove_file(target)
        }.map_err(|err| FleenError::FileIo(path.clone(), err.to_string()))?;
        self.refresh_file_cache(true);
        Ok(())
    }

    pub fn rename_page(&self, target: &String, new_name: &str) -> Result<(), FleenError> {
        let path = PathBuf::from(target);
        let mut new_path = path.clone();
        new_path.set_file_name(new_name);
        fs::rename(path, new_path).map_err(|err| FleenError::FileIo(target.clone(), err.to_string()))?;
        self.refresh_file_cache(true);
        Ok(())
    }

    pub fn root_path(&self) -> String {
        self.root.to_string_lossy().to_string()
    }
}
