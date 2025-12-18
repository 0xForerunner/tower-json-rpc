use std::pin::Pin;

use futures::FutureExt as _;
use http::header;
use http_body_util::BodyExt;
use hyper::body::{Body, Bytes};
use jsonrpsee_types::Request;
use serde_json::Value;

use crate::{
    error::JsonRpcError,
    server::{ServerRequest, ServerResponse},
};

impl<B> ServerRequest for http::Request<B>
where
    B: Body<Data = Bytes> + Send + 'static,
    B::Error: Into<JsonRpcError>,
{
    type Response = http::Response<B>;

    fn into_json_rpc_request(
        self,
    ) -> Pin<
        Box<
            dyn Future<
                    Output = Result<jsonrpsee_types::Request<'static>, crate::error::JsonRpcError>,
                > + Send
                + 'static,
        >,
    > {
        // let parts = self.into_parts();
        async move {
            let bytes = self.collect().await.map_err(Into::into)?.to_bytes();

            // TODO: make this better
            let cloned = bytes.clone();
            // let deser: Request<'static> = serde_json::from_slice(&cloned)?;
            // Ok(deser)
            todo!()
        }
        .boxed()
    }
}

impl<B> ServerResponse for http::Response<B>
where
    // B: From<Vec<u8>> + Send + 'static,
    B: Body<Data = Bytes> + Send + 'static,
    // B::Error: Into<JsonRpcError>,
{
    fn from_json_rpc_response(
        response: jsonrpsee_types::Response<'static, Value>,
    ) -> Pin<Box<dyn Future<Output = Result<Self, JsonRpcError>> + Send + 'static>> {
        async move {
            let json = serde_json::to_vec(&response).map_err(JsonRpcError::from)?;
            // let body: B = json.into();
            let body: B = todo!();

            http::Response::builder()
                .status(200)
                .header(header::CONTENT_TYPE, "application/json")
                .body(body)
                .map_err(Into::<JsonRpcError>::into)
        }
        .boxed()
    }
}
