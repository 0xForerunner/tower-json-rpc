use axum::{Router, http::StatusCode, routing::post_service};
use jsonrpsee_types::{ErrorCode, ErrorObjectOwned, Request, Response, ResponsePayload};
use tower::{ServiceBuilder, service_fn};
use tower_http::trace::TraceLayer;
use tower_json_rpc::{error::JsonRpcError, server::JsonRpcLayer};
use tower_json_rpc_derive::rpc;

#[rpc(server, namespace = "say")]
pub trait Say {
    #[method(name = "hello")]
    fn say_hello(&self, name: String) -> Result<String, ErrorObjectOwned>;
}

#[derive(Clone, Debug)]
struct SayImpl;

impl Say for SayImpl {
    fn say_hello(&self, name: String) -> Result<String, ErrorObjectOwned> {
        Ok(format!("Hello, {name}!"))
    }
}

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    let handler = SayImpl;
    let fallback = service_fn(|req: Request<'static>| async move {
        Ok::<_, std::convert::Infallible>(Response::new(
            ResponsePayload::error(ErrorObjectOwned::from(ErrorCode::MethodNotFound)),
            req.id,
        ))
    });

    let rpc = ServiceBuilder::new()
        .layer(TraceLayer::new_for_http())
        .layer(SayServerLayer::new(handler))
        .service(fallback);

    let app = Router::new().route(
        "/rpc",
        post_service(rpc).handle_error(|err: JsonRpcError| async move {
            (StatusCode::INTERNAL_SERVER_ERROR, err.to_string())
        }),
    );

    let listener = tokio::net::TcpListener::bind("127.0.0.1:3000").await?;
    axum::serve(listener, app).await?;

    Ok(())
}
