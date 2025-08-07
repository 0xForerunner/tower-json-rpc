use serde::{Deserialize, Serialize};
use serde_json::Value;

pub mod error;
pub mod layer;

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct JsonRpcRequest {
    pub method: String,
    pub params: Value,
    pub id: Option<Value>,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct JsonRpcResponse {
    pub result: Option<Value>,
    // pub error: Option<JsonRpcError>,
    pub id: Option<Value>,
}
