# Instructions

I just pulled in the proc mocros from the jsonrpsee repo. They are in tower-json-rpc-derive.

I would like the api of the macro to stay the same, but I want to change what it renders. 

It's used like this:

```rust
#[rpc(server, client, namespace = "say")]
pub trait Say {
	#[method(name = "hello")]
	async fn say_hello(&self, param_0: bool) -> Result<String, ErrorObjectOwned>;
}
```

Instead of what the macro currently does, I would like it to do a few different things:

- It should create an enum like this:
```rust
pub enum SayRequest{
	  Hello {
        param_0: bool
	  }
}

impl<'a> Into<jsonrpsee_types::Request<'a>> for SayRequest {
    ..
}

impl<'a> TryFrom<jsonrpsee_types::Request<'a>> for SayRequest {
	  Error = jsonrpsee_types::ErrorObjectOwned;
    ..
}
```

- It should also create a new tower Layer `SayServerLayer` where we use the fn impl defined in the input trait, `fn say_hello`
  - The new layer service should impl `Service<jsonrpsee_types::Request, Response = jsonrpsee_types::Response>`
- In the original macro you can do this:
```rust
#[subscription(name = "subscribeStorage" => "override", item = Vec<Hash>)]
```
I don't really know what this is doing, but maybe try to keep this functionality if you can.
- You can remove anything else related to jsonrpsee specific things in the macro.
- Please make add tests and make sure they're passing.
