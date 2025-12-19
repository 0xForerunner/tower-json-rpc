#![allow(async_fn_in_trait)]

use http::header;
use http_body_util::Full;
use hyper::body::Bytes;
use jsonrpsee_types::{ErrorCode, ErrorObjectOwned, Id, Request, Response, ResponsePayload};
use tower::{Layer, ServiceBuilder, ServiceExt, service_fn};
use tower_json_rpc::server::JsonRpcLayer;
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

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    let handler = SayImpl;
    let json_rpc =
        SayServerLayer::new(handler).layer(service_fn(|req: Request<'static>| async move {
            Ok::<_, std::convert::Infallible>(Response::new(
                ResponsePayload::<serde_json::Value>::error(ErrorObjectOwned::from(
                    ErrorCode::MethodNotFound,
                )),
                req.id,
            ))
        }));

    let app = ServiceBuilder::new().layer(JsonRpcLayer).service(json_rpc);

    let params = serde_json::value::to_raw_value(&vec![serde_json::json!("Ada")])?;
    let json_request = Request::owned("say_hello".to_string(), Some(params), Id::Number(1));
    let body = serde_json::to_vec(&json_request)?;

    let http_request: http::Request<Full<Bytes>> = http::Request::builder()
        .method("POST")
        .header(header::CONTENT_TYPE, "application/json")
        .body(Full::new(Bytes::from(body)))?;

    let http_response = app.oneshot(http_request).await?;
    let _status = http_response.status();

    Ok(())
}
