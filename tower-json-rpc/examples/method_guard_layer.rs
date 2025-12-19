#![allow(async_fn_in_trait)]

use jsonrpsee_types::{ErrorCode, ErrorObjectOwned, Request, Response, ResponsePayload};
use std::{
    future::Future,
    pin::Pin,
    task::{Context, Poll},
};
use tower::{Layer, Service, ServiceBuilder, service_fn};
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

#[derive(Clone)]
struct DenyHelloLayer;

#[derive(Clone)]
struct DenyHello<S> {
    inner: S,
}

impl<S> Layer<S> for DenyHelloLayer {
    type Service = DenyHello<S>;

    fn layer(&self, inner: S) -> Self::Service {
        DenyHello { inner }
    }
}

impl<S> Service<Request<'static>> for DenyHello<S>
where
    S: Service<Request<'static>, Response = Response<'static, serde_json::Value>> + Clone + 'static,
    S::Future: 'static,
{
    type Response = Response<'static, serde_json::Value>;
    type Error = S::Error;
    type Future = Pin<Box<dyn Future<Output = Result<Self::Response, Self::Error>> + 'static>>;

    fn poll_ready(&mut self, cx: &mut Context<'_>) -> Poll<Result<(), Self::Error>> {
        self.inner.poll_ready(cx)
    }

    fn call(&mut self, req: Request<'static>) -> Self::Future {
        let mut inner = self.inner.clone();
        Box::pin(async move {
            if let Ok(SayRequest::Hello { .. }) = SayRequest::try_from(req.clone()) {
                return Ok(Response::new(
                    ResponsePayload::<serde_json::Value>::error(ErrorObjectOwned::owned(
                        ErrorCode::InvalidRequest.code(),
                        ErrorCode::InvalidRequest.message(),
                        Some("say_hello is disabled"),
                    )),
                    req.id,
                ));
            }

            inner.call(req).await
        })
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
        .layer(DenyHelloLayer)
        .layer(SayServerLayer::new(handler))
        .service(fallback);

    let _ = app;
}
