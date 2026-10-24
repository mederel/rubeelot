//! The request body limit: every legitimate request is a tiny JSON document.

use tower_http::limit::RequestBodyLimitLayer;

/// Refuses with 413 a body that declares, or turns out while it is read, to be over `max_bytes`.
pub fn layer(max_bytes: usize) -> RequestBodyLimitLayer {
    RequestBodyLimitLayer::new(max_bytes)
}

#[cfg(test)]
mod tests {
    use axum::Router;
    use axum::body::{Body, Bytes};
    use axum::http::{Request, StatusCode};
    use axum::routing::post;
    use tower::ServiceExt;

    use super::*;

    fn app() -> Router {
        Router::new()
            .route(
                "/",
                post(|body: Bytes| async move { body.len().to_string() }),
            )
            .layer(layer(100))
    }

    /// A body sent in chunks, declaring no length.
    fn streamed(len: usize) -> Body {
        let chunks = vec![
            Ok::<_, std::io::Error>(Bytes::from(vec![b'a'; len / 2])),
            Ok(Bytes::from(vec![b'a'; len - len / 2])),
        ];
        Body::from_stream(futures_stream(chunks))
    }

    fn futures_stream(
        chunks: Vec<Result<Bytes, std::io::Error>>,
    ) -> impl tokio_stream::Stream<Item = Result<Bytes, std::io::Error>> {
        tokio_stream::iter(chunks)
    }

    #[tokio::test]
    async fn reads_a_streamed_body_up_to_the_limit() {
        let response = app()
            .oneshot(Request::post("/").body(streamed(100)).unwrap())
            .await
            .unwrap();
        assert_eq!(response.status(), StatusCode::OK);
    }

    #[tokio::test]
    async fn refuses_a_streamed_body_once_it_is_too_large() {
        let response = app()
            .oneshot(Request::post("/").body(streamed(101)).unwrap())
            .await
            .unwrap();
        assert_eq!(response.status(), StatusCode::PAYLOAD_TOO_LARGE);
    }
}
