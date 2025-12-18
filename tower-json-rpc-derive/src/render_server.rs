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

impl RpcDescription {
	pub(super) fn render_server(&self) -> Result<TokenStream2, syn::Error> {
		let trait_def = &self.trait_def;
		let trait_name = &self.trait_def.ident;
		let request_enum_name = quote::format_ident!("{}Request", trait_name);
		let server_layer_name = quote::format_ident!("{}ServerLayer", trait_name);
		let server_service_name = quote::format_ident!("{}ServerService", trait_name);
		
		let request_enum = self.render_request_enum(&request_enum_name)?;
		let into_request_impl = self.render_into_request(&request_enum_name)?;
		let try_from_request_impl = self.render_try_from_request(&request_enum_name)?;
		let server_layer = self.render_server_layer(&server_layer_name, &server_service_name, &request_enum_name)?;
		
		Ok(quote! {
			#trait_def
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
					Some(jsonrpsee_types::Params::Map({
						let mut map = serde_json::Map::new();
						#(map.insert(#param_names.to_string(), serde_json::to_value(#param_idents2).unwrap());)*
						map
					}))
				}
			} else {
				let param_idents2 = param_idents.clone();
				quote! {
					Some(jsonrpsee_types::Params::Array(vec![
						#(serde_json::to_value(#param_idents2).unwrap()),*
					]))
				}
			};
			
			quote! {
				Self::#variant_name { #(#param_idents),* } => {
					jsonrpsee_types::Request {
						id: jsonrpsee_types::Id::Number(0),
						method: #method_name.into(),
						params: #params_value,
						extensions: Default::default(),
					}
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
					Some(jsonrpsee_types::Params::Map({
						let mut map = serde_json::Map::new();
						#(map.insert(#param_names.to_string(), serde_json::to_value(#param_idents2).unwrap());)*
						map
					}))
				}
			} else {
				let param_idents2 = param_idents.clone();
				quote! {
					Some(jsonrpsee_types::Params::Array(vec![
						#(serde_json::to_value(#param_idents2).unwrap()),*
					]))
				}
			};
			
			quote! {
				Self::#variant_name { #(#param_idents),* } => {
					jsonrpsee_types::Request {
						id: jsonrpsee_types::Id::Number(0),
						method: #method_name.into(),
						params: #params_value,
						extensions: Default::default(),
					}
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
			let method_name = self.rpc_identifier(&method.name);
			
			let params_extraction = if method.params.is_empty() {
				quote! { Ok(Self::#variant_name {}) }
			} else if method.param_kind == crate::attributes::ParamKind::Map {
				let param_extractions = method.params.iter().map(|param| {
					let name = &param.arg_pat.ident;
					let param_name = param.name();
					let ty = &param.ty;
					quote! {
						let #name: #ty = map.get(#param_name)
							.ok_or_else(|| jsonrpsee_types::ErrorObjectOwned::invalid_params(format!("Missing parameter: {}", #param_name)))?
							.clone()
							.try_into()
							.map_err(|_| jsonrpsee_types::ErrorObjectOwned::invalid_params(format!("Invalid parameter type: {}", #param_name)))?;
					}
				});
				
				let param_idents = method.params.iter().map(|p| &p.arg_pat.ident);
				quote! {
					if let Some(jsonrpsee_types::Params::Map(map)) = request.params {
						#(#param_extractions)*
						Ok(Self::#variant_name { #(#param_idents),* })
					} else {
						Err(jsonrpsee_types::ErrorObjectOwned::invalid_params("Expected map parameters"))
					}
				}
			} else {
				let param_count = method.params.len();
				let param_extractions = method.params.iter().enumerate().map(|(i, param)| {
					let name = &param.arg_pat.ident;
					let ty = &param.ty;
					quote! {
						let #name: #ty = serde_json::from_value(arr[#i].clone())
							.map_err(|_| jsonrpsee_types::ErrorObjectOwned::invalid_params(format!("Invalid parameter at index {}", #i)))?;
					}
				});
				
				let param_idents = method.params.iter().map(|p| &p.arg_pat.ident);
				quote! {
					if let Some(jsonrpsee_types::Params::Array(arr)) = request.params {
						if arr.len() != #param_count {
							return Err(jsonrpsee_types::ErrorObjectOwned::invalid_params(format!("Expected {} parameters, got {}", #param_count, arr.len())));
						}
						#(#param_extractions)*
						Ok(Self::#variant_name { #(#param_idents),* })
					} else {
						Err(jsonrpsee_types::ErrorObjectOwned::invalid_params("Expected array parameters"))
					}
				}
			};
			
			quote! {
				#method_name => #params_extraction
			}
		});
		
		let sub_arms = self.subscriptions.iter().map(|sub| {
			let variant_name = to_variant_name(&sub.name);
			let method_name = self.rpc_identifier(&sub.name);
			
			let params_extraction = if sub.params.is_empty() {
				quote! { Ok(Self::#variant_name {}) }
			} else if sub.param_kind == crate::attributes::ParamKind::Map {
				let param_extractions = sub.params.iter().map(|param| {
					let name = &param.arg_pat.ident;
					let param_name = param.name();
					let ty = &param.ty;
					quote! {
						let #name: #ty = map.get(#param_name)
							.ok_or_else(|| jsonrpsee_types::ErrorObjectOwned::invalid_params(format!("Missing parameter: {}", #param_name)))?
							.clone()
							.try_into()
							.map_err(|_| jsonrpsee_types::ErrorObjectOwned::invalid_params(format!("Invalid parameter type: {}", #param_name)))?;
					}
				});
				
				let param_idents = sub.params.iter().map(|p| &p.arg_pat.ident);
				quote! {
					if let Some(jsonrpsee_types::Params::Map(map)) = request.params {
						#(#param_extractions)*
						Ok(Self::#variant_name { #(#param_idents),* })
					} else {
						Err(jsonrpsee_types::ErrorObjectOwned::invalid_params("Expected map parameters"))
					}
				}
			} else {
				let param_count = sub.params.len();
				let param_extractions = sub.params.iter().enumerate().map(|(i, param)| {
					let name = &param.arg_pat.ident;
					let ty = &param.ty;
					quote! {
						let #name: #ty = serde_json::from_value(arr[#i].clone())
							.map_err(|_| jsonrpsee_types::ErrorObjectOwned::invalid_params(format!("Invalid parameter at index {}", #i)))?;
					}
				});
				
				let param_idents = sub.params.iter().map(|p| &p.arg_pat.ident);
				quote! {
					if let Some(jsonrpsee_types::Params::Array(arr)) = request.params {
						if arr.len() != #param_count {
							return Err(jsonrpsee_types::ErrorObjectOwned::invalid_params(format!("Expected {} parameters, got {}", #param_count, arr.len())));
						}
						#(#param_extractions)*
						Ok(Self::#variant_name { #(#param_idents),* })
					} else {
						Err(jsonrpsee_types::ErrorObjectOwned::invalid_params("Expected array parameters"))
					}
				}
			};
			
			quote! {
				#method_name => #params_extraction
			}
		});
		
		Ok(quote! {
			impl<'a> TryFrom<jsonrpsee_types::Request<'a>> for #enum_name {
				type Error = jsonrpsee_types::ErrorObjectOwned;
				
				fn try_from(request: jsonrpsee_types::Request<'a>) -> Result<Self, Self::Error> {
					match request.method.as_ref() {
						#(#method_arms,)*
						#(#sub_arms,)*
						_ => Err(jsonrpsee_types::ErrorObjectOwned::method_not_found())
					}
				}
			}
		})
	}
	
	fn render_server_layer(&self, layer_name: &syn::Ident, service_name: &syn::Ident, request_enum_name: &syn::Ident) -> Result<TokenStream2, syn::Error> {
		let trait_name = &self.trait_def.ident;
		
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
					match handler.#method_ident(#(#param_idents2),*)#await_token {
						Ok(result) => {
							let value = serde_json::to_value(result).unwrap();
							jsonrpsee_types::Response::new(value, request_id.clone())
						}
						Err(err) => {
							jsonrpsee_types::Response::new_error(request_id.clone(), err)
						}
					}
				}
			}
		});
		
		let sub_match_arms = self.subscriptions.iter().map(|sub| {
			let variant_name = to_variant_name(&sub.name);
			quote! {
				#request_enum_name::#variant_name { .. } => {
					jsonrpsee_types::Response::new_error(
						request_id.clone(),
						jsonrpsee_types::ErrorObjectOwned::invalid_request("Subscriptions not yet implemented")
					)
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
			
			impl<S, H> tower::Service<jsonrpsee_types::Request<'static>> for #service_name<S, H>
			where
				S: tower::Service<jsonrpsee_types::Request<'static>, Response = jsonrpsee_types::Response<'static, serde_json::Value>> + Send,
				S::Future: Send,
				H: #trait_name + Send + Sync + 'static
			{
				type Response = jsonrpsee_types::Response<'static, serde_json::Value>;
				type Error = S::Error;
				type Future = std::pin::Pin<Box<dyn std::future::Future<Output = Result<Self::Response, Self::Error>> + Send>>;
				
				fn poll_ready(&mut self, cx: &mut std::task::Context<'_>) -> std::task::Poll<Result<(), Self::Error>> {
					self.inner.poll_ready(cx)
				}
				
				fn call(&mut self, request: jsonrpsee_types::Request<'static>) -> Self::Future {
					let handler = self.handler.clone();
					let request_id = request.id.clone();
					
					Box::pin(async move {
						let parsed_request = match #request_enum_name::try_from(request) {
							Ok(req) => req,
							Err(err) => {
								return Ok(jsonrpsee_types::Response::new_error(request_id, err));
							}
						};
						
						let response = match parsed_request {
							#(#method_match_arms)*
							#(#sub_match_arms)*
						};
						
						Ok(response)
					})
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