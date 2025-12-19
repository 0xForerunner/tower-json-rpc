//! WebSocket client for JSON-RPC with subscription support.
//!
//! This module provides a thin wrapper around jsonrpsee's WebSocket client.

use std::{
    future::Future,
    pin::Pin,
    sync::Arc,
    task::{Context, Poll},
};

use jsonrpsee::{
    core::{ClientError, client::ClientT},
    ws_client::WsClient as JsWsClient,
};
use jsonrpsee_types::{Request, Response, ResponsePayload};
use serde_json::Value;
use tower::Service;

use crate::error::JsonRpcError;

/// A thin wrapper around jsonrpsee's WebSocket client.
#[derive(Clone)]
pub struct WsClient {
    inner: Arc<JsWsClient>,
}

impl WsClient {
    /// Create a new `WsClient` from a jsonrpsee `WsClient`.
    pub fn new(inner: JsWsClient) -> Self {
        Self {
            inner: Arc::new(inner),
        }
    }

    /// Get a reference to the underlying jsonrpsee client.
    pub fn inner(&self) -> &JsWsClient {
        &self.inner
    }
}

impl std::ops::Deref for WsClient {
    type Target = JsWsClient;

    fn deref(&self) -> &Self::Target {
        &self.inner
    }
}

impl From<JsWsClient> for WsClient {
    fn from(inner: JsWsClient) -> Self {
        Self::new(inner)
    }
}

impl Service<Request<'static>> for WsClient {
    type Response = Response<'static, Value>;
    type Error = JsonRpcError;
    type Future =
        Pin<Box<dyn Future<Output = Result<Self::Response, Self::Error>> + Send + 'static>>;

    fn poll_ready(&mut self, _cx: &mut Context<'_>) -> Poll<Result<(), Self::Error>> {
        Poll::Ready(Ok(()))
    }

    fn call(&mut self, request: Request<'static>) -> Self::Future {
        let client = self.inner.clone();
        let method = request.method.to_string();
        let id = request.id.clone();

        // Convert params to Vec<Value> for jsonrpsee
        let params: Vec<Value> = if let Some(raw_params) = request.params {
            serde_json::from_str(raw_params.get()).unwrap_or_default()
        } else {
            vec![]
        };

        Box::pin(async move {
            let result: Result<Value, ClientError> = client.request(&method, params).await;

            match result {
                Ok(value) => Ok(Response::new(ResponsePayload::success(value), id)),
                Err(e) => Err(JsonRpcError::RequestProcessing(e.to_string())),
            }
        })
    }
}

// Re-export jsonrpsee ws_client types for convenience
pub use jsonrpsee::ws_client::WsClientBuilder;

// Re-export subscription types
pub use jsonrpsee::core::client::{Subscription, SubscriptionClientT};
