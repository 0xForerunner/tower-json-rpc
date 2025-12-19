#![allow(async_fn_in_trait)]

use jsonrpsee_types::Request;
use tower_json_rpc_derive::rpc;

#[rpc(server, namespace = "say")]
pub trait Say {
    #[method(name = "hello")]
    async fn say_hello(&self, name: String) -> Result<String, jsonrpsee_types::ErrorObjectOwned>;
}

fn main() {
    let request: Request<'static> = SayRequest::Hello { name: "Ada".into() }.into();
    let parsed = SayRequest::try_from(request).expect("request should parse");
    let _ = parsed;
}
