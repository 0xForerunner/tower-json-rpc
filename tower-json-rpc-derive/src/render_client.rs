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
use crate::attributes::ParamKind;
use proc_macro2::TokenStream as TokenStream2;
use quote::quote;

impl RpcDescription {
    pub(super) fn render_client(&self) -> Result<TokenStream2, syn::Error> {
        if !self.subscriptions.is_empty() {
            return Err(syn::Error::new_spanned(
                &self.trait_def.ident,
                "Client generation for subscriptions is not supported yet",
            ));
        }

        let trait_name = &self.trait_def.ident;
        let client_trait_name = quote::format_ident!("{}Client", trait_name);
        let request_enum_name = quote::format_ident!("{}Request", trait_name);
        let response_enum_name = quote::format_ident!("{}Response", trait_name);

        // Generate the request enum, response enum, and trait impls
        // Only if server is not also being generated (to avoid duplicate request enum)
        let request_enum = if !self.needs_server {
            let enum_def = self.render_client_request_enum(&request_enum_name)?;
            let into_request = self.render_client_into_request(&request_enum_name)?;
            quote! {
                #enum_def
                #into_request
            }
        } else {
            TokenStream2::new()
        };

        // Always generate response enum and RpcRequest/RpcResponse impls for client
        let response_enum = self.render_client_response_enum(&response_enum_name)?;
        let rpc_request_impl =
            self.render_rpc_request_impl(&request_enum_name, &response_enum_name)?;

        let methods = self.methods.iter().map(|method| {
			let method_ident = &method.signature.sig.ident;
			let generics = &method.signature.sig.generics;
			let inputs = &method.signature.sig.inputs;
			let ok_ty = ok_type(&method.signature.sig.output);
			let method_name = self.rpc_identifier(&method.name);
			let param_idents: Vec<_> = method.params.iter().map(|param| &param.arg_pat.ident).collect();
			let params_value = if method.params.is_empty() {
				quote! { None }
			} else if method.param_kind == ParamKind::Map {
				let param_names = method.params.iter().map(|param| param.name());
				let param_idents2 = param_idents.clone();
				quote! {
					Some(::serde_json::value::to_raw_value(&{
						let mut map = ::serde_json::Map::new();
						#(map.insert(#param_names.to_string(), ::serde_json::to_value(#param_idents2).unwrap());)*
						map
					}).unwrap())
				}
			} else {
				let param_idents2 = param_idents.clone();
				quote! {
					Some(::serde_json::value::to_raw_value(&vec![
						#(::serde_json::to_value(#param_idents2).unwrap()),*
					]).unwrap())
				}
			};

			quote! {
				fn #method_ident #generics (#inputs) -> ::core::pin::Pin<
					Box<
						dyn ::core::future::Future<
							Output = Result<#ok_ty, ::tower_json_rpc::error::JsonRpcError>,
						> + 'static,
					>,
				> {
					let request_id: ::jsonrpsee_types::Id<'static> = ::jsonrpsee_types::Id::Number(0);
					let request: ::jsonrpsee_types::Request<'static> = ::jsonrpsee_types::Request::<'static>::owned(
						#method_name.into(),
						#params_value,
						request_id,
					);
					let service = self.clone();
					Box::pin(async move {
						let client_request = Req::from_json_rpc_request(request).await?;
						let mut service = service;
						::tower_json_rpc::__private::futures_util::future::poll_fn(|cx| {
							match ::tower::Service::poll_ready(&mut service, cx) {
								::core::task::Poll::Ready(Ok(())) => ::core::task::Poll::Ready(Ok(())),
								::core::task::Poll::Ready(Err(err)) => ::core::task::Poll::Ready(Err(err.into())),
								::core::task::Poll::Pending => ::core::task::Poll::Pending,
							}
						})
						.await?;
						let response = ::tower::Service::call(&mut service, client_request).await.map_err(Into::into)?;
						let response = <Req::Response as ::tower_json_rpc::client::ClientResponse>::to_json_rpc_response(response).await?;
						match response.payload {
							::jsonrpsee_types::ResponsePayload::Success(value) => {
								let result: #ok_ty = ::serde_json::from_value(value.into_owned())?;
								Ok(result)
							}
							::jsonrpsee_types::ResponsePayload::Error(err) => {
								Err(::tower_json_rpc::error::JsonRpcError::RequestProcessing(err.to_string()))
							}
						}
					})
				}
			}
		});

        Ok(quote! {
            #request_enum
            #response_enum
            #rpc_request_impl

            pub trait #client_trait_name<Req>
            where
                Req: ::tower_json_rpc::client::ClientRequest + Send + 'static,
                Req::Response: ::tower_json_rpc::client::ClientResponse + Send + 'static,
                Self: ::tower::Service<Req, Response = <Req as ::tower_json_rpc::client::ClientRequest>::Response> + Clone + Send + 'static,
                <Self as ::tower::Service<Req>>::Future: 'static,
                <Self as ::tower::Service<Req>>::Error: Into<::tower_json_rpc::error::JsonRpcError> + Send + 'static,
            {
                #(#methods)*
            }

            impl<T, Req> #client_trait_name<Req> for T
            where
                Req: ::tower_json_rpc::client::ClientRequest + Send + 'static,
                Req::Response: ::tower_json_rpc::client::ClientResponse + Send + 'static,
                T: ::tower::Service<Req, Response = <Req as ::tower_json_rpc::client::ClientRequest>::Response> + Clone + Send + 'static,
                <T as ::tower::Service<Req>>::Future: 'static,
                <T as ::tower::Service<Req>>::Error: Into<::tower_json_rpc::error::JsonRpcError> + Send + 'static,
            {}
        })
    }

    fn render_client_request_enum(
        &self,
        enum_name: &syn::Ident,
    ) -> Result<TokenStream2, syn::Error> {
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

        Ok(quote! {
            #[derive(Debug, Clone)]
            pub enum #enum_name {
                #(#variants,)*
            }
        })
    }

    fn render_client_into_request(
        &self,
        enum_name: &syn::Ident,
    ) -> Result<TokenStream2, syn::Error> {
        let arms = self.methods.iter().map(|method| {
			let variant_name = to_variant_name(&method.name);
			let method_name = self.rpc_identifier(&method.name);
			let param_idents: Vec<_> = method.params.iter().map(|param| &param.arg_pat.ident).collect();

			let params_value = if method.params.is_empty() {
				quote! { None }
			} else if method.param_kind == ParamKind::Map {
				let param_names = method.params.iter().map(|p| p.name());
				let param_idents2 = param_idents.clone();
				quote! {
					Some(::serde_json::value::to_raw_value(&{
						let mut map = ::serde_json::Map::new();
						#(map.insert(#param_names.to_string(), ::serde_json::to_value(#param_idents2).unwrap());)*
						map
					}).unwrap())
				}
			} else {
				let param_idents2 = param_idents.clone();
				quote! {
					Some(::serde_json::value::to_raw_value(&vec![
						#(::serde_json::to_value(#param_idents2).unwrap()),*
					]).unwrap())
				}
			};

			quote! {
				#enum_name::#variant_name { #(#param_idents),* } => {
					::jsonrpsee_types::Request::owned(#method_name.into(), #params_value, ::jsonrpsee_types::Id::Number(0))
				}
			}
		});

        Ok(quote! {
            impl<'a> From<#enum_name> for ::jsonrpsee_types::Request<'a> {
                fn from(req: #enum_name) -> Self {
                    match req {
                        #(#arms)*
                    }
                }
            }
        })
    }

    fn render_client_response_enum(
        &self,
        enum_name: &syn::Ident,
    ) -> Result<TokenStream2, syn::Error> {
        let variants = self.methods.iter().map(|method| {
            let variant_name = to_variant_name(&method.name);
            let ok_ty = ok_type(&method.signature.sig.output);
            quote! {
                #variant_name(#ok_ty)
            }
        });

        Ok(quote! {
            #[derive(Debug, Clone)]
            pub enum #enum_name {
                #(#variants,)*
            }

            impl ::tower_json_rpc::client::RpcResponse for #enum_name {}
        })
    }

    fn render_rpc_request_impl(
        &self,
        request_enum_name: &syn::Ident,
        response_enum_name: &syn::Ident,
    ) -> Result<TokenStream2, syn::Error> {
        let parse_arms = self.methods.iter().map(|method| {
            let variant_name = to_variant_name(&method.name);
            let ok_ty = ok_type(&method.signature.sig.output);
            let param_idents: Vec<_> = method
                .params
                .iter()
                .map(|param| &param.arg_pat.ident)
                .collect();

            // Match pattern for request variant (with or without fields)
            let pattern = if method.params.is_empty() {
                quote! { #request_enum_name::#variant_name {} }
            } else {
                quote! { #request_enum_name::#variant_name { #(#param_idents),* } }
            };

            quote! {
                #pattern => {
                    let result: #ok_ty = ::serde_json::from_value(value)?;
                    Ok(#response_enum_name::#variant_name(result))
                }
            }
        });

        Ok(quote! {
            impl ::tower_json_rpc::client::RpcRequest for #request_enum_name {
                type Response = #response_enum_name;

                fn parse_response(&self, value: ::serde_json::Value) -> Result<Self::Response, ::tower_json_rpc::error::JsonRpcError> {
                    match self {
                        #(#parse_arms)*
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

fn ok_type(output: &syn::ReturnType) -> syn::Type {
    match output {
        syn::ReturnType::Default => syn::parse_quote!(()),
        syn::ReturnType::Type(_, ty) => {
            result_ok_type(ty.as_ref()).unwrap_or_else(|| (*ty.as_ref()).clone())
        }
    }
}

fn result_ok_type(ty: &syn::Type) -> Option<syn::Type> {
    let syn::Type::Path(type_path) = ty else {
        return None;
    };
    let segment = type_path.path.segments.last()?;
    if segment.ident != "Result" {
        return None;
    }
    let syn::PathArguments::AngleBracketed(args) = &segment.arguments else {
        return None;
    };
    if args.args.len() != 2 {
        return None;
    }
    let mut iter = args.args.iter();
    let syn::GenericArgument::Type(ok_ty) = iter.next()? else {
        return None;
    };
    let _ = iter.next()?;
    Some(ok_ty.clone())
}
