use tower_json_rpc_derive::rpc;
use jsonrpsee_types::ErrorObjectOwned;

#[rpc(server, namespace = "say")]
pub trait Say {
    #[method(name = "hello", aliases = ["say_hello_alias"])]
    async fn say_hello(&self, param_0: bool) -> Result<String, ErrorObjectOwned>;
    
    #[method(name = "goodbye")]
    fn say_goodbye(&self, name: String) -> Result<String, ErrorObjectOwned>;
}

struct SayImpl;

impl Say for SayImpl {
    async fn say_hello(&self, param_0: bool) -> Result<String, ErrorObjectOwned> {
        if param_0 {
            Ok("Hello!".to_string())
        } else {
            Ok("Hi!".to_string())
        }
    }
    
    fn say_goodbye(&self, name: String) -> Result<String, ErrorObjectOwned> {
        Ok(format!("Goodbye, {}!", name))
    }
}

#[test]
fn test_enum_generation() {
    // Test that the enum is generated
    let request = SayRequest::Hello { param_0: true };
    
    // Test Into<Request>
    let _json_request: jsonrpsee_types::Request<'static> = request.into();
}

#[test]
fn test_try_from_request() {
    // Create a JSON-RPC request
    let params = serde_json::value::to_raw_value(&vec![serde_json::json!(true)]).unwrap();
    let request: jsonrpsee_types::Request<'static> =
        jsonrpsee_types::Request::owned("say_hello".to_string(), Some(params), jsonrpsee_types::Id::Number(1));
    
    // Test TryFrom<Request>
    let parsed = SayRequest::try_from(request);
    assert!(parsed.is_ok());
    
    match parsed.unwrap() {
        SayRequest::Hello { param_0 } => {
            assert_eq!(param_0, true);
        }
        _ => panic!("Wrong variant"),
    }

    let alias_params = serde_json::value::to_raw_value(&vec![serde_json::json!(true)]).unwrap();
    let alias_request: jsonrpsee_types::Request<'static> = jsonrpsee_types::Request::owned(
        "say_hello_alias".to_string(),
        Some(alias_params),
        jsonrpsee_types::Id::Number(2),
    );

    let parsed_alias = SayRequest::try_from(alias_request);
    assert!(parsed_alias.is_ok());
}

#[tokio::test]
async fn test_service_layer() {
    use tower::{Layer, Service, ServiceExt};
    
    let handler = SayImpl;
    let layer = SayServerLayer::new(handler);
    
    // Create a dummy inner service
    let inner = tower::service_fn(|_req: jsonrpsee_types::Request<'static>| async move {
        Ok::<_, std::convert::Infallible>(jsonrpsee_types::Response::<'static, serde_json::Value>::new(
            jsonrpsee_types::ResponsePayload::success(serde_json::json!("inner")),
            jsonrpsee_types::Id::Number(0),
        ))
    });
    
    let mut service = layer.layer(inner);
    
    // Test the service
    let params = serde_json::value::to_raw_value(&vec![serde_json::json!(true)]).unwrap();
    let request: jsonrpsee_types::Request<'static> =
        jsonrpsee_types::Request::owned("say_hello".to_string(), Some(params), jsonrpsee_types::Id::Number(1));
    
    let response = service.ready().await.unwrap().call(request).await.unwrap();
    
    // Check that we got a successful response
    assert!(matches!(
        response.payload,
        jsonrpsee_types::ResponsePayload::Success(_)
    ));
}

#[tokio::test]
async fn test_service_layer_fallback() {
    use tower::{Layer, Service, ServiceExt};

    let handler = SayImpl;
    let layer = SayServerLayer::new(handler);

    let inner = tower::service_fn(|_req: jsonrpsee_types::Request<'static>| async move {
        Ok::<_, std::convert::Infallible>(jsonrpsee_types::Response::<'static, serde_json::Value>::new(
            jsonrpsee_types::ResponsePayload::success(serde_json::json!("inner")),
            jsonrpsee_types::Id::Number(0),
        ))
    });

    let mut service = layer.layer(inner);

    let request: jsonrpsee_types::Request<'static> =
        jsonrpsee_types::Request::owned("say_unknown".to_string(), None, jsonrpsee_types::Id::Number(2));

    let response = service.ready().await.unwrap().call(request).await.unwrap();
    let payload = serde_json::to_value(response).unwrap();
    assert_eq!(payload.get("result").and_then(|value| value.as_str()), Some("inner"));
}
