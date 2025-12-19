#![allow(dead_code)]

use tower_json_rpc_derive::rpc;

#[test]
fn test_macro_expansion() {
    // This will cause a compilation test if the macro doesn't expand properly
    
    #[rpc(server, namespace = "test")]
    pub trait TestRpc {
        #[method(name = "hello")]
        async fn hello(&self) -> Result<String, jsonrpsee_types::ErrorObjectOwned>;
    }
    
    // If the macro expanded correctly, these types should exist
    let _request = TestRpcRequest::Hello {};
}
