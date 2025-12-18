use tower_json_rpc_derive::rpc;
use jsonrpsee_types::ErrorObjectOwned;

#[rpc(server, namespace = "say")]
pub trait Say {
    #[method(name = "hello")]
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
    let _json_request: jsonrpsee_types::Request = request.into();
}

#[test]
fn test_try_from_request() {
    // Create a JSON-RPC request
    let request = jsonrpsee_types::Request {
        id: jsonrpsee_types::Id::Number(1),
        method: "say_hello".into(),
        params: Some(jsonrpsee_types::Params::Array(vec![
            serde_json::json!(true)
        ])),
        extensions: Default::default(),
    };
    
    // Test TryFrom<Request>
    let parsed = SayRequest::try_from(request);
    assert!(parsed.is_ok());
    
    match parsed.unwrap() {
        SayRequest::Hello { param_0 } => {
            assert_eq!(param_0, true);
        }
        _ => panic!("Wrong variant"),
    }
}

#[tokio::test]
async fn test_service_layer() {
    use tower::ServiceExt;
    
    let handler = SayImpl;
    let layer = SayServerLayer::new(handler);
    
    // Create a dummy inner service
    let inner = tower::service_fn(|_req: jsonrpsee_types::Request<'static>| async move {
        Ok::<_, std::convert::Infallible>(jsonrpsee_types::Response::<'static, serde_json::Value>::new(
            serde_json::json!("inner"),
            jsonrpsee_types::Id::Number(0),
        ))
    });
    
    let mut service = layer.layer(inner);
    
    // Test the service
    let request = jsonrpsee_types::Request {
        id: jsonrpsee_types::Id::Number(1),
        method: "say_hello".into(),
        params: Some(jsonrpsee_types::Params::Array(vec![
            serde_json::json!(true)
        ])),
        extensions: Default::default(),
    };
    
    let response = service.ready().await.unwrap().call(request).await.unwrap();
    
    // Check that we got a successful response
    assert!(response.is_success());
}