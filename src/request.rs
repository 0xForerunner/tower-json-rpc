use serde_json::value::RawValue;
use std::borrow::Cow;

use crate::{client::OutgoingRequest, error::JsonRpcError, server::IncomingRequest};

impl<B> IncomingRequest for http::Request<B> {
    type Response = http::Response<B>;
}

impl<B> OutgoingRequest for http::Request<B> {
    type Response = http::Response<Incoming>;
}

use hyper::body::Incoming;
use serde::{Deserialize, Serialize};
use serde_json::Value;

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct JsonRpcRequest<'a, ID = Value, Params = Value>
where
    ID: Clone,
    Params: Clone,
{
    #[serde(borrow)]
    pub id: Cow<'a, ID>,
    #[serde(borrow)]
    pub method: Cow<'a, str>,
    #[serde(borrow)]
    pub params: Cow<'a, Params>,
}

impl<'a, ID, Params, B> TryFrom<http::Request<B>> for JsonRpcRequest<'a, ID, Params>
where
    ID: Clone + Deserialize<'a>,
    Params: Clone + Deserialize<'a>,
    B: hyper::body::Body + Send + 'static,
    B::Data: Send,
    B::Error: Into<JsonRpcError>,
{
    type Error = JsonRpcError;

    fn try_from(value: http::Request<B>) -> Result<Self, Self::Error> {
        todo!()
    }
}

impl<'a, ID, Params, B> TryFrom<JsonRpcRequest<'a, ID, Params>> for http::Request<B>
where
    ID: Clone + Deserialize<'a>,
    Params: Clone + Deserialize<'a>,
    B: hyper::body::Body + Send + 'static,
    B::Data: Send,
    B::Error: Into<JsonRpcError>,
{
    type Error = JsonRpcError;

    fn try_from(value: JsonRpcRequest<'a, ID, Params>) -> Result<Self, Self::Error> {
        todo!()
    }
}
