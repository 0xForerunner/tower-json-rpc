# tower-json-rpc

Tower-native JSON-RPC building blocks with a typed, layer-first API. This crate keeps the
JSON-RPC protocol types from `jsonrpsee-types`, but replaces jsonrpsee-style RPC middleware
with plain Tower `Service` and `Layer` composition.

## Why

- Treat JSON-RPC as just another Tower service.
- Compose behavior (timeouts, tracing, auth, rate limits) with standard Tower layers.
- Generate typed request enums instead of runtime method registries.
- Keep transport concerns (HTTP, WS, custom) separate from protocol and business logic.

## Crates

- `tower-json-rpc`: core traits, layers, and error types.
- `tower-json-rpc-derive`: `#[rpc]` macro that generates typed requests and a server layer.

## Core building blocks

Protocol types:

- `tower_json_rpc::types` re-exports `jsonrpsee_types::*` (`Request`, `Response`, `ErrorObjectOwned`, etc).

Server side:

- `ServerRequest` and `ServerResponse`: convert transport requests/responses to and from JSON-RPC.
- `JsonRpcLayer`: a generic Tower layer that performs the conversion and calls a JSON-RPC service.

Client side:

- `ClientRequest` and `ClientResponse`: convert JSON-RPC requests/responses to and from transport types.
- `JsonRpcClientLayer`: a generic Tower layer for clients.

## Macro: typed API + tower layer

Define your RPC API as a trait. The macro keeps your trait intact and generates:

- `<Trait>Request` enum with one variant per method (and subscription).
- `impl From<<Trait>Request> for jsonrpsee_types::Request`.
- `impl TryFrom<jsonrpsee_types::Request> for <Trait>Request`.
- `<Trait>ServerLayer` and `<Trait>ServerService` that dispatch to your trait implementation.

Example:

```rust
use jsonrpsee_types::ErrorObjectOwned;
use tower_json_rpc_derive::rpc;

#[rpc(server, namespace = "say")]
pub trait Say {
    #[method(name = "hello")]
    async fn say_hello(&self, name: String) -> Result<String, ErrorObjectOwned>;
}

struct SayImpl;

impl Say for SayImpl {
    async fn say_hello(&self, name: String) -> Result<String, ErrorObjectOwned> {
        Ok(format!("Hello, {name}!"))
    }
}
```

Generated types (simplified):

```rust
enum SayRequest {
    Hello { name: String },
}

impl From<SayRequest> for jsonrpsee_types::Request<'static> { /* ... */ }
impl TryFrom<jsonrpsee_types::Request<'_>> for SayRequest { /* ... */ }
struct SayServerLayer<H> { /* ... */ }
```

## Layer-first server composition

Put JSON-RPC in the same Tower stack as the rest of your services.

```rust
use jsonrpsee_types::{ErrorObjectOwned, Id, Request, Response};
use tower::{ServiceBuilder, service_fn};
use tower_json_rpc::server::JsonRpcLayer;

let handler = SayImpl;

// The generated layer needs an inner service; this can be a fallback.
let json_rpc_service = SayServerLayer::new(handler).layer(service_fn(|req: Request<'static>| async move {
    Ok::<_, std::convert::Infallible>(
        Response::new_error(req.id, ErrorObjectOwned::method_not_found())
    )
}));

let app = ServiceBuilder::new()
    // Any Tower layers can wrap JSON-RPC.
    .layer(tower_http::trace::TraceLayer::new_for_http())
    .layer(JsonRpcLayer)
    .service(json_rpc_service);
```

## HTTP + JSON-RPC layering

`JsonRpcLayer` is the bridge between transport and protocol. Layers outside it operate on
`http::Request<B>`/`http::Response<B>`, and layers inside it operate on
`jsonrpsee_types::Request`/`jsonrpsee_types::Response`.

```rust
use http_body_util::Full;
use hyper::body::Bytes;
use jsonrpsee_types::{ErrorObjectOwned, Request, Response};
use tower::{ServiceBuilder, ServiceExt, service_fn};
use tower_json_rpc::server::JsonRpcLayer;

let app = ServiceBuilder::new()
    // HTTP-focused layers see http::Request<B>.
    .layer(tower_http::trace::TraceLayer::new_for_http())
    // Bridge: http::Request<B> -> jsonrpsee_types::Request.
    .layer(JsonRpcLayer)
    // RPC-focused layers see jsonrpsee_types::Request.
    .layer(DenyHelloLayer)
    .layer(SayServerLayer::new(SayImpl))
    .service(service_fn(|req: Request<'static>| async move {
        Ok::<_, std::convert::Infallible>(
            Response::new_error(req.id, ErrorObjectOwned::method_not_found())
        )
    }));

let http_req: http::Request<Full<Bytes>> = todo!();
let http_resp = app.oneshot(http_req).await?;
```

The conversion itself is driven by `ServerRequest` and `ServerResponse` implementations.
Use the provided HTTP impls or define your own for custom transports.

## Custom layers that match typed requests

Because the macro generates `TryFrom<Request>` for the typed enum, any layer can
parse and match on variants before the handler runs. This is a clean place to
add per-method auth, metrics, or feature gates.

```rust
use jsonrpsee_types::{ErrorObjectOwned, Request, Response};
use std::{future::Future, pin::Pin, task::{Context, Poll}};
use tower::{Layer, Service};

#[derive(Clone)]
struct DenyHelloLayer;

#[derive(Clone)]
struct DenyHello<S> {
    inner: S,
}

impl<S> Layer<S> for DenyHelloLayer {
    type Service = DenyHello<S>;

    fn layer(&self, inner: S) -> Self::Service {
        DenyHello { inner }
    }
}

impl<S> Service<Request<'static>> for DenyHello<S>
where
    S: Service<Request<'static>, Response = Response<'static, serde_json::Value>> + Clone + Send + 'static,
    S::Future: Send + 'static,
{
    type Response = Response<'static, serde_json::Value>;
    type Error = S::Error;
    type Future = Pin<Box<dyn Future<Output = Result<Self::Response, Self::Error>> + Send>>;

    fn poll_ready(&mut self, cx: &mut Context<'_>) -> Poll<Result<(), Self::Error>> {
        self.inner.poll_ready(cx)
    }

    fn call(&mut self, req: Request<'static>) -> Self::Future {
        let mut inner = self.inner.clone();
        Box::pin(async move {
            if let Ok(SayRequest::Hello { .. }) = SayRequest::try_from(req.clone()) {
                return Ok(Response::new_error(
                    req.id,
                    ErrorObjectOwned::invalid_request("say_hello is disabled"),
                ));
            }

            inner.call(req).await
        })
    }
}

let app = ServiceBuilder::new()
    .layer(DenyHelloLayer)
    .layer(SayServerLayer::new(SayImpl))
    .layer(JsonRpcLayer)
    .service(tower::service_fn(|req: Request<'static>| async move {
        Ok::<_, std::convert::Infallible>(
            Response::new_error(req.id, ErrorObjectOwned::method_not_found())
        )
    }));
```

## Composing multiple RPC APIs

When you have multiple `#[rpc]` traits, build a service per API and route by
method. Namespaces keep routing simple and avoid collisions.

```rust
use jsonrpsee_types::{ErrorObjectOwned, Request, Response};
use tower::{Service, ServiceExt};

let say = SayServerLayer::new(SayImpl).layer(tower::service_fn(|req: Request<'static>| async move {
    Ok::<_, std::convert::Infallible>(
        Response::new_error(req.id, ErrorObjectOwned::method_not_found())
    )
}));

let admin = AdminServerLayer::new(AdminImpl).layer(tower::service_fn(|req: Request<'static>| async move {
    Ok::<_, std::convert::Infallible>(
        Response::new_error(req.id, ErrorObjectOwned::method_not_found())
    )
}));

let router = tower::service_fn(move |req: Request<'static>| {
    let mut say = say.clone();
    let mut admin = admin.clone();
    async move {
        match req.method.as_ref() {
            m if m.starts_with("say_") => say.ready().await?.call(req).await,
            m if m.starts_with("admin_") => admin.ready().await?.call(req).await,
            _ => Ok(Response::new_error(req.id, ErrorObjectOwned::method_not_found())),
        }
    }
});

let app = ServiceBuilder::new()
    .layer(JsonRpcLayer)
    .service(router);
```

You can also route by attempting `SayRequest::try_from` / `AdminRequest::try_from`
if you want to keep dispatch logic fully typed. If parsing fails due to invalid
params, return that error; if it fails because the method is unknown, try the next
service.

## Typed requests for clients or tests

You can build or parse JSON-RPC requests without touching raw JSON.

```rust
use jsonrpsee_types::Request;

let request: Request<'static> = SayRequest::Hello { name: "Ada".into() }.into();
let parsed = SayRequest::try_from(request)?;
```

## Attributes and parameter encoding

The macro keeps the jsonrpsee-style attribute surface:

- `#[rpc(server, client, namespace = "foo", namespace_separator = ".")]`
- `#[method(name = "bar", param_kind = "map")]`
- `#[subscription(name = "subscribeX", item = ItemType)]`
- `#[argument(rename = "paramName")]`

Parameters can be encoded as arrays (default) or maps (`param_kind = "map"`). Map keys
use argument names, or the `#[argument(rename = "...")]` override.

## Design notes

- No RPC middleware type. If you want middleware, use Tower layers.
- Transport is fully pluggable via the `ServerRequest` / `ServerResponse` and
  `ClientRequest` / `ClientResponse` traits.
- Subscriptions are represented in the generated request enum; server-side handling
  is still evolving.

## Status

This is early-stage and intentionally minimal. The README reflects the intended API.
Expect fast iteration and some rough edges while the core pieces are finalized.
