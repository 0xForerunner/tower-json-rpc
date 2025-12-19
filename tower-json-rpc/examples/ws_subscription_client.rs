//! Example of using the WebSocket client with subscriptions.
//!
//! This example demonstrates how to:
//! 1. Connect to a WebSocket JSON-RPC server
//! 2. Make regular RPC calls
//! 3. Subscribe to notifications using the generated subscription trait
//!
//! To run this example, you need a JSON-RPC server that supports WebSocket
//! subscriptions (like an Ethereum node with WebSocket enabled).
//!
//! ```bash
//! # Start a local Ethereum node (e.g., Anvil from Foundry)
//! anvil
//!
//! # In another terminal, run the example
//! cargo run --example ws_subscription_client --features ws
//! ```

#![allow(async_fn_in_trait)]
#![allow(non_snake_case)]

use tower_json_rpc::error::JsonRpcError;
use tower_json_rpc::ws_client::{WsClient, WsClientBuilder};
use tower_json_rpc_derive::rpc;

/// Ethereum-style RPC trait with subscriptions.
#[rpc(client, namespace = "eth")]
pub trait Eth {
    /// Get the current block number.
    async fn blockNumber(&self) -> Result<String, JsonRpcError>;

    /// Subscribe to new block headers.
    // #[subscription(name = "subscribe" => "subscription", item = NewHead)]
    async fn subscribe_new_heads(&self) -> Result<(), JsonRpcError>;
}

/// A new block header notification.
#[derive(Debug, Clone, serde::Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct NewHead {
    pub number: Option<String>,
    pub hash: Option<String>,
    pub parent_hash: String,
    pub nonce: Option<String>,
    pub sha3_uncles: String,
    pub logs_bloom: Option<String>,
    pub transactions_root: String,
    pub state_root: String,
    pub receipts_root: String,
    pub miner: Option<String>,
    pub difficulty: Option<String>,
    pub total_difficulty: Option<String>,
    pub extra_data: String,
    pub size: Option<String>,
    pub gas_limit: String,
    pub gas_used: String,
    pub timestamp: String,
}

#[tokio::main]
async fn main() -> Result<(), JsonRpcError> {
    // Connect to a local Ethereum node (e.g., Geth, Anvil)
    let url = std::env::var("WS_URL").unwrap_or_else(|_| "ws://localhost:8545".to_string());

    println!("Connecting to {}...", url);

    // Build the jsonrpsee client and wrap it
    let js_client = WsClientBuilder::default()
        .build(&url)
        .await
        .map_err(|e| JsonRpcError::RequestProcessing(e.to_string()))?;

    let client = WsClient::new(js_client);

    client.subscribe_new_heads();

    client.blockNumber().await?;

    Ok(())
}
