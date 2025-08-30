use std::{default, fs};
use std::io::Error;
use std::path::PathBuf;
use eframe::egui::TextBuffer;
use markdown::message::Message;
use markdown::{CompileOptions, Constructs, Options, ParseOptions};
use markdown::mdast::Node;
use serde::Deserialize;
use thiserror::Error;

pub struct RenderOutput {
    pub filename: PathBuf,
    pub contents: String
}

#[derive(Deserialize)]
pub struct Frontmatter {
    layout: Option<String>,
    title: Option<String>
}

impl Frontmatter {
    fn apply_layout(self, content: String, root: PathBuf) -> Result<String, RenderError> {
        let title = self.title.unwrap_or(String::new());
        if let Some(layout) = self.layout {
            let absolute_layout = root.join(layout);
            let layout = fs::read_to_string(absolute_layout.clone()).map_err(|e| RenderError::FileReadError(e, absolute_layout))?;
            Ok(layout.replace("$title", title.as_str()).replace("$content", content.as_str()))
        } else {
            Ok(content)
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

pub fn render(source: PathBuf, root: PathBuf) -> Result<RenderOutput, RenderError> {
    let contents = match source.extension().map(|o| o.to_str().unwrap()) {
        Some("md") => render_as_markdown(source.clone(), root),
        _ => todo!()
    }?;
    let filename = source.with_extension("html");
    Ok(RenderOutput { filename, contents })
}

fn render_as_markdown(source: PathBuf, root: PathBuf) -> Result<String, RenderError> {
    let absolute_source = root.join(source.clone());
    let contents = fs::read_to_string(absolute_source.clone()).map_err(|e| RenderError::FileReadError(e, source.clone()))?;
    let options = markdown_options();
    let html = markdown::to_html_with_options(contents.as_str(), &options).map_err(|e| RenderError::MarkdownParseError(e, source.clone()))?;
    let ast = markdown::to_mdast(contents.as_str(), &options.parse).map_err(|e| RenderError::MarkdownParseError(e, source.clone()))?;

    if let Some(frontmatter) = find_frontmatter(ast, source.clone())? {
        frontmatter.apply_layout(html, root)
    } else {
        Ok(html)
    }
}

// Construct the
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

    #[test]
    fn test_rendering_markdown() {
        match render("index.md".into(), "./testdata".into()) {
            Err(e) => println!("{}", e),
            Ok(ro) => {
                println!("File: {}", ro.filename.to_str().unwrap());
                println!("Contents:\n\n{}", ro.contents);
            }
        }
    }
}