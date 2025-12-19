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
use crate::rpc_macro::RpcSubscription;
use proc_macro2::TokenStream as TokenStream2;
use quote::quote;

impl RpcDescription {
    pub(super) fn render_client(&self) -> Result<TokenStream2, syn::Error> {
        let trait_name = &self.trait_def.ident;
        let client_trait_name = quote::format_ident!("{}Client", trait_name);
        let request_enum_name = quote::format_ident!("{}Request", trait_name);
        let response_enum_name = quote::format_ident!("{}Response", trait_name);

        // Generate the request enum if server is not also being generated (to avoid duplicate request enum)
        let request_enum = if !self.needs_server {
            self.render_client_request_enum(&request_enum_name)?
        } else {
            TokenStream2::new()
        };

        // Always generate response enum and ServerRequest/ServerResponse impls for client
        let response_enum = self.render_client_response_enum(&response_enum_name)?;
        let server_request_impl =
            self.render_server_request_impl(&request_enum_name, &response_enum_name)?;

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

        // Generate subscription methods if there are any
        let subscription_methods: Vec<_> = self.subscriptions.iter()
            .map(|sub| self.render_subscription_method(sub))
            .collect::<Result<Vec<_>, _>>()?;

        // Generate the client trait - for regular RPC methods
        let client_trait = if !self.methods.is_empty() {
            quote! {
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
            }
        } else {
            TokenStream2::new()
        };

        // Generate subscription client trait - for subscription methods
        let subscription_client_trait = if !self.subscriptions.is_empty() {
            let subscription_client_trait_name = quote::format_ident!("{}SubscriptionClient", trait_name);
            quote! {
                pub trait #subscription_client_trait_name: ::jsonrpsee::core::client::SubscriptionClientT {
                    #(#subscription_methods)*
                }

                impl<T> #subscription_client_trait_name for T
                where
                    T: ::jsonrpsee::core::client::SubscriptionClientT,
                {}
            }
        } else {
            TokenStream2::new()
        };

        Ok(quote! {
            #request_enum
            #response_enum
            #server_request_impl
            #client_trait
            #subscription_client_trait
        })
    }

    fn render_subscription_method(&self, sub: &RpcSubscription) -> Result<TokenStream2, syn::Error> {
        let method_ident = &sub.signature.sig.ident;
        let inputs = &sub.signature.sig.inputs;
        let item_ty = &sub.item;
        let subscribe_method = self.rpc_identifier(&sub.name);
        let unsubscribe_method = self.rpc_identifier(&sub.unsubscribe);

        let param_idents: Vec<_> = sub.params.iter().map(|param| &param.arg_pat.ident).collect();
        let params_builder = if sub.params.is_empty() {
            quote! { ::jsonrpsee::core::params::ArrayParams::new() }
        } else if sub.param_kind == ParamKind::Map {
            let param_names = sub.params.iter().map(|p| p.name());
            let param_idents2 = param_idents.clone();
            quote! {
                {
                    let mut params = ::jsonrpsee::core::params::ObjectParams::new();
                    #(params.insert(#param_names, #param_idents2).unwrap();)*
                    params
                }
            }
        } else {
            let param_idents2 = param_idents.clone();
            quote! {
                {
                    let mut params = ::jsonrpsee::core::params::ArrayParams::new();
                    #(params.insert(#param_idents2).unwrap();)*
                    params
                }
            }
        };

        Ok(quote! {
            fn #method_ident(#inputs) -> impl ::core::future::Future<
                Output = Result<
                    ::jsonrpsee::core::client::Subscription<#item_ty>,
                    ::jsonrpsee::core::client::Error
                >
            > + Send {
                let params = #params_builder;
                ::jsonrpsee::core::client::SubscriptionClientT::subscribe(
                    self,
                    #subscribe_method,
                    params,
                    #unsubscribe_method,
                )
            }
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

    fn render_server_request_impl(
        &self,
        request_enum_name: &syn::Ident,
        response_enum_name: &syn::Ident,
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
				#request_enum_name::#variant_name { #(#param_idents),* } => {
					::jsonrpsee_types::Request::owned(#method_name.into(), #params_value, ::jsonrpsee_types::Id::Number(0))
				}
			}
		});

        Ok(quote! {
            impl ::tower_json_rpc::server::ServerRequest for #request_enum_name {
                type Response = #response_enum_name;

                fn into_json_rpc_request(
                    self,
                ) -> ::core::pin::Pin<Box<dyn ::core::future::Future<Output = Result<::jsonrpsee_types::Request<'static>, ::tower_json_rpc::error::JsonRpcError>> + Send + 'static>> {
                    Box::pin(async move {
                        Ok(match self {
                            #(#arms)*
                        })
                    })
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

        // Generate try-parse arms for from_json_rpc_response
        // Note: This tries each variant type in order and returns the first successful parse.
        // This may not correctly identify the variant if multiple methods return the same type.
        let try_parse_arms = self.methods.iter().map(|method| {
            let variant_name = to_variant_name(&method.name);
            let ok_ty = ok_type(&method.signature.sig.output);
            quote! {
                if let Ok(result) = ::serde_json::from_value::<#ok_ty>(value.clone()) {
                    return Ok(#enum_name::#variant_name(result));
                }
            }
        });

        Ok(quote! {
            #[derive(Debug, Clone)]
            pub enum #enum_name {
                #(#variants,)*
            }

            impl ::tower_json_rpc::server::ServerResponse for #enum_name {
                fn from_json_rpc_response(
                    response: ::jsonrpsee_types::Response<'static, ::serde_json::Value>,
                ) -> ::core::pin::Pin<Box<dyn ::core::future::Future<Output = Result<Self, ::tower_json_rpc::error::JsonRpcError>> + Send + 'static>> {
                    Box::pin(async move {
                        match response.payload {
                            ::jsonrpsee_types::ResponsePayload::Success(value) => {
                                let value = value.into_owned();
                                #(#try_parse_arms)*
                                Err(::tower_json_rpc::error::JsonRpcError::RequestProcessing(
                                    "Failed to deserialize response into any known variant".to_string()
                                ))
                            }
                            ::jsonrpsee_types::ResponsePayload::Error(err) => {
                                Err(::tower_json_rpc::error::JsonRpcError::RequestProcessing(err.to_string()))
                            }
                        }
                    })
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
