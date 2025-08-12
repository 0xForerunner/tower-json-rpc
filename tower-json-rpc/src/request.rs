//
// impl<ID, Params, B> TryIntoJsonRpcRequest<ID, Params> for http::Request<B>
// where
//     ID: Clone + DeserializeOwned + 'static,
//     Params: Clone + DeserializeOwned + 'static,
//     B: Body<Data = Bytes> + Send + 'static,
//     B::Error: Into<JsonRpcError>,
// {
//     async fn into_json_rpc_request(
//         self,
//     ) -> Result<JsonRpcRequest<'static, ID, Params>, JsonRpcError> {
//         // let parts = self.into_parts();
//         let bytes = self
//             .into_body()
//             .collect()
//             .await
//             .map_err(Into::into)?
//             .to_bytes();
//
//         // TODO: make this better
//         let cloned = bytes.clone();
//         let deser: JsonRpcRequest<ID, Params> = serde_json::from_slice(&cloned)?;
//         let owned = deser.to_owned();
//         Ok(owned)
//     }
// }
//
// impl<'a, B> TryFromJsonRpcRequest<'a> for http::Request<B>
// where
//     B: From<Vec<u8>> + Body + Into<Bytes> + Send + 'static,
//     B::Data: Send,
//     B::Error: Into<JsonRpcError>,
// {
//     async fn from_json_rpc_request(rpc: JsonRpcRequest<'a>) -> Result<Self, JsonRpcError>
//     where
//         Self: Sized,
//     {
//         let json = serde_json::to_vec(&rpc).map_err(JsonRpcError::from)?;
//         let body: B = json.into();
//
//         Request::builder()
//             .method(Method::POST)
//             .uri("/") // <- adjust if you need something else
//             .header(header::CONTENT_TYPE, "application/json")
//             .body(body)
//             .map_err(Into::<JsonRpcError>::into)
//     }
// }
