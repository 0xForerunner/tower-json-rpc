use std::{future::Future, pin::Pin};

use http::header;
use http_body_util::{BodyExt, Full};
use hyper::body::{Body, Bytes};
use jsonrpsee_types::{Request, Response};
use serde_json::Value;

use crate::{
    error::JsonRpcError,
    server::{ServerRequest, ServerResponse},
};

impl<B> ServerRequest for http::Request<B>
where
    B: Body<Data = Bytes> + Send + 'static,
    B::Error: Into<JsonRpcError> + Send + 'static,
{
    type Response = http::Response<Full<Bytes>>;

    fn into_json_rpc_request(
        self,
    ) -> Pin<
        Box<
            dyn Future<Output = Result<jsonrpsee_types::Request<'static>, JsonRpcError>>
                + Send
                + 'static,
        >,
    > {
        Box::pin(async move {
            let bytes = self.collect().await.map_err(Into::into)?.to_bytes();
            let request: Request<'_> = serde_json::from_slice(&bytes)?;
            let params = request.params.map(|params| params.into_owned());
            let request =
                Request::owned(request.method.into_owned(), params, request.id.into_owned());
            Ok(request)
        })
    }
}

impl ServerRequest for Request<'static> {
    type Response = Response<'static, Value>;

    fn into_json_rpc_request(
        self,
    ) -> Pin<
        Box<
            dyn Future<Output = Result<jsonrpsee_types::Request<'static>, JsonRpcError>>
                + Send
                + 'static,
        >,
    > {
        Box::pin(async move { Ok(self) })
    }
}

impl ServerResponse for http::Response<Full<Bytes>> {
    fn from_json_rpc_response(
        response: jsonrpsee_types::Response<'static, Value>,
    ) -> Pin<Box<dyn Future<Output = Result<Self, JsonRpcError>> + Send + 'static>> {
        Box::pin(async move {
            let json = serde_json::to_vec(&response).map_err(JsonRpcError::from)?;
            let body = Full::new(Bytes::from(json));

            http::Response::builder()
                .status(200)
                .header(header::CONTENT_TYPE, "application/json")
                .body(body)
                .map_err(Into::<JsonRpcError>::into)
        })
    }
}

impl ServerResponse for Response<'static, Value> {
    fn from_json_rpc_response(
        response: jsonrpsee_types::Response<'static, Value>,
    ) -> Pin<Box<dyn Future<Output = Result<Self, JsonRpcError>> + Send + 'static>> {
        Box::pin(async move { Ok(response) })
    }
}
