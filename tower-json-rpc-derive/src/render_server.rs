// Copyright 2019-2021 Parity Technologies (UK) Ltd.
//
// Permission is hereby granted, free of charge, to any
// person obtaining a copy of this software and associated
// documentation files (the "Software"), to deal in the
// Software without restriction, including without
// limitation the rights to use, copy, modify, merge,
// publish, distribute, sublicense, and/or sell copies of
// the Software, and to permit persons to whom the Software
// is furnished to do so, subject to the following
// conditions:
//
// The above copyright notice and this permission notice
// shall be included in all copies or substantial portions
// of the Software.
//
// THE SOFTWARE IS PROVIDED "AS IS", WITHOUT WARRANTY OF
// ANY KIND, EXPRESS OR IMPLIED, INCLUDING BUT NOT LIMITED
// TO THE WARRANTIES OF MERCHANTABILITY, FITNESS FOR A
// PARTICULAR PURPOSE AND NONINFRINGEMENT. IN NO EVENT
// SHALL THE AUTHORS OR COPYRIGHT HOLDERS BE LIABLE FOR ANY
// CLAIM, DAMAGES OR OTHER LIABILITY, WHETHER IN AN ACTION
// OF CONTRACT, TORT OR OTHERWISE, ARISING FROM, OUT OF OR
// IN CONNECTION WITH THE SOFTWARE OR THE USE OR OTHER
// DEALINGS IN THE SOFTWARE.

use super::RpcDescription;
use proc_macro2::TokenStream as TokenStream2;
use quote::quote;
use std::borrow::Cow;

impl RpcDescription {
	pub(super) fn render_server(&self) -> Result<TokenStream2, syn::Error> {
		let trait_name = &self.trait_def.ident;
		let request_enum_name = quote::format_ident!("{}Request", trait_name);
		let server_layer_name = quote::format_ident!("{}ServerLayer", trait_name);
		let server_service_name = quote::format_ident!("{}ServerService", trait_name);
		
		let request_enum = self.render_request_enum(&request_enum_name)?;
		let into_request_impl = self.render_into_request(&request_enum_name)?;
		let try_from_request_impl = self.render_try_from_request(&request_enum_name)?;
		let server_layer = self.render_server_layer(&server_layer_name, &server_service_name, &request_enum_name)?;
		
		Ok(quote! {
			#request_enum
			#into_request_impl
			#try_from_request_impl
			#server_layer
		})
	}
	
	fn render_request_enum(&self, enum_name: &syn::Ident) -> Result<TokenStream2, syn::Error> {
		let variants = self.methods.iter().map(|method| {
			let variant_name = to_variant_name(&method.name);
			let params = method.params.iter().map(|param| {
				let name = &param.arg_pat.ident;
				let ty = &param.ty;
				quote! { #name: #ty }
			});
			
			quote! {
				#variant_name {
					#(#params),*
				}
			}
		});
		
		let sub_variants = self.subscriptions.iter().map(|sub| {
			let variant_name = to_variant_name(&sub.name);
			let params = sub.params.iter().map(|param| {
				let name = &param.arg_pat.ident;
				let ty = &param.ty;
				quote! { #name: #ty }
			});
			
			quote! {
				#variant_name {
					#(#params),*
				}
			}
		});
		
		Ok(quote! {
			#[derive(Debug, Clone)]
			pub enum #enum_name {
				#(#variants,)*
				#(#sub_variants,)*
			}
		})
	}
	
	fn render_into_request(&self, enum_name: &syn::Ident) -> Result<TokenStream2, syn::Error> {
		let arms = self.methods.iter().map(|method| {
			let variant_name = to_variant_name(&method.name);
			let method_name = self.rpc_identifier(&method.name);
			let param_idents: Vec<_> = method.params.iter().map(|param| &param.arg_pat.ident).collect();
			
			let params_value = if method.params.is_empty() {
				quote! { None }
			} else if method.param_kind == crate::attributes::ParamKind::Map {
				let param_names = method.params.iter().map(|p| p.name());
				let param_idents2 = param_idents.clone();
				quote! {
					Some(serde_json::value::to_raw_value(&{
						let mut map = serde_json::Map::new();
						#(map.insert(#param_names.to_string(), serde_json::to_value(#param_idents2).unwrap());)*
						map
					}).unwrap())
				}
			} else {
				let param_idents2 = param_idents.clone();
				quote! {
					Some(serde_json::value::to_raw_value(&vec![
						#(serde_json::to_value(#param_idents2).unwrap()),*
					]).unwrap())
				}
			};
			
			quote! {
				#enum_name::#variant_name { #(#param_idents),* } => {
					jsonrpsee_types::Request::owned(#method_name.into(), #params_value, jsonrpsee_types::Id::Number(0))
				}
			}
		});
		
		let sub_arms = self.subscriptions.iter().map(|sub| {
			let variant_name = to_variant_name(&sub.name);
			let method_name = self.rpc_identifier(&sub.name);
			let param_idents: Vec<_> = sub.params.iter().map(|param| &param.arg_pat.ident).collect();
			
			let params_value = if sub.params.is_empty() {
				quote! { None }
			} else if sub.param_kind == crate::attributes::ParamKind::Map {
				let param_names = sub.params.iter().map(|p| p.name());
				let param_idents2 = param_idents.clone();
				quote! {
					Some(serde_json::value::to_raw_value(&{
						let mut map = serde_json::Map::new();
						#(map.insert(#param_names.to_string(), serde_json::to_value(#param_idents2).unwrap());)*
						map
					}).unwrap())
				}
			} else {
				let param_idents2 = param_idents.clone();
				quote! {
					Some(serde_json::value::to_raw_value(&vec![
						#(serde_json::to_value(#param_idents2).unwrap()),*
					]).unwrap())
				}
			};
			
			quote! {
				#enum_name::#variant_name { #(#param_idents),* } => {
					jsonrpsee_types::Request::owned(#method_name.into(), #params_value, jsonrpsee_types::Id::Number(0))
				}
			}
		});
		
		Ok(quote! {
			impl<'a> From<#enum_name> for jsonrpsee_types::Request<'a> {
				fn from(req: #enum_name) -> Self {
					match req {
						#(#arms)*
						#(#sub_arms)*
					}
				}
			}
		})
	}
	
	fn render_try_from_request(&self, enum_name: &syn::Ident) -> Result<TokenStream2, syn::Error> {
		let method_arms = self.methods.iter().map(|method| {
			let variant_name = to_variant_name(&method.name);
			let method_names = names_with_aliases(self.rpc_identifier(&method.name), &method.aliases);
			let method_match = method_names.iter().map(|name| quote! { #name });
			
			let params_extraction = if method.params.is_empty() {
				quote! { Ok(Self::#variant_name {}) }
			} else if method.param_kind == crate::attributes::ParamKind::Map {
				let param_extractions = method.params.iter().map(|param| {
					let name = &param.arg_pat.ident;
					let param_name = param.name();
					let ty = &param.ty;
					quote! {
						let #name: #ty = map.get(#param_name)
							.ok_or_else(|| jsonrpsee_types::ErrorObjectOwned::owned(
								jsonrpsee_types::ErrorCode::InvalidParams.code(),
								jsonrpsee_types::ErrorCode::InvalidParams.message(),
								Some(format!("Missing parameter: {}", #param_name)),
							))?
							.clone();
						let #name: #ty = serde_json::from_value(#name)
							.map_err(|err| jsonrpsee_types::ErrorObjectOwned::owned(
								jsonrpsee_types::ErrorCode::InvalidParams.code(),
								jsonrpsee_types::ErrorCode::InvalidParams.message(),
								Some(err.to_string()),
							))?;
					}
				});
				
				let param_idents = method.params.iter().map(|p| &p.arg_pat.ident);
				quote! {{
					let map: serde_json::Map<String, serde_json::Value> = request.params().parse()?;
					#(#param_extractions)*
					Ok(Self::#variant_name { #(#param_idents),* })
				}}
			} else {
				let param_count = method.params.len();
				let param_extractions = method.params.iter().enumerate().map(|(i, param)| {
					let name = &param.arg_pat.ident;
					let ty = &param.ty;
					quote! {
						let #name: #ty = serde_json::from_value(arr[#i].clone())
							.map_err(|err| jsonrpsee_types::ErrorObjectOwned::owned(
								jsonrpsee_types::ErrorCode::InvalidParams.code(),
								jsonrpsee_types::ErrorCode::InvalidParams.message(),
								Some(err.to_string()),
							))?;
					}
				});
				
				let param_idents = method.params.iter().map(|p| &p.arg_pat.ident);
				quote! {{
					let arr: Vec<serde_json::Value> = request.params().parse()?;
					if arr.len() != #param_count {
						return Err(jsonrpsee_types::ErrorObjectOwned::owned(
							jsonrpsee_types::ErrorCode::InvalidParams.code(),
							jsonrpsee_types::ErrorCode::InvalidParams.message(),
							Some(format!("Expected {} parameters, got {}", #param_count, arr.len())),
						));
					}
					#(#param_extractions)*
					Ok(Self::#variant_name { #(#param_idents),* })
				}}
			};
			
			quote! {
				#(#method_match)|* => #params_extraction
			}
		});
		
		let sub_arms = self.subscriptions.iter().map(|sub| {
			let variant_name = to_variant_name(&sub.name);
			let sub_names = names_with_aliases(self.rpc_identifier(&sub.name), &sub.aliases);
			let sub_match = sub_names.iter().map(|name| quote! { #name });
			
			let params_extraction = if sub.params.is_empty() {
				quote! { Ok(Self::#variant_name {}) }
			} else if sub.param_kind == crate::attributes::ParamKind::Map {
				let param_extractions = sub.params.iter().map(|param| {
					let name = &param.arg_pat.ident;
					let param_name = param.name();
					let ty = &param.ty;
					quote! {
						let #name: #ty = map.get(#param_name)
							.ok_or_else(|| jsonrpsee_types::ErrorObjectOwned::owned(
								jsonrpsee_types::ErrorCode::InvalidParams.code(),
								jsonrpsee_types::ErrorCode::InvalidParams.message(),
								Some(format!("Missing parameter: {}", #param_name)),
							))?
							.clone();
						let #name: #ty = serde_json::from_value(#name)
							.map_err(|err| jsonrpsee_types::ErrorObjectOwned::owned(
								jsonrpsee_types::ErrorCode::InvalidParams.code(),
								jsonrpsee_types::ErrorCode::InvalidParams.message(),
								Some(err.to_string()),
							))?;
					}
				});
				
				let param_idents = sub.params.iter().map(|p| &p.arg_pat.ident);
				quote! {{
					let map: serde_json::Map<String, serde_json::Value> = request.params().parse()?;
					#(#param_extractions)*
					Ok(Self::#variant_name { #(#param_idents),* })
				}}
			} else {
				let param_count = sub.params.len();
				let param_extractions = sub.params.iter().enumerate().map(|(i, param)| {
					let name = &param.arg_pat.ident;
					let ty = &param.ty;
					quote! {
						let #name: #ty = serde_json::from_value(arr[#i].clone())
							.map_err(|err| jsonrpsee_types::ErrorObjectOwned::owned(
								jsonrpsee_types::ErrorCode::InvalidParams.code(),
								jsonrpsee_types::ErrorCode::InvalidParams.message(),
								Some(err.to_string()),
							))?;
					}
				});
				
				let param_idents = sub.params.iter().map(|p| &p.arg_pat.ident);
				quote! {{
					let arr: Vec<serde_json::Value> = request.params().parse()?;
					if arr.len() != #param_count {
						return Err(jsonrpsee_types::ErrorObjectOwned::owned(
							jsonrpsee_types::ErrorCode::InvalidParams.code(),
							jsonrpsee_types::ErrorCode::InvalidParams.message(),
							Some(format!("Expected {} parameters, got {}", #param_count, arr.len())),
						));
					}
					#(#param_extractions)*
					Ok(Self::#variant_name { #(#param_idents),* })
				}}
			};
			
			quote! {
				#(#sub_match)|* => #params_extraction
			}
		});
		
		Ok(quote! {
			impl<'a> TryFrom<jsonrpsee_types::Request<'a>> for #enum_name {
				type Error = jsonrpsee_types::ErrorObjectOwned;
				
				fn try_from(request: jsonrpsee_types::Request<'a>) -> Result<Self, Self::Error> {
					match request.method.as_ref() {
						#(#method_arms,)*
						#(#sub_arms,)*
						_ => Err(jsonrpsee_types::ErrorObjectOwned::from(jsonrpsee_types::ErrorCode::MethodNotFound))
					}
				}
			}
		})
	}
	
	fn render_server_layer(&self, layer_name: &syn::Ident, service_name: &syn::Ident, request_enum_name: &syn::Ident) -> Result<TokenStream2, syn::Error> {
		let trait_name = &self.trait_def.ident;
		let mut all_method_names = Vec::new();
		for method in &self.methods {
			all_method_names.extend(names_with_aliases(self.rpc_identifier(&method.name), &method.aliases));
		}
		for sub in &self.subscriptions {
			all_method_names.extend(names_with_aliases(self.rpc_identifier(&sub.name), &sub.aliases));
		}
		let all_method_match = all_method_names.iter().map(|name| quote! { #name });
		
		let method_match_arms = self.methods.iter().map(|method| {
			let variant_name = to_variant_name(&method.name);
			let method_ident = &method.signature.sig.ident;
			let param_idents: Vec<_> = method.params.iter().map(|param| &param.arg_pat.ident).collect();
			let await_token = if method.signature.sig.asyncness.is_some() {
				quote! { .await }
			} else {
				quote! {}
			};
			
			let param_idents2 = param_idents.clone();
			quote! {
				#request_enum_name::#variant_name { #(#param_idents),* } => {
					let handler = handler.clone();
					let request_id = request_id.clone();
					Box::pin(async move {
						match handler.#method_ident(#(#param_idents2),*)#await_token {
							Ok(result) => {
								let value = serde_json::to_value(result).unwrap();
								jsonrpsee_types::Response::new(
									jsonrpsee_types::ResponsePayload::success(value),
									request_id,
								)
							}
							Err(err) => {
								jsonrpsee_types::Response::new(
									jsonrpsee_types::ResponsePayload::error(err),
									request_id,
								)
							}
						}
					}) as ::tower_json_rpc::server::BoxFuture<
						jsonrpsee_types::Response<'static, serde_json::Value>,
					>
				}
			}
		});
		
		let sub_match_arms = self.subscriptions.iter().map(|sub| {
			let variant_name = to_variant_name(&sub.name);
			quote! {
				#request_enum_name::#variant_name { .. } => {
					let request_id = request_id.clone();
					Box::pin(async move {
						jsonrpsee_types::Response::new(
							jsonrpsee_types::ResponsePayload::error(
								jsonrpsee_types::ErrorObjectOwned::owned(
									jsonrpsee_types::ErrorCode::InvalidRequest.code(),
									jsonrpsee_types::ErrorCode::InvalidRequest.message(),
									Some("Subscriptions not yet implemented"),
								)
							),
							request_id,
						)
					}) as ::tower_json_rpc::server::BoxFuture<
						jsonrpsee_types::Response<'static, serde_json::Value>,
					>
				}
			}
		});
		
		Ok(quote! {
			pub struct #layer_name<H> {
				handler: std::sync::Arc<H>,
			}
			
			impl<H> #layer_name<H> 
			where
				H: #trait_name + Send + Sync + 'static
			{
				pub fn new(handler: H) -> Self {
					Self {
						handler: std::sync::Arc::new(handler)
					}
				}
			}
			
			impl<S, H> tower::Layer<S> for #layer_name<H>
			where
				H: #trait_name + Send + Sync + 'static
			{
				type Service = #service_name<S, H>;
				
				fn layer(&self, inner: S) -> Self::Service {
					#service_name {
						inner,
						handler: self.handler.clone(),
					}
				}
			}
			
			pub struct #service_name<S, H> {
				inner: S,
				handler: std::sync::Arc<H>,
			}
			
			impl<S, H, Req> tower::Service<Req> for #service_name<S, H>
			where
				Req: ::tower_json_rpc::server::ServerRequest,
				S: tower::Service<jsonrpsee_types::Request<'static>, Response = jsonrpsee_types::Response<'static, serde_json::Value>> + Clone + Send + 'static,
				S::Future: Send + 'static,
				S::Error: Into<::tower_json_rpc::error::JsonRpcError> + Send + 'static,
				H: #trait_name + Send + Sync + 'static
			{
				type Response = <Req as ::tower_json_rpc::server::ServerRequest>::Response;
				type Error = ::tower_json_rpc::error::JsonRpcError;
				type Future = ::tower_json_rpc::server::BoxFuture<Result<Self::Response, Self::Error>>;
				
				fn poll_ready(&mut self, cx: &mut std::task::Context<'_>) -> std::task::Poll<Result<(), Self::Error>> {
					self.inner.poll_ready(cx).map_err(Into::into)
				}
				
				fn call(&mut self, request: Req) -> Self::Future {
					use ::tower_json_rpc::__private::futures_util::future::FutureExt;

					let handler = self.handler.clone();
					let mut inner = self.inner.clone();
					let fut = request.into_json_rpc_request();

					Box::pin(fut.then(move |result| match result {
						Ok(json_request) => {
							let json_request: jsonrpsee_types::Request<'static> = json_request;
							let request_id = json_request.id.clone();

							if !matches!(json_request.method.as_ref(), #(#all_method_match)|*) {
								let service_fut = inner.call(json_request);
								return Box::pin(
									service_fut.then(move |service_result| match service_result {
										Ok(response) => <Req::Response as ::tower_json_rpc::server::ServerResponse>::from_json_rpc_response(response),
										Err(err) => Box::pin(async move { Err(err.into()) }),
									}),
								) as ::tower_json_rpc::server::BoxFuture<
									Result<Req::Response, ::tower_json_rpc::error::JsonRpcError>,
								>;
							}

							let response_fut: ::tower_json_rpc::server::BoxFuture<
								jsonrpsee_types::Response<'static, serde_json::Value>,
							> = match #request_enum_name::try_from(json_request) {
								Ok(parsed_request) => match parsed_request {
									#(#method_match_arms)*
									#(#sub_match_arms)*
								},
								Err(err) => {
									let request_id = request_id.clone();
									Box::pin(async move {
										jsonrpsee_types::Response::new(
											jsonrpsee_types::ResponsePayload::error(err),
											request_id,
										)
									})
								}
							};

							Box::pin(response_fut.then(move |response| {
								<Req::Response as ::tower_json_rpc::server::ServerResponse>::from_json_rpc_response(response)
							})) as ::tower_json_rpc::server::BoxFuture<
								Result<Req::Response, ::tower_json_rpc::error::JsonRpcError>,
							>
						}
						Err(err) => Box::pin(async move { Err(err) }),
					}))
				}
			}
			
			impl<S, H> Clone for #service_name<S, H> 
			where
				S: Clone
			{
				fn clone(&self) -> Self {
					Self {
						inner: self.inner.clone(),
						handler: self.handler.clone()
					}
				}
			}
		})
	}
}

fn to_variant_name(method_name: &str) -> syn::Ident {
	let mut result = String::new();
	let mut capitalize_next = true;
	
	for ch in method_name.chars() {
		if ch == '_' || ch == '-' {
			capitalize_next = true;
		} else if capitalize_next {
			result.push(ch.to_ascii_uppercase());
			capitalize_next = false;
		} else {
			result.push(ch);
		}
	}

	syn::Ident::new(&result, proc_macro2::Span::call_site())
}

fn names_with_aliases(primary: Cow<'_, str>, aliases: &[String]) -> Vec<String> {
	let mut names = Vec::with_capacity(1 + aliases.len());
	names.push(primary.to_string());
	names.extend(aliases.iter().cloned());
	names
}
