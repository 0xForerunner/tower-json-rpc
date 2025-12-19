#![allow(async_fn_in_trait)]

use http_body_util::{BodyExt, Full};
use hyper::body::Bytes;
use hyper_util::client::legacy::{Client, connect::HttpConnector};
use hyper_util::rt::TokioExecutor;
use jsonrpsee_types::{ErrorObjectOwned, Request, Response};
use serde_json::Value;
use std::{
    future::Future,
    pin::Pin,
    task::{Context, Poll},
};
use tower::Service;
use tower_json_rpc::error::JsonRpcError;
use tower_json_rpc_derive::rpc;

// #[rpc(client, namespace = "say")]
pub trait Say {
    // #[method(name = "hello")]
    async fn say_hello(&self, name: String) -> Result<String, jsonrpsee_types::ErrorObjectOwned>;
}

pub trait SayClient<Req> {
    async fn say_hello(&self, name: String) -> Result<String, jsonrpsee_types::ErrorObjectOwned>;
}

impl<T, Req> SayClient<Req> for T
where
    Req: tower_json_rpc::client::ClientRequest + Send + 'static,
    Req::Response: tower_json_rpc::client::ClientResponse + Send + 'static,
    T: Service<Req> + Clone + Send + 'static,
    <T as Service<Req>>::Future: 'static,
    <T as Service<Req>>::Error: Into<JsonRpcError> + Send + 'static,
{
    async fn say_hello(&self, name: String) -> Result<String, jsonrpsee_types::ErrorObjectOwned> {
        todo!()
    }
}

#[tokio::main]
async fn main() -> Result<(), ErrorObjectOwned> {
    let client: Client<_, http_body_util::Full<Bytes>> =
        Client::builder(TokioExecutor::new()).build_http();

    // we should be able to call methods directly on clients that are Service<Request<B>>
    // We can accomplish this with by doing `impl<T, Req> Say for T where T: Service<Req> + ...`
    // This should be done by the client derive macro.
    client.say_hello("test".into()).await?;

    Ok(())
}
