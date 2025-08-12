# Instructions

please create a new crate in this workspace called `tower-json-rpc-derive`.
It should create a proc macro that can be use like this:

```rust
#[rpc(server, client, namespace = "eth")]
pub enum EthMethods {
    GetBlockByNumber {
        block_parameter: BlockParameter,
        transaction_details: bool
    }
    GetLogs((Option<Address>, Option<u64>, Option<u64>))
}
```
