use http_body_util::Full;
use hyper::body::Bytes;
use hyper_rustls::HttpsConnector;
use hyper_util::client::legacy::{Client, connect::HttpConnector};
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

pub trait ClientResponse: Send + 'static {
    fn to_json_rpc_response(
        self,
    ) -> Pin<
        Box<dyn Future<Output = Result<Response<'static, Value>, JsonRpcError>> + Send + 'static>,
    >;
}

pub type MyClient = Client<HttpsConnector<HttpConnector>, Full<Bytes>>;

/// A layer that maps http requests to JSON-RPC requests.
#[derive(Clone, Debug, Default)]
pub struct JsonRpcClientLayer<Req> {
    _req: std::marker::PhantomData<Req>,
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
    Req: ClientRequest + Clone + Send + 'static,
    S: Service<Req, Response = <Req as ClientRequest>::Response> + Clone + Send + 'static,
    S::Future: Send + 'static,
    S::Error: Into<JsonRpcError>,
{
    type Response = Response<'static, Value>;
    type Error = JsonRpcError;
    type Future =
        Pin<Box<dyn Future<Output = Result<Self::Response, Self::Error>> + Send + 'static>>;

    fn poll_ready(&mut self, cx: &mut Context<'_>) -> Poll<Result<(), Self::Error>> {
        self.inner.poll_ready(cx).map_err(Into::into)
    }

    fn call(&mut self, request: Request<'static>) -> Self::Future {
        // See https://github.com/tower-rs/tower/blob/abb375d08cf0ba34c1fe76f66f1aba3dc4341013/tower-service/src/lib.rs#L276
        // for an explanation of this pattern
        let mut service = self.clone();
        service.inner = std::mem::replace(&mut self.inner, service.inner);

        Box::pin(async move {
            let client_request = Req::from_json_rpc_request(request).await?;
            let response = service
                .inner
                .call(client_request)
                .await
                .map_err(Into::into)?;
            response.to_json_rpc_response().await
        })
    }
}
#[cfg(test)]
mod tests {
    use http_body_util::Full;
    use hyper::body::Bytes;
    use hyper_util::{client::legacy::Client, rt::TokioExecutor};

    use tower::Service;
    use tower::ServiceExt;

    use crate::client::{JsonRpcClientLayer, MyClient};

    #[tokio::test]
    async fn test_build_client() {
        let connector = hyper_rustls::HttpsConnectorBuilder::new()
            .with_native_roots()
            .expect("no native root CA certificates found")
            .https_or_http()
            .enable_http1()
            .build();

        let builder: MyClient = Client::builder(TokioExecutor::new()).build(connector);

        let mut client = tower::ServiceBuilder::new()
            .layer(JsonRpcClientLayer::<http::Request<Full<Bytes>>>::default())
            .service(builder);

        client.ready();
        let req = todo!();
        let _ = client.call(req).await;
    }
}
