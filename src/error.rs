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
}

impl From<Infallible> for JsonRpcError {
    fn from(err: Infallible) -> Self {
        match err {}
    }
}
