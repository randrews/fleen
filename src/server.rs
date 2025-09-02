use std::fs;
use std::path::PathBuf;
use std::sync::Arc;
use axum::body::Body;
use axum::extract::State;
use axum::http::Uri;
use axum::response::{IntoResponse, Response};
use axum::Router;
use axum::routing::get;
use crate::renderer::{default_path, render, RenderOutput};

pub async fn start_server(root: PathBuf) {
    let state = Arc::new(root);
    let app = Router::new()
        .route("/", get(|State(root): State<Arc<PathBuf>>| async {
            let path = default_path(&root);
            serve_path(String::from(path.to_str().unwrap()), root)
        }))
        .route("/{*path}", get(|State(root): State<Arc<PathBuf>>, uri: Uri| async move {
            serve_path(String::from(uri.path()), root)
        }))
        .with_state(state);

    // run our app with hyper, listening globally on port 3000
    let listener = tokio::net::TcpListener::bind("0.0.0.0:3000").await.unwrap();
    axum::serve(listener, app).await.unwrap();
}

fn serve_path(path: String, root: impl AsRef<PathBuf>) -> impl IntoResponse {
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

