use std::task::{Context, Poll};

use axum::Router;
use jsonrpsee_types::{ErrorObjectOwned, Request, Response};
use serde_json::Value;
use tower::{Service, ServiceBuilder, service_fn};
use tower_json_rpc::{
    error::JsonRpcError,
    server::{BoxFuture, JsonRpcLayer, ServerRequest},
};
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

impl<S> tower::Layer<S> for SayImpl {
    type Service = SayServerService2<S>;
    fn layer(&self, inner: S) -> Self::Service {
        SayServerService2 {
            inner,
            layer: self.clone(),
        }
    }
}

#[derive(Debug, Clone)]
pub struct SayServerService2<S> {
    inner: S,
    layer: SayImpl,
}

impl<S, Req> Service<Req> for SayServerService2<S>
where
    Req: ServerRequest,
    S: Service<Request<'static>, Response = Response<'static, Value>> + Clone + Send + 'static,
    S::Future: Send + 'static,
    S::Error: Into<JsonRpcError> + Send + 'static,
{
    type Response = Req::Response;
    type Error = JsonRpcError;
    type Future = BoxFuture<Result<Self::Response, Self::Error>>;

    fn poll_ready(&mut self, cx: &mut Context<'_>) -> Poll<Result<(), Self::Error>> {
        self.inner.poll_ready(cx).map_err(Into::into)
    }

    fn call(&mut self, request: Req) -> Self::Future {
        use futures_util::future::FutureExt;

        let mut service = self.inner.clone();
        let fut = request.into_json_rpc_request();

        todo!()
        // Box::pin(fut.then(move |result| match result {
        //     Ok(json_rpc_request) => {
        //         let service_fut = service.call(json_rpc_request);
        //         Box::pin(
        //             service_fut.then(move |service_result| match service_result {
        //                 Ok(response) => Req::Response::from_json_rpc_response(response),
        //                 Err(e) => Box::pin(async move { Err(e.into()) }),
        //             }),
        //         ) as BoxFuture<Result<Req::Response, JsonRpcError>>
        //     }
        //     Err(e) => Box::pin(async move { Err(e) }),
        // }))
    }
}

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    // let app = Router::new()
    //     .layer(SayServerLayer::new(SayImpl))
    //     .layer(JsonRpcLayer);
    //
    // let listener = tokio::net::TcpListener::bind("127.0.0.1:3000").await?;
    // axum::serve(listener, app).await?;
    //
    // Ok(())
    let jsonrpc = service_fn(|req: Request<'static>| async move {
        let id = req.id.clone();
        let method = req.method.to_string();

        Ok::<_, JsonRpcError>(Response::new(
            jsonrpsee_types::ResponsePayload::success(serde_json::json!({ "method": method })),
            id,
        ))
    });

    let svc = SayServerService2 {
        inner: (),
        layer: SayImpl,
    };

    // 2) Wrap it with your layer: now it should accept HTTP requests
    let http_svc = ServiceBuilder::new()
        .map_err(|_e| todo!())
        .layer(JsonRpcLayer)
        .layer(SayImpl)
        // .layer(SayServerLayer::new(SayImpl));
        .service(jsonrpc);

    let app = Router::new().route_service("/", http_svc);

    let listener = tokio::net::TcpListener::bind("127.0.0.1:3000").await?;
    axum::serve(listener, app).await?;
    Ok(())
}
