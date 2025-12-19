#![allow(async_fn_in_trait)]

use jsonrpsee_types::{ErrorCode, ErrorObjectOwned, Request, Response, ResponsePayload};
use tower::{ServiceBuilder, service_fn};
use tower_json_rpc_derive::rpc;

#[rpc(server, namespace = "say")]
pub trait Say {
    #[method(name = "hello")]
    async fn say_hello(&self, name: String) -> Result<String, ErrorObjectOwned>;
}

struct SayImpl;

impl Say for SayImpl {
    async fn say_hello(&self, name: String) -> Result<String, ErrorObjectOwned> {
        Ok(format!("Hello, {name}!"))
    }
}

fn main() {
    let handler = SayImpl;
    let fallback = service_fn(|req: Request<'static>| async move {
        Ok::<_, std::convert::Infallible>(Response::new(
            ResponsePayload::<serde_json::Value>::error(ErrorObjectOwned::from(
                ErrorCode::MethodNotFound,
            )),
            req.id,
        ))
    });

    let app = ServiceBuilder::new()
        .layer(SayServerLayer::new(handler))
        .service(fallback);

    let _ = app;
}
