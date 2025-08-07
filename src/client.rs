use futures_util::FutureExt;
use http_body_util::Full;
use hyper::body::Bytes;
use hyper_rustls::HttpsConnector;
use hyper_util::client::legacy::{Client, connect::HttpConnector};
use std::{
    pin::Pin,
    task::{Context, Poll},
};
use tower::{Layer, Service};

use crate::{error::JsonRpcError, request::JsonRpcRequest, response::JsonRpcResponse};

pub trait OutgoingRequest {
    type Response;
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

/// Automatically authenticates every client request with the given `secret`.
#[derive(Debug, Clone)]
pub struct JsonRpcClient<S, Req> {
    inner: S,
    _req: std::marker::PhantomData<Req>,
}

impl<S, Req> Service<JsonRpcRequest> for JsonRpcClient<S, Req>
where
    S: Service<Req, Response = <Req as OutgoingRequest>::Response> + Clone + Send + 'static,
    S::Future: Send + 'static,
    S::Error: Into<JsonRpcError>,
    Req: TryFrom<JsonRpcRequest> + OutgoingRequest + Clone + Send + 'static,
    <Req as TryFrom<JsonRpcRequest>>::Error: Into<JsonRpcError>,
    <Req as OutgoingRequest>::Response: TryInto<JsonRpcResponse>,
    <<Req as OutgoingRequest>::Response as TryInto<JsonRpcResponse>>::Error: Into<JsonRpcError>,
{
    type Response = JsonRpcResponse;
    type Error = JsonRpcError;
    type Future =
        Pin<Box<dyn Future<Output = Result<Self::Response, Self::Error>> + Send + 'static>>;

    fn poll_ready(&mut self, cx: &mut Context<'_>) -> Poll<Result<(), Self::Error>> {
        self.inner.poll_ready(cx).map_err(Into::into)
    }

    fn call(&mut self, request: JsonRpcRequest) -> Self::Future {
        // See https://github.com/tower-rs/tower/blob/abb375d08cf0ba34c1fe76f66f1aba3dc4341013/tower-service/src/lib.rs#L276
        // for an explanation of this pattern
        let mut service = self.clone();
        service.inner = std::mem::replace(&mut self.inner, service.inner);

        async move {
            let response = service
                .inner
                .call(request.try_into().map_err(Into::<JsonRpcError>::into)?)
                .await
                .map_err(Into::<JsonRpcError>::into)?;
            response.try_into().map_err(Into::into)
        }
        .boxed()
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
