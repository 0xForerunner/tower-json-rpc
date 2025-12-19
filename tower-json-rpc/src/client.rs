use http_body_util::{BodyExt, Full};
use hyper::body::{Bytes, Incoming};
use jsonrpsee_types::{Request, Response};
use serde_json::Value;
use std::{
    future::Future,
    pin::Pin,
    task::{Context, Poll},
};
use tower::{Layer, Service};

use crate::error::JsonRpcError;

pub trait ClientRequest: Sized + Send + 'static {
    type Response: ClientResponse;

    fn from_json_rpc_request(
        request: Request<'static>,
    ) -> Pin<Box<dyn Future<Output = Result<Self, JsonRpcError>> + Send + 'static>>;
}

/// Implementation for hyper HTTP requests with `Full<Bytes>` body.
/// This is the common case when using hyper_util's legacy Client.
impl ClientRequest for hyper::Request<Full<Bytes>> {
    type Response = hyper::Response<Incoming>;

    fn from_json_rpc_request(
        request: Request<'static>,
    ) -> Pin<Box<dyn Future<Output = Result<Self, JsonRpcError>> + Send + 'static>> {
        Box::pin(async move {
            let body = serde_json::to_vec(&request)?;
            let http_request = hyper::Request::builder()
                .method(hyper::Method::POST)
                .header(hyper::header::CONTENT_TYPE, "application/json")
                .body(Full::new(Bytes::from(body)))
                .map_err(|e| JsonRpcError::RequestProcessing(e.to_string()))?;
            Ok(http_request)
        })
    }
}

pub trait ClientResponse: Send + 'static {
    fn to_json_rpc_response(
        self,
    ) -> Pin<
        Box<dyn Future<Output = Result<Response<'static, Value>, JsonRpcError>> + Send + 'static>,
    >;
}

impl ClientResponse for hyper::Response<Incoming> {
    fn to_json_rpc_response(
        self,
    ) -> Pin<
        Box<dyn Future<Output = Result<Response<'static, Value>, JsonRpcError>> + Send + 'static>,
    > {
        Box::pin(async move {
            let body = self.into_body().collect().await.map_err(|e| {
                JsonRpcError::RequestProcessing(format!("Failed to read response body: {}", e))
            })?;
            let bytes = body.to_bytes();
            let response: Response<'_, Value> = serde_json::from_slice(&bytes)?;
            Ok(response.into_owned())
        })
    }
}

/// A layer that maps http requests to JSON-RPC requests.
#[derive(Clone, Debug)]
pub struct JsonRpcClientLayer<Req> {
    _req: std::marker::PhantomData<Req>,
}

impl<Req> Default for JsonRpcClientLayer<Req> {
    fn default() -> Self {
        Self {
            _req: std::marker::PhantomData,
        }
    }
}

impl<S, Req> Layer<S> for JsonRpcClientLayer<Req> {
    type Service = JsonRpcClient<S, Req>;

    fn layer(&self, inner: S) -> Self::Service {
        JsonRpcClient {
            inner,
            _req: std::marker::PhantomData,
        }
    }
}

/// Maps JSON-RPC requests to client requests
#[derive(Debug, Clone)]
pub struct JsonRpcClient<S, Req> {
    inner: S,
    _req: std::marker::PhantomData<Req>,
}

impl<S, Req> Service<Request<'static>> for JsonRpcClient<S, Req>
where
    Req: ClientRequest + Send + 'static,
    S: Service<Req, Response = <Req as ClientRequest>::Response> + Clone + Send + 'static,
    S::Future: Send + 'static,
    S::Error: Into<JsonRpcError> + Send + 'static,
{
    type Response = Response<'static, Value>;
    type Error = JsonRpcError;
    type Future =
        Pin<Box<dyn Future<Output = Result<Self::Response, Self::Error>> + Send + 'static>>;

    fn poll_ready(&mut self, cx: &mut Context<'_>) -> Poll<Result<(), Self::Error>> {
        self.inner.poll_ready(cx).map_err(Into::into)
    }

    fn call(&mut self, request: Request<'static>) -> Self::Future {
        let mut service = self.inner.clone();

        Box::pin(async move {
            let client_request = Req::from_json_rpc_request(request).await?;
            let response = service.call(client_request).await.map_err(Into::into)?;
            response.to_json_rpc_response().await
        })
    }
}
#[cfg(test)]
mod tests {
    use jsonrpsee_types::{Id, Request, Response, ResponsePayload};
    use std::{future::Future, pin::Pin};
    use tower::{ServiceBuilder, ServiceExt, service_fn};

    use crate::client::{ClientRequest, ClientResponse, JsonRpcClientLayer};
    use crate::error::JsonRpcError;

    #[derive(Clone)]
    struct DummyRequest(Request<'static>);

    struct DummyResponse(Response<'static, serde_json::Value>);

    impl ClientRequest for DummyRequest {
        type Response = DummyResponse;

        fn from_json_rpc_request(
            request: Request<'static>,
        ) -> Pin<Box<dyn Future<Output = Result<Self, JsonRpcError>> + Send + 'static>> {
            Box::pin(async move { Ok(DummyRequest(request)) })
        }
    }

    impl ClientResponse for DummyResponse {
        fn to_json_rpc_response(
            self,
        ) -> Pin<
            Box<
                dyn Future<Output = Result<Response<'static, serde_json::Value>, JsonRpcError>>
                    + Send
                    + 'static,
            >,
        > {
            Box::pin(async move { Ok(self.0) })
        }
    }

    #[tokio::test]
    async fn client_layer_roundtrip() {
        let service = ServiceBuilder::new()
            .layer(JsonRpcClientLayer::<DummyRequest>::default())
            .service(service_fn(|req: DummyRequest| async move {
                let id = req.0.id.clone();
                let method = req.0.method.to_string();
                Ok::<_, std::convert::Infallible>(DummyResponse(Response::new(
                    ResponsePayload::success(serde_json::json!({ "method": method })),
                    id,
                )))
            }));

        let request: Request<'static> = Request::owned("ping".to_string(), None, Id::Number(7));

        let response = service.oneshot(request).await.unwrap();
        assert!(matches!(response.payload, ResponsePayload::Success(_)));
    }
}
