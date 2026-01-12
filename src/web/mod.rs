use rust_embed::RustEmbed;
use axum::{
    response::{IntoResponse, Response},
    http::{StatusCode, Uri, header},
    body::Body,
};

#[derive(RustEmbed)]
#[folder = "web/dist/"]
struct Assets;

pub async fn static_handler(uri: Uri) -> impl IntoResponse {
    let path = uri.path().trim_start_matches('/');

    // Serve index.html for root
    if path.is_empty() || path == "index.html" {
        return serve_index();
    }

    // Try to serve the requested file
    if let Some(content) = Assets::get(path) {
        let mime = mime_guess::from_path(path).first_or_octet_stream();
        return Response::builder()
            .header(header::CONTENT_TYPE, mime.as_ref())
            .body(Body::from(content.data.to_vec()))
            .unwrap()
            .into_response();
    }

    // SPA fallback: serve index.html for non-API routes
    // This allows React Router to handle client-side routing
    if !path.starts_with("api/") {
        return serve_index();
    }

    // 404 for everything else
    (StatusCode::NOT_FOUND, "Not found").into_response()
}

fn serve_index() -> Response {
    if let Some(index) = Assets::get("index.html") {
        Response::builder()
            .header(header::CONTENT_TYPE, "text/html")
            .body(Body::from(index.data.to_vec()))
            .unwrap()
            .into_response()
    } else {
        (StatusCode::INTERNAL_SERVER_ERROR, "index.html not found").into_response()
    }
}
