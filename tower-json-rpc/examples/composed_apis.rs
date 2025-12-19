#![allow(async_fn_in_trait)]

use jsonrpsee_types::{ErrorCode, ErrorObjectOwned, Request, Response, ResponsePayload};
use tower::{Layer, Service, ServiceBuilder, service_fn};
use tower_json_rpc::server::JsonRpcLayer;
use tower_json_rpc_derive::rpc;

#[rpc(server, namespace = "say")]
pub trait Say {
    #[method(name = "hello")]
    async fn say_hello(&self, name: String) -> Result<String, ErrorObjectOwned>;
}

#[rpc(server, namespace = "admin")]
pub trait Admin {
    #[method(name = "ping")]
    async fn ping(&self) -> Result<String, ErrorObjectOwned>;
}

struct SayImpl;
struct AdminImpl;

impl Say for SayImpl {
    async fn say_hello(&self, name: String) -> Result<String, ErrorObjectOwned> {
        Ok(format!("Hello, {name}!"))
    }
}

impl Admin for AdminImpl {
    async fn ping(&self) -> Result<String, ErrorObjectOwned> {
        Ok("pong".to_string())
    }
}

fn main() {
    let say = SayServerLayer::new(SayImpl).layer(service_fn(|req: Request<'static>| async move {
        Ok::<_, std::convert::Infallible>(Response::new(
            ResponsePayload::<serde_json::Value>::error(ErrorObjectOwned::from(
                ErrorCode::MethodNotFound,
            )),
            req.id,
        ))
    }));

    let admin =
        AdminServerLayer::new(AdminImpl).layer(service_fn(|req: Request<'static>| async move {
            Ok::<_, std::convert::Infallible>(Response::new(
                ResponsePayload::<serde_json::Value>::error(ErrorObjectOwned::from(
                    ErrorCode::MethodNotFound,
                )),
                req.id,
            ))
        }));

    let router = service_fn(move |req: Request<'static>| {
        let mut say = say.clone();
        let mut admin = admin.clone();
        async move {
            match req.method.as_ref() {
                m if m.starts_with("say_") => say.call(req).await,
                m if m.starts_with("admin_") => admin.call(req).await,
                _ => Ok::<_, std::convert::Infallible>(Response::new(
                    ResponsePayload::<serde_json::Value>::error(ErrorObjectOwned::from(
                        ErrorCode::MethodNotFound,
                    )),
                    req.id,
                )),
            }
        }
    });

    let app = ServiceBuilder::new().layer(JsonRpcLayer).service(router);
    let _ = app;
}
