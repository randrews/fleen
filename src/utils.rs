use std::path::{Path, PathBuf};
use std::process::Command;
use std::{fs, io, time};
use tinyrand::{Rand, Seeded};
use crate::fleen_app::FleenError;

pub fn initialize_site(root: &Path) -> Result<(), io::Error> {
    fs::create_dir(root.join("_layouts"))?;
    fs::create_dir(root.join("_scripts"))?;
    fs::create_dir(root.join("assets"))?;
    fs::create_dir(root.join("images"))?;
    fs::write(root.join("_layouts/default.html"), include_str!("../templates/default_layout.html"))?;
    fs::write(root.join("_scripts/deploy.sh"), include_str!("../templates/deploy.sh"))?;
    fs::write(root.join("assets/.keep"), "")?;
    fs::write(root.join("images/.keep"), "")?;
    Ok(())
}

pub fn open_filename(filename: &str) -> Result<(), FleenError> {
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

pub fn unique_image_name(image_dir: &Path) -> Result<PathBuf, FleenError> {
    let mut rng = tinyrand::StdRand::seed(time::SystemTime::now().duration_since(time::UNIX_EPOCH).unwrap().as_secs());
    loop {
        let fname = format!("image_{}.png", random_name(&mut rng));
        let path = image_dir.join(fname);
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

pub fn open_server(port: &str) {
    // If this doesn't work, not like I can do much about it.
    let _ = Command::new("open").arg(format!("http://localhost:{}", port)).spawn();
}

pub fn label_for_path(path: &Path) -> String {
    path.file_name().unwrap().to_string_lossy().to_string()
}

pub fn id_for_path(path: &Path) -> String {
    path.to_string_lossy().to_string()
}