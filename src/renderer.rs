use std::{fs, io};
use std::io::Error;
use std::path::{Path, PathBuf};
use markdown::message::Message;
use markdown::{Constructs, Options, ParseOptions};
use markdown::mdast::Node;
use serde::Deserialize;
use thiserror::Error;
use crate::fleen_app::FleenError;

/// The things we might return from trying to render a file
#[derive(Clone, PartialEq, Debug)]
pub enum RenderOutput {
    /// A string which had been rendered from some source file or script
    Rendered(PathBuf, String),
    /// Hidden output should be returned by the test server but not rendered to a file
    Hidden(PathBuf, String),
    /// The raw contents of the given (relative) path.
    RawFile(PathBuf),
    /// No contents; this file should not be output / test server should return 404
    NoOutput,
    /// This is a directory; the test server won't return anything but we need to mkdir it
    Dir(PathBuf)
}

impl RenderOutput {
    pub fn file_operation(&self, root: &Path, target: &Path) -> Result<(), io::Error> {
        match self {
            RenderOutput::Rendered(path, contents) => {
                fs::write(target.join(path), contents)
            }
            RenderOutput::Hidden(_, _) | RenderOutput::NoOutput => Ok(()), // Don't do anything!
            RenderOutput::RawFile(path) => {
                fs::copy(root.join(path), target.join(path))?;
                Ok(())
            }
            RenderOutput::Dir(path) => {
                fs::create_dir(target.join(path))
            }
        }
    }
}

#[derive(Deserialize)]
pub struct Frontmatter {
    layout: Option<String>,
    title: Option<String>,
    published: Option<bool>
}

impl Frontmatter {
    fn apply_layout(self, content: String, filename: PathBuf, root: &Path) -> Result<RenderOutput, RenderError> {
        let title = self.title.unwrap_or_default();
        let wrapped = if let Some(layout) = self.layout {
            let absolute_layout = root.join(layout);
            let layout = fs::read_to_string(absolute_layout.clone()).map_err(|e| RenderError::FileRead(e, absolute_layout))?;
            layout.replace("$title", title.as_str()).replace("$content", content.as_str())
        } else {
            content
        };
        if let Some(false) = self.published {
            Ok(RenderOutput::Hidden(filename.with_extension("html"), wrapped))
        } else {
            Ok(RenderOutput::Rendered(filename.with_extension("html"), wrapped))
        }
    }
}

#[derive(Error, Debug)]
pub enum RenderError {
    #[error("Error reading {1}: {0}")]
    FileRead(Error, PathBuf),
    #[error("Error parsing Markdown {1}: {0}")]
    MarkdownParse(Message, PathBuf),
    #[error("Error parsing frontmatter in {1}: {0}")]
    FrontmatterParse(toml::de::Error, PathBuf)
}

/// Take a source file path (relative to the root) and the root path, and return a RenderOutput for it.
/// This function is called for server output, which has different rules from file output.
pub fn server_render(source: PathBuf, root: &Path) -> Result<RenderOutput, RenderError> {
    let extension = source.extension().map(|o| o.to_str().unwrap());
    if skipped_path(source.clone()) {
        // Skipped path, nothing
        Ok(RenderOutput::NoOutput)
    } else if root.join(source.clone()).is_dir() {
        // Dir, which matters for producing files
        Ok(RenderOutput::Dir(source))
    } else if let Ok(true) = fs::exists(root.join(source.clone())) {
        match extension {
            // Asked for a markdown file, but those become html, and we should request it as html:
            Some("md") => Ok(RenderOutput::NoOutput),
            // Not a markdown file, but it exists, return it raw
            _ => Ok(RenderOutput::RawFile(source))
        }
    } else if matches!(extension, Some("html")) &&
        let Ok(true) = fs::exists(root.join(source.with_extension("md"))) {
        // We asked for an html file which doesn't exist but a corresponding md file does, render it
        render_as_markdown(source.with_extension("md"), root)
    } else {
        // Asked for something which doesn't exist and it's not the md -> html case, 404:
        Ok(RenderOutput::NoOutput)
    }
}

pub fn file_render(source: PathBuf, root: &Path) -> Result<RenderOutput, RenderError> {
    let extension = source.extension().map(|o| o.to_str().unwrap());
    if skipped_path(source.clone()) {
        // Skipped path, nothing
        Ok(RenderOutput::NoOutput)
    } else if root.join(source.clone()).is_dir() {
        // Dir, which matters for producing files
        Ok(RenderOutput::Dir(source))
    } else if let Ok(true) = fs::exists(root.join(source.clone())) {
        match extension {
            // Asked for a markdown file, render it
            Some("md") => render_as_markdown(source.clone(), root),
            // Not a markdown file, but it exists, return it raw
            _ => Ok(RenderOutput::RawFile(source))
        }
    } else {
        // Asked for something which doesn't exist:
        Ok(RenderOutput::NoOutput)
    }
}

// If any element of the path starts with an underscore, we want to skip rendering it.
// In addition, if a cheeky person has put .. in the path, just skip it (which will trigger a 404 from the dev server)
fn skipped_path(source: PathBuf) -> bool {
    source.iter().any(|el| {
        match el.to_str() {
            Some("..") => true,
            Some(s) if s.starts_with("_") => true,
            _ => false
        }
    })
}

// This gets called by `render` if the source path extension is md
fn render_as_markdown(source: PathBuf, root: &Path) -> Result<RenderOutput, RenderError> {
    let absolute_source = root.join(source.clone());
    let contents = fs::read_to_string(absolute_source.clone()).map_err(|e| RenderError::FileRead(e, source.clone()))?;
    let options = markdown_options();
    let html = markdown::to_html_with_options(contents.as_str(), &options).map_err(|e| RenderError::MarkdownParse(e, source.clone()))?;
    let ast = markdown::to_mdast(contents.as_str(), &options.parse).map_err(|e| RenderError::MarkdownParse(e, source.clone()))?;

    if let Some(frontmatter) = find_frontmatter(ast, source.clone())? {
        frontmatter.apply_layout(html, source, root)
    } else {
        Ok(RenderOutput::Rendered(source.with_extension("html"), html))
    }
}

// Construct the Markdown options we'll render with
fn markdown_options() -> Options {
    markdown::Options {
        parse: ParseOptions {
            constructs: Constructs {
                frontmatter: true,
                gfm_table: true,
                ..Default::default()
            },
            ..Default::default()
        },
        ..Default::default()
    }
}

// Look for and try to parse toml frontmatter
fn find_frontmatter(node: Node, source: PathBuf) -> Result<Option<Frontmatter>, RenderError> {
    if let Some(children) = node.children() {
        for child in children.iter() {
            if let Node::Toml(toml_str) = child {
                let fmatter: Frontmatter = toml::from_str(toml_str.value.as_str()).map_err(|e| RenderError::FrontmatterParse(e, source))?;
                return Ok(Some(fmatter))
            }
        }
        Ok(None)
    } else { Ok(None) }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn render_file(path: impl Into<PathBuf>) -> RenderOutput {
        match server_render(path.into(), Path::new("./testdata")) {
            Ok(ro) => ro,
            Err(e) => {
                println!("{}", e);
                panic!();
            }
        }
    }

    #[test]
    fn test_rendering_markdown() {
        let contents = render_file("index.html");
        assert!(matches!(contents, RenderOutput::Rendered(_, _)));
        let RenderOutput::Rendered(filename, contents) = contents else { unreachable!() };
        assert_eq!(filename.to_str(), Some("index.html")); // Goes to the right filename
        assert!(contents.starts_with("<!DOCTYPE html>")); // Uses the layout
        assert!(contents.matches("Pest Toast").next().is_some()); // Replaces in the title
        assert!(contents.matches("<code class=\"language-rust\">").next().is_some()); // Renders the code snippet
        assert!(contents.matches("<table>").next().is_some()); // Renders the table
    }

    #[test]
    fn test_no_layout() {
        let contents = render_file("nolayout.html");
        assert!(matches!(contents, RenderOutput::Rendered(_, _))); // Normal output
        let RenderOutput::Rendered(_, contents) = contents else { unreachable!() };
        assert!(contents.starts_with("<p>This file has no layout")) // Doesn't use any layout, just the contents
    }

    #[test]
    fn test_no_output() {
        let contents = render_file("_skipped.html");
        assert!(matches!(contents, RenderOutput::NoOutput)); // This begins with an underscore so it's skipped

        let contents = render_file("_layouts/post.html");
        assert!(matches!(contents, RenderOutput::NoOutput)); // This is in a dir that begins with an underscore
    }

    #[test]
    fn test_hidden() {
        let contents = render_file("hidden.html");
        assert!(matches!(contents, RenderOutput::Hidden(_, _))); // The dev server should return it but it shouldn't produce a file
        let RenderOutput::Hidden(_, contents) = contents else { unreachable!() };
        assert!(contents.starts_with("<!DOCTYPE html>")); // Uses the layout
        assert!(contents.matches("This file should render to hidden").next().is_some());

        let contents = render_file("not_hidden.html");
        assert!(matches!(contents, RenderOutput::Rendered(_, _))); // If we don't specify, it's published by default
    }

    #[test]
    fn test_raw() {
        let contents = render_file("raw.txt");
        assert_eq!(contents, RenderOutput::RawFile(PathBuf::from("raw.txt")));
    }

    #[test]
    fn test_dotdot() {
        assert!(matches!(render_file("../src/main.rs"), RenderOutput::NoOutput));
    }

    #[test]
    fn test_dir() {
        assert!(matches!(render_file("dir"), RenderOutput::Dir(_)));
    }

    #[test]
    fn test_ask_for_html() {
        let contents = render_file("index.html");
        assert!(matches!(contents, RenderOutput::Rendered(_, _))) // We asked for the html file which doesn't exist but the md does
    }
}