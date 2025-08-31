use std::fs;
use std::io::Error;
use std::path::PathBuf;
use markdown::message::Message;
use markdown::{Constructs, Options, ParseOptions};
use markdown::mdast::Node;
use serde::Deserialize;
use thiserror::Error;

/// The things we might return from trying to render a file
#[derive(Clone, PartialEq, Debug)]
pub enum RenderOutput {
    /// A string which had been rendered from some source file or script
    Rendered(PathBuf, String),
    /// Hidden output should be returned by the test server but not rendered to a file
    Hidden(PathBuf, String),
    /// The raw contents of the path
    RawFile(PathBuf),
    /// No contents; this file should not be output / test server should return 404
    NoOutput,
    /// This is a directory; the test server won't return anything but we need to mkdir it
    Dir(PathBuf)
}

#[derive(Deserialize)]
pub struct Frontmatter {
    layout: Option<String>,
    title: Option<String>,
    published: bool
}

impl Frontmatter {
    fn apply_layout(self, content: String, filename: PathBuf, root: PathBuf) -> Result<RenderOutput, RenderError> {
        let title = self.title.unwrap_or(String::new());
        let wrapped = if let Some(layout) = self.layout {
            let absolute_layout = root.join(layout);
            let layout = fs::read_to_string(absolute_layout.clone()).map_err(|e| RenderError::FileReadError(e, absolute_layout))?;
            layout.replace("$title", title.as_str()).replace("$content", content.as_str())
        } else {
            content
        };
        if self.published {
            Ok(RenderOutput::Rendered(filename.with_extension("html"), wrapped))
        } else {
            Ok(RenderOutput::Hidden(filename.with_extension("html"), wrapped))
        }
    }
}

#[derive(Error, Debug)]
pub enum RenderError {
    #[error("Error reading {1}: {0}")]
    FileReadError(Error, PathBuf),
    #[error("Error parsing Markdown {1}: {0}")]
    MarkdownParseError(Message, PathBuf),
    #[error("Error parsing frontmatter in {1}: {0}")]
    FrontmatterParseError(toml::de::Error, PathBuf)
}

/// Take a source file path (relative to the root) and the root path, and return a RenderOutput for it
pub fn render(source: PathBuf, root: PathBuf) -> Result<RenderOutput, RenderError> {
    if skipped_path(source.clone()) {
        return Ok(RenderOutput::NoOutput)
    }
    if root.join(source.clone()).is_dir() {
        return Ok(RenderOutput::Dir(source))
    }
    match source.extension().map(|o| o.to_str().unwrap()) {
        Some("md") => render_as_markdown(source.clone(), root),
        _ => Ok(RenderOutput::RawFile(source))
    }
}

// If any element of the path starts with an underscore, we want to skip rendering it.
// In addition, if a cheeky person has put .. in the path, just skip it (which will trigger a 404 from the dev server)
fn skipped_path(source: PathBuf) -> bool {
    source.into_iter().any(|el| {
        match el.to_str() {
            Some("..") => true,
            Some(s) if s.starts_with("_") => true,
            _ => false
        }
    })
}

// This gets called by `render` if the source path extension is md
fn render_as_markdown(source: PathBuf, root: PathBuf) -> Result<RenderOutput, RenderError> {
    let absolute_source = root.join(source.clone());
    let contents = fs::read_to_string(absolute_source.clone()).map_err(|e| RenderError::FileReadError(e, source.clone()))?;
    let options = markdown_options();
    let html = markdown::to_html_with_options(contents.as_str(), &options).map_err(|e| RenderError::MarkdownParseError(e, source.clone()))?;
    let ast = markdown::to_mdast(contents.as_str(), &options.parse).map_err(|e| RenderError::MarkdownParseError(e, source.clone()))?;

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
        for child in children.into_iter() {
            if let Node::Toml(toml_str) = child {
                let fmatter: Frontmatter = toml::from_str(toml_str.value.as_str()).map_err(|e| RenderError::FrontmatterParseError(e, source))?;
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
        match render(path.into(), "./testdata".into()) {
            Ok(ro) => ro,
            Err(e) => {
                println!("{}", e);
                panic!();
            }
        }
    }

    #[test]
    fn test_rendering_markdown() {
        let contents = render_file("index.md");
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
        let contents = render_file("nolayout.md");
        assert!(matches!(contents, RenderOutput::Rendered(_, _))); // Normal output
        let RenderOutput::Rendered(_, contents) = contents else { unreachable!() };
        assert!(contents.starts_with("<p>This file has no layout")) // Doesn't use any layout, just the contents
    }

    #[test]
    fn test_no_output() {
        let contents = render_file("_skipped.md");
        assert!(matches!(contents, RenderOutput::NoOutput)); // This begins with an underscore so it's skipped

        let contents = render_file("_layouts/post.html");
        assert!(matches!(contents, RenderOutput::NoOutput)); // This is in a dir that begins with an underscore
    }

    #[test]
    fn test_hidden() {
        let contents = render_file("hidden.md");
        assert!(matches!(contents, RenderOutput::Hidden(_, _))); // The dev server should return it but it shouldn't produce a file
        let RenderOutput::Hidden(_, contents) = contents else { unreachable!() };
        assert!(contents.starts_with("<!DOCTYPE html>")); // Uses the layout
        assert!(contents.matches("This file should render to hidden").next().is_some());
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
}