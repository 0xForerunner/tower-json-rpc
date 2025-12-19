use jsonrpsee_types::{Id, Request, Response, ResponsePayload};
use std::{future::Future, pin::Pin};
use tower::{ServiceBuilder, ServiceExt, service_fn};
use tower_json_rpc::client::{ClientRequest, ClientResponse, JsonRpcClientLayer};
use tower_json_rpc::error::JsonRpcError;

#[derive(Clone)]
struct DummyRequest(Request<'static>);

struct DummyResponse(Response<'static, serde_json::Value>);

impl ClientRequest for DummyRequest {
    type Response = DummyResponse;

    fn from_json_rpc_request(
        request: Request<'static>,
    ) -> Pin<Box<dyn Future<Output = Result<Self, JsonRpcError>> + Send + 'static>> {
        Box::pin(async move { Ok(DummyRequest(request)) })
    }
}

impl ClientResponse for DummyResponse {
    fn to_json_rpc_response(
        self,
    ) -> Pin<
        Box<
            dyn Future<Output = Result<Response<'static, serde_json::Value>, JsonRpcError>>
                + Send
                + 'static,
        >,
    > {
        Box::pin(async move { Ok(self.0) })
    }
}

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    let service = ServiceBuilder::new()
        .layer(JsonRpcClientLayer::<DummyRequest>::default())
        .service(service_fn(|req: DummyRequest| async move {
            let id = req.0.id.clone();
            let method = req.0.method.to_string();
            Ok::<_, std::convert::Infallible>(DummyResponse(Response::new(
                ResponsePayload::success(serde_json::json!({ "method": method })),
                id,
            )))
        }));

    let request: Request<'static> = Request::owned("ping".to_string(), None, Id::Number(7));
    let response = service.oneshot(request).await?;
    let _ = response;

    Ok(())
}
