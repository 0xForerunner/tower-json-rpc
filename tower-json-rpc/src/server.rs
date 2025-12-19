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
    use http::header;
    use http_body_util::{BodyExt, Full};
    use hyper::body::Bytes;
    use jsonrpsee_types::{Id, Request, Response, ResponsePayload};
    use serde_json::Value;
    use tower::{ServiceBuilder, ServiceExt, service_fn};

    use crate::server::JsonRpcLayer;

    #[tokio::test]
    async fn http_to_jsonrpc_roundtrip() {
        let svc = ServiceBuilder::new()
            .layer(JsonRpcLayer)
            .service(service_fn(|req: Request<'static>| async move {
                let id = req.id.clone();
                let method = req.method.to_string();
                Ok::<_, std::convert::Infallible>(Response::new(
                    ResponsePayload::success(serde_json::json!({ "method": method })),
                    id,
                ))
            }));

        let params = serde_json::value::to_raw_value(&vec![serde_json::json!(true)]).unwrap();
        let json_request: Request<'static> =
            Request::owned("say_hello".to_string(), Some(params), Id::Number(1));

        let body = serde_json::to_vec(&json_request).unwrap();
        let http_request = http::Request::builder()
            .method("POST")
            .header(header::CONTENT_TYPE, "application/json")
            .body(Full::new(Bytes::from(body)))
            .unwrap();

        let http_response = svc.oneshot(http_request).await.unwrap();
        assert_eq!(http_response.status(), 200);

        let response_bytes = http_response
            .into_body()
            .collect()
            .await
            .unwrap()
            .to_bytes();
        let response: Response<'_, Value> = serde_json::from_slice(&response_bytes).unwrap();
        assert!(matches!(response.payload, ResponsePayload::Success(_)));
    }
}
