use std::path::PathBuf;

use axum::response::Response as AxumResponse;
use axum::routing::get;
use axum::Router;
use axum::{
    body::Body,
    extract::State,
    http::{Request, Response, StatusCode, Uri},
    response::IntoResponse,
};
use tower::ServiceExt;
use tower_http::services::ServeDir;

#[derive(Clone, Default)]
pub struct Options {
    pub site_root: String,
    pub site_addr: String,
}

pub fn shell() -> String {
    format!(r#"<!DOCTYPE html>
<html lang="en">
    <head>
        <meta charset="utf-8"/>
        <meta name="viewport" content="width=device-width, initial-scale=1"/>
        <link rel="stylesheet" href="/pkg/pizza-ui.css"/>
    </head>
    <body>
        <div id="main"></div>
        <script type="module">
            import init from '/pkg/pizza-ui.js';
            init();
        </script>
    </body>
</html>"#)
}

#[tokio::main]
async fn main() {
    let site_root = "target/site";
    let site_addr = "127.0.0.1:3000";
    let options = Options {
        site_root: site_root.to_string(),
        site_addr: site_addr.to_string(),
    };

    let site_root_path = PathBuf::from(site_root);
    if !site_root_path.exists() {
        tokio::fs::create_dir_all(&site_root_path).await.unwrap();
    }
    let pkg_path = site_root_path.join("pkg");
    if !pkg_path.exists() {
        tokio::fs::create_dir_all(&pkg_path).await.unwrap();
    }

    let index_path = site_root_path.join("index.html");

    tokio::fs::write(index_path, shell())
        .await
        .expect("could not write index.html");

    let app = Router::new()
        .route("/", get(file_and_error_handler))
        .fallback(file_and_error_handler)
        .with_state(options);

    println!("listening on http://{}", &site_addr);
    let listener = tokio::net::TcpListener::bind(&site_addr).await.unwrap();
    axum::serve(listener, app.into_make_service())
        .await
        .unwrap();
}

pub async fn file_and_error_handler(
    uri: Uri,
    State(options): State<Options>,
) -> AxumResponse {
    let root = options.site_root.clone();
    match get_static_file(uri.clone(), &root).await {
        Ok(res) => res.into_response(),
        Err(_) => get_static_file(Uri::from_static("/index.html"), &root)
            .await
            .expect("could not find index.html")
            .into_response(),
    }
}

async fn get_static_file(uri: Uri, root: &str) -> Result<Response<Body>, (StatusCode, String)> {
    let req = Request::builder()
        .uri(uri.clone())
        .body(Body::empty())
        .unwrap();
    match ServeDir::new(root).oneshot(req).await {
        Ok(res) => Ok(res.map(Body::new)),
        Err(err) => Err((
            StatusCode::INTERNAL_SERVER_ERROR,
            format!("Something went wrong: {err}"),
        )),
    }
}
