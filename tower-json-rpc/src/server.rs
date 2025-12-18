use jsonrpsee_types::{Request, Response};
use serde_json::Value;
use std::{
    future::Future,
    pin::Pin,
    task::{Context, Poll},
};
use tower::{Layer, Service};

use crate::error::JsonRpcError;

pub trait ServerRequest: Send + 'static {
    type Response: ServerResponse;

    fn into_json_rpc_request(
        self,
    ) -> Pin<Box<dyn Future<Output = Result<Request<'static>, JsonRpcError>> + Send + 'static>>;
}

pub trait ServerResponse: Sized + Send + 'static {
    fn from_json_rpc_response(
        response: Response<'static, Value>,
    ) -> Pin<Box<dyn Future<Output = Result<Self, JsonRpcError>> + Send + 'static>>;
}

/// A layer that maps http requests to JSON-RPC requests.
#[derive(Clone, Debug)]
pub struct JsonRpcLayer;

impl<S> Layer<S> for JsonRpcLayer {
    type Service = JsonRpcServer<S>;

    fn layer(&self, inner: S) -> Self::Service {
        JsonRpcServer { inner }
    }
}

/// JSON-RPC server that wraps an inner service
#[derive(Debug, Clone)]
pub struct JsonRpcServer<S> {
    inner: S,
}

// Helper type to avoid lifetime issues
type BoxFuture<T> = Pin<Box<dyn Future<Output = T> + Send + 'static>>;

impl<S, Req> Service<Req> for JsonRpcServer<S>
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

        Box::pin(fut.then(move |result| match result {
            Ok(json_rpc_request) => {
                let service_fut = service.call(json_rpc_request);
                Box::pin(
                    service_fut.then(move |service_result| match service_result {
                        Ok(response) => Req::Response::from_json_rpc_response(response),
                        Err(e) => Box::pin(async move { Err(e.into()) }),
                    }),
                ) as BoxFuture<Result<Req::Response, JsonRpcError>>
            }
            Err(e) => Box::pin(async move { Err(e) }),
        }))
    }
}

#[cfg(test)]
mod tests {
    use hyper::server::conn::http1;
    use hyper_util::service::TowerToHyperService;
    use jsonrpsee_types::{Request, Response};
    use serde_json::Value;

    use std::{convert::Infallible, net::SocketAddr};
    use tokio::net::TcpListener;

    use crate::server::JsonRpcLayer;

    async fn handle_json_rpc<'a, T: Clone + Sized + 'a>(
        _req: Request<'a>,
    ) -> Result<Response<'a, T>, Infallible> {
        todo!();
    }

    #[tokio::test]
    async fn test_build_service() {
        let addr: SocketAddr = ([127, 0, 0, 1], 3000).into();
        let app = tower::ServiceBuilder::new()
            .layer(JsonRpcLayer)
            .service_fn(handle_json_rpc::<Value>);

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
