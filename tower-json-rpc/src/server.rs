use futures_util::FutureExt;
use jsonrpsee_types::{Request, Response};
use serde_json::Value;
// From reth_rpc_layer
use std::{
    pin::Pin,
    task::{Context, Poll},
};
use tower::{Layer, Service};

use crate::error::JsonRpcError;

pub trait ServerRequest {
    type Response: ServerResponse;

    async fn into_json_rpc_request<'a>(self) -> Result<Request<'a>, JsonRpcError>;
}

pub trait ServerResponse: Sized {
    async fn from_json_rpc_response<'a, P: Clone>(
        response: Response<'a, P>,
    ) -> Result<Self, JsonRpcError>;
}

/// A layer that maps http requests to JSON-RPC requests.
#[derive(Clone, Debug)]
pub struct JsonRpcLayer;

impl<S> Layer<S> for JsonRpcLayer {
    type Service = JsonRpcServer<S>;

    fn layer(&self, inner: S) -> Self::Service {
        todo!();
    }
}

/// Automatically authenticates every client request with the given `secret`.
#[derive(Debug, Clone)]
pub struct JsonRpcServer<S> {
    inner: S,
}

impl<'req, 'res, S, Req> Service<Req> for JsonRpcServer<S>
where
    Req: ServerRequest + Send + 'static,
    S: Service<Request<'req>, Response = Response<'res, Value>> + Clone + Send + 'static,
    S::Future: Send + 'static,
    S::Error: Into<JsonRpcError>,
{
    type Response = <Req as ServerRequest>::Response;
    type Error = JsonRpcError;
    type Future =
        Pin<Box<dyn Future<Output = Result<Self::Response, Self::Error>> + Send + 'static>>;

    fn poll_ready(&mut self, cx: &mut Context<'_>) -> Poll<Result<(), Self::Error>> {
        self.inner.poll_ready(cx).map_err(Into::into)
    }

    fn call(&mut self, request: Req) -> Self::Future {
        // See https://github.com/tower-rs/tower/blob/abb375d08cf0ba34c1fe76f66f1aba3dc4341013/tower-service/src/lib.rs#L276
        // for an explanation of this pattern
        // let mut service = self.clone();
        // service.inner = std::mem::replace(&mut self.inner, service.inner);
        //
        // async move {
        //     let response = service
        //         .inner
        //         .call(request.into_json_rpc_request().await?)
        //         .await
        //         .map_err(Into::into)?;
        //     <Req as ServerRequest>::Response::from_json_rpc_response(response).await
        //     // response.try_into().map_err(Into::into)
        // }
        // .boxed()
        todo!()
    }
}

#[cfg(test)]
mod tests {
    use hyper::server::conn::http1;
    use hyper_util::service::TowerToHyperService;

    use std::{convert::Infallible, net::SocketAddr};
    use tokio::net::TcpListener;

    use crate::{request::JsonRpcRequest, response::JsonRpcResponse, server::JsonRpcLayer};

    async fn handle_json_rpc<'a>(_req: JsonRpcRequest<'a>) -> Result<JsonRpcResponse, Infallible> {
        todo!();
    }

    #[tokio::test]
    async fn test_build_service() {
        let addr: SocketAddr = ([127, 0, 0, 1], 3000).into();
        let app = tower::ServiceBuilder::new()
            .layer(JsonRpcLayer)
            .service_fn(handle_json_rpc);

        let hyper_svc = TowerToHyperService::new(app);

        let listener = TcpListener::bind(addr).await.unwrap();

        loop {
            let (stream, _) = listener.accept().await.unwrap();

            let io = hyper_util::rt::TokioIo::new(stream);
            let service_clone = hyper_svc.clone();

            tokio::task::spawn(async move {
                http1::Builder::new()
                    .serve_connection(io, service_clone)
                    .await
            });
        }
    }
}
