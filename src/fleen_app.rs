use std::cell::RefCell;
use std::{fs, io, time};
use std::path::{Path, PathBuf};
use std::process::Command;
use clipboard_rs::Clipboard;
use clipboard_rs::common::RustImage;
use tempfile::TempDir;
use thiserror::Error;
use tinyrand::{Rand, Seeded};
use tokio::task::JoinHandle;
use crate::fleen_app::FleenError::{RootDirNonexistence, RootDirPopulated, TargetDir};
use crate::fleen_app::TreeEntry::{CloseDir, Dir};
use crate::renderer;
use crate::renderer::{RenderError, RenderOutput};

#[derive(Error, Debug)]
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
    FileCreate(PathBuf, String),
    #[error("Image dir doesn't exist")]
    NoImageDir,
    #[error("No image on clipboard")]
    NoClipboardImage,
    #[error("Render error: {0}")]
    RenderError(#[from] RenderError),
    #[error("Target dir is invalid (can't contain the app dir or vice versa)")]
    TargetDir,
    #[error("IO error: {0}")]
    Io(#[from] io::Error),
    #[error("Deploy script missing! Create _scripts/deploy.sh")]
    ScriptMissing,
    #[error("Deploy script error:\n\n{0}")]
    DeployError(String)
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
        if matches!(file_type, FileType::Dir) {
            while target.is_file() { target.pop(); }
        }

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
            FileType::File => fs::write(target.clone(), contents),
            FileType::Dir => fs::create_dir(target.clone())
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

    pub fn image_dir_exists(&self) -> bool {
        self.root.join("images").is_dir()
    }

    pub fn unique_image_name(&self) -> Result<PathBuf, FleenError> {
        if !self.image_dir_exists() { return Err(FleenError::NoImageDir) }
        let mut rng = tinyrand::StdRand::seed(time::SystemTime::now().duration_since(time::UNIX_EPOCH).unwrap().as_secs());
        loop {
            let fname = format!("image_{}.png", Self::random_name(&mut rng));
            let path = self.root.join("images").join(fname);
            if !path.exists() {
                return Ok(path)
            }
        }
    }

    fn random_name(rng: &mut tinyrand::StdRand) -> String {
        let mut s = String::new();
        let chs = ['0', '1', '2', '3', '4', '5', '6', '7', '8', '9', 'a', 'b', 'c', 'd', 'e', 'f'];
        for _ in 0..8 {
            let n = rng.next_lim_usize(chs.len());
            s.push(chs[n]);
        }
        s
    }

    pub fn paste_image(&self) -> Result<String, FleenError> {
        let c = clipboard_rs::ClipboardContext::new().map_err(|_| FleenError::NoClipboardImage)?;
        let img = c.get_image().map_err(|_| FleenError::NoClipboardImage)?;
        let target_path = self.unique_image_name()?;
        img.save_to_path(target_path.to_str().unwrap()).map_err(|e| FleenError::FileCreate(target_path.clone(), e.to_string()))?;
        self.refresh_file_cache(true);
        let uri = format!("/images/{}", target_path.file_name().unwrap().to_str().unwrap());
        let _ = c.set_text(format!("![]({})", uri));
        Ok("Image saved!".to_string())
    }

    pub fn compile(&self) -> Result<Vec<RenderOutput>, FleenError> {
        let mut sources = vec![]; // The list of renderoutputs we need to perform

        // Traverse a directory
        fn visit_dir(dir: &Path, root: &Path, sources: &mut Vec<RenderOutput>) -> Result<(), RenderError> {
            // Root is the app root. Dir is the directory path within the app root, like "assets".
            // File is the filename (or child dir name) within the dir, so, root+dir+file is an
            // absolute path
            for entry in root.join(dir).read_dir().unwrap() {
                let file = PathBuf::from(entry.unwrap().file_name());
                sources.push(renderer::file_render(dir.join(&file), root)?);
                if root.join(dir).join(&file).is_dir() {
                    // root + dir + file is a child directory, so we want to recurse...
                    // into dir + file.
                    visit_dir(&dir.join(&file), root, sources)?
                }
            }
            Ok(())
        }
        visit_dir(Path::new(""), &self.root, &mut sources)?;
        Ok(sources)
    }

    pub fn build_site(&self, target: &Path) -> Result<(), FleenError> {
        // Ensure neither the target nor src dirs are ancestors of the other
        if self.root.ancestors().any(|a| a == target) ||
            target.ancestors().any(|a| a == self.root) {
            return Err(TargetDir)
        }

        // Clear the target directory first:
        for entry in fs::read_dir(target)? {
            let entry = entry?;
            if entry.file_type()?.is_dir() {
                fs::remove_dir_all(entry.path())?
            } else {
                fs::remove_file(entry.path())?
            }
        }

        // Decide which actions we need to do to build the site
        let actions = self.compile()?;

        // And then do them!
        for action in actions.into_iter() {
            action.file_operation(&self.root, target)?;
        }
        Ok(())
    }

    pub fn build_and_deploy(&self) -> Result<JoinHandle<Result<String, FleenError>>, FleenError> {
        let output_dir = tempfile::tempdir().map_err(|_| TargetDir)?;
        self.build_site(&output_dir.path())?; // Attempt to build the site somewhere

        let deploy_script_path = self.root.join("_scripts/deploy.sh");
        if !deploy_script_path.exists() {
            Err(FleenError::ScriptMissing)
        } else {
            let mut command = Command::new(deploy_script_path);
            command.current_dir(output_dir.path()); // don't consume dir!
            Ok(self.build_and_deploy_inner(output_dir, command))
        }
    }

    fn build_and_deploy_inner(&self, output_dir: TempDir, mut command: Command) -> JoinHandle<Result<String, FleenError>> {
        tokio::spawn(async move {
            let output = command.output();
            let status = command.status().unwrap();
            output_dir.close().map_err(|e| FleenError::Io(e))?;

            match output {
                Ok(output) => {
                    let output = String::from_utf8(output.stdout).unwrap_or("Error reading deploy script output".to_string());
                    if status.success() {
                        Ok(output)
                    } else {
                        Err(FleenError::DeployError(output))
                    }
                },
                Err(e) => {
                    Err(FleenError::DeployError(e.to_string()))
                }
            }
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn find_rendered_index(actions: &Vec<RenderOutput>, path: &str) -> Option<usize> {
        let path = PathBuf::from(path);
        actions.iter().position(|a| matches!(a, RenderOutput::Rendered(p, _) if p == &path))
    }

    fn find_dir_index(actions: &Vec<RenderOutput>, path: &str) -> Option<usize> {
        let path = PathBuf::from(path);
        actions.iter().position(|a| matches!(a, RenderOutput::Dir(p) if p == &path))
    }

    fn find_raw_index(actions: &Vec<RenderOutput>, path: &str) -> Option<usize> {
        let path = PathBuf::from(path);
        actions.iter().position(|a| matches!(a, RenderOutput::RawFile(p) if p == &path))
    }

    #[test]
    fn test_compile() {
        let app = FleenApp::open(PathBuf::from("./testdata")).unwrap();
        let actions = app.compile().unwrap();

        // Rendered files in the root that exist
        assert!(find_rendered_index(&actions,"index.html").is_some());
        assert!(find_rendered_index(&actions,"nolayout.html").is_some());
        assert!(find_rendered_index(&actions,"not_hidden.html").is_some());

        // Rendered files that are hidden for whatever reason
        assert!(find_rendered_index(&actions,"hidden.html").is_none());
        assert!(find_rendered_index(&actions,"_skipped.html").is_none());

        // The original md files are not reproduced:
        assert!(find_rendered_index(&actions,"index.md").is_none());
        assert!(find_rendered_index(&actions,"nolayout.md").is_none());
        assert!(find_rendered_index(&actions,"not_hidden.md").is_none());

        // A subdir
        assert!(find_dir_index(&actions, "dir").is_some());

        // The thing in it should be made after the dir itself:
        let file_idx = find_rendered_index(&actions, "dir/subdir.html").unwrap();
        let dir_idx = find_dir_index(&actions, "dir").unwrap();
        assert!(file_idx > dir_idx);

        // Raw files should be produced:
        assert!(find_raw_index(&actions, "raw.txt").is_some());

        // But not hidden ones:
        assert!(find_raw_index(&actions, "_layouts/post.html").is_none());
    }
}