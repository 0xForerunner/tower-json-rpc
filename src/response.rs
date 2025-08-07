use crate::error::JsonRpcError;

use serde::{Deserialize, Serialize};
use serde_json::Value;

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct JsonRpcResponse {
    pub result: Option<Value>,
    // pub error: Option<JsonRpcError>,
    pub id: Option<Value>,
}

impl<B> TryFrom<JsonRpcResponse> for http::Response<B> {
    type Error = JsonRpcError;

    fn try_from(value: JsonRpcResponse) -> Result<Self, Self::Error> {
        todo!()
    }
}

impl<B> TryFrom<http::Response<B>> for JsonRpcResponse {
    type Error = JsonRpcError;

    fn try_from(value: http::Response<B>) -> Result<Self, Self::Error> {
        todo!()
    }
}
