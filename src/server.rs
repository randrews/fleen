use std::fs;
use std::path::PathBuf;
use axum::body::Body;
use axum::extract::State;
use axum::http::Uri;
use axum::response::{IntoResponse, Response};
use axum::Router;
use axum::routing::get;
use crate::renderer::{default_path, render, RenderOutput};

pub async fn start_server(root: PathBuf, port: u32) {
    let app = Router::new()
        .route("/", get(|State(root): State<PathBuf>| async move {
            // We need a separate route for the default path because {*p} must match at least one thing
            let path = default_path(&root);
            serve_path(String::from(path.to_str().unwrap()), root.clone())
        }))
        .route("/{*path}", get(|State(root): State<PathBuf>, uri: Uri| async move {
            serve_path(String::from(uri.path()), root.clone())
        }))
        .with_state(root);

    let listener = tokio::net::TcpListener::bind(format!("0.0.0.0:{}", port)).await.unwrap();
    axum::serve(listener, app).await.unwrap();
}

fn serve_path(path: String, root: PathBuf) -> impl IntoResponse {
    let path = path.strip_prefix("/").unwrap_or(path.as_str());
    let render = render(path.into(), root.as_ref());

    match render {
        Ok(RenderOutput::Rendered(_, content)) |
        Ok(RenderOutput::Hidden(_, content)) => {
            Response::builder()
                .status(200)
                .body(Body::from(content)).unwrap()
        }
        Ok(RenderOutput::RawFile(file)) => {
            Response::builder()
                .status(200)
                .body(Body::from(fs::read(file).unwrap_or(vec![]))).unwrap()
        }
        Ok(RenderOutput::NoOutput) |
        Ok(RenderOutput::Dir(_)) => {
            // TODO a nicer 404 page?
            Response::builder()
                .status(404)
                .body(Body::from(include_str!("../templates/404.html"))).unwrap()
        }
        Err(err) => {
            Response::builder()
                .status(500)
                .body(Body::from(format!("{}", err))).unwrap()
        }
    }
}

