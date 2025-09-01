use std::path::PathBuf;
use std::sync::Arc;
use axum::extract::State;
use axum::response::{IntoResponse, Response};
use axum::Router;
use axum::routing::get;
use crate::renderer::{default_path, render, RenderOutput};

pub async fn start_server(root: PathBuf) {
    let state = Arc::new(root);
    let app = Router::new()
        .route("/", get(handler))
        .with_state(state);

    // run our app with hyper, listening globally on port 3000
    let listener = tokio::net::TcpListener::bind("0.0.0.0:3000").await.unwrap();
    axum::serve(listener, app).await.unwrap();
}

async fn handler(
    State(root): State<Arc<PathBuf>>,
) -> impl IntoResponse {
    let path = default_path(&root);
    serve_path(String::from(path.to_str().unwrap()), root)
}

fn serve_path(path: String, root: impl AsRef<PathBuf>) -> impl IntoResponse {
    let render = render(path.into(), root.as_ref());
    match render {
        Ok(RenderOutput::Rendered(file, content)) |
        Ok(RenderOutput::Hidden(file, content)) => {
            Response::builder()
                .status(200)
                .header("Content-Type", "text/html")
                .body(content).unwrap()
        }
        Ok(RenderOutput::RawFile(file)) => { todo!() }
        Ok(RenderOutput::NoOutput) |
        Ok(RenderOutput::Dir(_)) => {
            Response::builder()
                .status(404)
                .body(String::from("Not Found")).unwrap()
        }
        Err(err) => {
            Response::builder()
                .status(500)
                .body(format!("{}", err)).unwrap()
        }
    }
}

