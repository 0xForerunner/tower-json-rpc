use axum::Router;
use jsonrpsee_types::{ErrorObjectOwned, Request, Response};
use tower::{ServiceBuilder, service_fn};
use tower_json_rpc::server::JsonRpcLayer;
use tower_json_rpc_derive::rpc;

#[rpc(server, namespace = "say")]
pub trait Say {
    #[method(name = "hello")]
    fn say_hello(&self, name: String) -> Result<String, ErrorObjectOwned>;
}

#[derive(Clone, Debug)]
struct SayImpl;

impl Say for SayImpl {
    fn say_hello(&self, name: String) -> Result<String, ErrorObjectOwned> {
        Ok(format!("Hello, {name}!"))
    }
}

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    // let app = Router::new()
    //     .layer(SayServerLayer::new(SayImpl))
    //     .layer(JsonRpcLayer);
    //
    // let listener = tokio::net::TcpListener::bind("127.0.0.1:3000").await?;
    // axum::serve(listener, app).await?;
    //
    // Ok(())
    let jsonrpc = service_fn(|req: Request<'static>| async move {
        let id = req.id.clone();
        let method = req.method.to_string();

        Ok::<_, std::convert::Infallible>(Response::new(
            jsonrpsee_types::ResponsePayload::success(serde_json::json!({ "method": method })),
            id,
        ))
    });

    // 2) Wrap it with your layer: now it should accept HTTP requests
    let http_svc = ServiceBuilder::new()
        .map_err(|e| todo!())
        .layer(JsonRpcLayer)
        .service(jsonrpc);

    // 3) Mount that service into axum at some endpoint
    let app = Router::new().route_service("/", http_svc);

    let listener = tokio::net::TcpListener::bind("127.0.0.1:3000").await?;
    axum::serve(listener, app).await?;
    Ok(())
}
