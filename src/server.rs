use std::convert::Infallible;
use std::net::SocketAddr;
use std::path::PathBuf;
use std::pin::Pin;
use http_body_util::Full;
use hyper::{Request, Response};
use hyper::body::{Bytes, Incoming};
use hyper::server::conn::http1;
use hyper::service::{service_fn, Service};
use hyper_util::rt::TokioIo;
use tokio::net::TcpListener;
use crate::renderer::{default_path, render, RenderError, RenderOutput};

// pub async fn hello(root: &PathBuf, req: Request<hyper::body::Incoming>) -> Result<Response<Full<Bytes>>, Infallible> {
//     Ok(Response::new(Full::new(Bytes::from("Hello, World!"))))
// }

#[derive(Clone)]
struct FleenServer {
    root: PathBuf
}

impl Service<Request<Incoming>> for FleenServer {
    type Response = Response<Full<Bytes>>;
    type Error = hyper::Error;
    type Future = Pin<Box<dyn Future<Output = Result<Self::Response, Self::Error>> + Send>>;

    fn call(&self, req: Request<Incoming>) -> Self::Future {
        fn mk_response(s: String) -> Result<Response<Full<Bytes>>, hyper::Error> {
            Ok(Response::builder().body(Full::new(Bytes::from(s))).unwrap())
        }

        let path: PathBuf = if req.uri().path() == "/" || req.uri().path() == "" {
            default_path(&self.root)
        } else {
            req.uri().path().into()
        };

        let res = match render(path, self.root.clone()) {
            Ok(_) => mk_response(String::from("banana")),
            Err(e) => mk_response(format!("{}", e))
        };

        Box::pin(async { res })
    }
}

pub async fn start_server(root: PathBuf) -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
    let addr = SocketAddr::from(([127, 0, 0, 1], 3000));
    let listener = TcpListener::bind(addr).await?;
    let server = FleenServer { root };

    loop {
        let (stream, _) = listener.accept().await?;
        let io = TokioIo::new(stream);
        let server = server.clone();

        tokio::task::spawn(async move {
            if let Err(err) = http1::Builder::new()
                .serve_connection(io, server)
                .await
            {
                eprintln!("Error serving connection: {:?}", err);
            }
        });
    }
}