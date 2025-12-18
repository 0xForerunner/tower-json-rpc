use std::convert::Infallible;

use thiserror::Error;

#[derive(Error, Debug)]
pub enum JsonRpcError {
    #[error("Request processing error: {0}")]
    RequestProcessing(String),
    #[error("Response serialization error: {0}")]
    ResponseSerialization(String),
    #[error("Request deserialization error: {0}")]
    RequestDeserialization(String),
    #[error("error building: {0}")]
    IntoRpcRequest(String),
    #[error(transparent)]
    HyperClient(#[from] hyper_util::client::legacy::Error),
    #[error(transparent)]
    SerdeJson(#[from] serde_json::Error),
    #[error(transparent)]
    Http(#[from] http::Error),
    #[error(transparent)]
    Hyper(#[from] hyper::Error),
}

impl From<Infallible> for JsonRpcError {
    fn from(err: Infallible) -> Self {
        match err {}
    }
}
