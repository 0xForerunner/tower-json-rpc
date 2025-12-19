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

//! Declaration of the JSON RPC generator procedural macros.

use std::borrow::Cow;

use crate::attributes::{
	Aliases, Argument, AttributeMeta, MissingArgument, NameMapping, ParamKind, optional, parse_param_kind,
};
use proc_macro2::TokenStream as TokenStream2;
use quote::quote;
use syn::spanned::Spanned;
use syn::{Attribute, Token, punctuated::Punctuated};

/// Represents a single argument in a RPC call.
///
/// stores modifications based on attributes
#[derive(Debug, Clone)]
pub struct RpcFnArg {
	pub(crate) arg_pat: syn::PatIdent,
	rename_to: Option<String>,
	pub(crate) ty: syn::Type,
}

impl RpcFnArg {
	pub fn from_arg_attrs(arg_pat: syn::PatIdent, ty: syn::Type, attrs: &mut Vec<syn::Attribute>) -> syn::Result<Self> {
		let mut rename_to = None;

		if let Some(attr) = find_attr(attrs, "argument") {
			let [rename] = AttributeMeta::parse(attr.clone())?.retain(["rename"])?;

			let rename = optional(rename, Argument::string)?;

			if let Some(rename) = rename {
				rename_to = Some(rename);
			}
		}

		// remove argument attribute after inspection
		attrs.retain(|attr| !attr.meta.path().is_ident("argument"));

	Ok(Self { arg_pat, rename_to, ty })
}
	/// Return the string representation of this argument when (de)seriaizing.
	pub fn name(&self) -> String {
		self.rename_to.clone().unwrap_or_else(|| self.arg_pat.ident.to_string())
	}
}

#[derive(Debug, Clone)]
pub struct RpcMethod {
	pub name: String,
	pub params: Vec<RpcFnArg>,
	pub param_kind: ParamKind,
	pub signature: syn::TraitItemFn,
	pub aliases: Vec<String>,
}

impl RpcMethod {
	pub fn from_item(attr: Option<Attribute>, mut method: syn::TraitItemFn) -> syn::Result<Self> {
		let (aliases, blocking, name, param_kind) = if let Some(attr) = attr {
			let [aliases, blocking, name, param_kind, with_extensions] =
				AttributeMeta::parse(attr)?.retain(["aliases", "blocking", "name", "param_kind", "with_extensions"])?;

			let aliases = parse_aliases(aliases)?;
			let blocking = optional(blocking, Argument::flag)?.is_some();
			// Use explicit name if provided, otherwise convert snake_case to camelCase
			let name = optional(name, Argument::string)?
				.unwrap_or_else(|| snake_to_camel(&method.sig.ident.to_string()));
			let param_kind = parse_param_kind(param_kind)?;
			let _with_extensions = optional(with_extensions, Argument::flag)?.is_some();

			(aliases, blocking, name, param_kind)
		} else {
			// No attribute - use defaults, converting snake_case to camelCase
			(Vec::new(), false, snake_to_camel(&method.sig.ident.to_string()), ParamKind::Array)
		};

		if blocking && method.sig.asyncness.is_some() {
			return Err(syn::Error::new(method.sig.span(), "Blocking method must be synchronous"));
		}

		let params: Vec<_> = method
			.sig
			.inputs
			.iter_mut()
			.filter_map(|arg| match arg {
				syn::FnArg::Receiver(_) => None,
				syn::FnArg::Typed(arg) => match &*arg.pat {
					syn::Pat::Ident(name) => {
						Some(RpcFnArg::from_arg_attrs(name.clone(), (*arg.ty).clone(), &mut arg.attrs))
					}
					syn::Pat::Wild(wild) => Some(Err(syn::Error::new(
						wild.underscore_token.span(),
						"Method argument names must be valid Rust identifiers; got `_` instead",
					))),
					_ => Some(Err(syn::Error::new(
						arg.span(),
						format!("Unexpected method signature input; got {:?} ", *arg.pat),
					))),
				},
			})
			.collect::<Result<_, _>>()?;

		// We've analyzed attributes and don't need them anymore.
		method.attrs.clear();

		Ok(Self {
			aliases,
			name,
			params,
			param_kind,
			signature: method,
		})
	}
}

#[derive(Debug, Clone)]
pub struct RpcSubscription {
	pub name: String,
	pub unsubscribe: String,
	#[allow(dead_code)]
	pub notif_name: String,
	pub item: syn::Type,
	pub params: Vec<RpcFnArg>,
	pub param_kind: ParamKind,
	pub aliases: Vec<String>,
	pub signature: syn::TraitItemFn,
}

impl RpcSubscription {
	pub fn from_item(attr: syn::Attribute, mut sub: syn::TraitItemFn) -> syn::Result<Self> {
		let [aliases, item, name, param_kind, unsubscribe, unsubscribe_aliases, with_extensions] =
			AttributeMeta::parse(attr)?.retain([
				"aliases",
				"item",
				"name",
				"param_kind",
				"unsubscribe",
				"unsubscribe_aliases",
				"with_extensions",
			])?;

		let aliases = parse_aliases(aliases)?;
		let map = name?.value::<NameMapping>()?;
		let name = map.name;
		let notif_name = map.mapped.unwrap_or_else(|| name.clone());
		let item: syn::Type = item?.value()?;
		let param_kind = parse_param_kind(param_kind)?;
		let _unsubscribe_aliases = parse_aliases(unsubscribe_aliases)?;
		let _with_extensions = optional(with_extensions, Argument::flag)?.is_some();

		let unsubscribe = match parse_subscribe(unsubscribe)? {
			Some(unsub) => unsub,
			None => build_unsubscribe_method(&name).unwrap_or_else(||
				panic!("Could not generate the unsubscribe method with name '{name}'. You need to provide the name manually using the `unsubscribe` attribute in your RPC API definition"),
			),
		};

		let params: Vec<_> = sub
			.sig
			.inputs
			.iter_mut()
			.filter_map(|arg| match arg {
				syn::FnArg::Receiver(_) => None,
				syn::FnArg::Typed(arg) => match &*arg.pat {
					syn::Pat::Ident(name) => {
						Some(RpcFnArg::from_arg_attrs(name.clone(), (*arg.ty).clone(), &mut arg.attrs))
					}
					_ => panic!("Identifier in signature must be an ident"),
				},
			})
			.collect::<Result<_, _>>()?;

		// We've analyzed attributes and don't need them anymore.
		sub.attrs.clear();

		Ok(Self {
			name,
			unsubscribe,
			notif_name,
			item,
			params,
			param_kind,
			aliases,
			signature: sub,
		})
	}
}

#[derive(Debug)]
pub struct RpcDescription {
	/// Switch denoting that server trait must be generated.
	/// Assuming that trait to which attribute is applied is named `Foo`, the generated
	/// server trait will have `FooServer` name.
	pub(crate) needs_server: bool,
	/// Switch denoting that client trait must be generated.
	pub(crate) needs_client: bool,
	/// Optional prefix for RPC namespace.
	pub(crate) namespace: Option<String>,
	/// Optional separator between namespace and method name. Defaults to `_`.
	pub(crate) namespace_separator: Option<String>,
	/// Trait definition in which all the attributes were stripped.
	pub(crate) trait_def: syn::ItemTrait,
	/// List of RPC methods defined in the trait.
	pub(crate) methods: Vec<RpcMethod>,
	/// List of RPC subscriptions defined in the trait.
	pub(crate) subscriptions: Vec<RpcSubscription>,
}

impl RpcDescription {
	pub fn from_item(attr: Attribute, mut item: syn::ItemTrait) -> syn::Result<Self> {
		let [client, server, namespace, namespace_separator, client_bounds, server_bounds] =
			AttributeMeta::parse(attr)?.retain([
				"client",
				"server",
				"namespace",
				"namespace_separator",
				"client_bounds",
				"server_bounds",
			])?;

		let needs_server = optional(server, Argument::flag)?.is_some();
		let needs_client = optional(client, Argument::flag)?.is_some();
		let namespace = optional(namespace, Argument::string)?;
		let namespace_separator = optional(namespace_separator, Argument::string)?;
		let _client_bounds: Option<Punctuated<syn::WherePredicate, Token![,]>> =
			optional(client_bounds, Argument::group)?;
		let _server_bounds: Option<Punctuated<syn::WherePredicate, Token![,]>> =
			optional(server_bounds, Argument::group)?;
		if !needs_server && !needs_client {
			return Err(syn::Error::new_spanned(&item.ident, "Either 'server' or 'client' attribute must be applied"));
		}

		if _client_bounds.is_some() && !needs_client {
			return Err(syn::Error::new_spanned(
				&item.ident,
				"Attribute 'client' must be specified with 'client_bounds'",
			));
		}

		if _server_bounds.is_some() && !needs_server {
			return Err(syn::Error::new_spanned(
				&item.ident,
				"Attribute 'server' must be specified with 'server_bounds'",
			));
		}

		item.attrs.clear(); // Remove RPC attributes.

		let mut methods = Vec::new();
		let mut subscriptions = Vec::new();

		// Go through all the methods in the trait and collect methods and
		// subscriptions.
		for entry in item.items.iter() {
			if let syn::TraitItem::Fn(method) = entry {
				if method.sig.receiver().is_none() {
					return Err(syn::Error::new_spanned(&method.sig, "First argument of the trait must be '&self'"));
				}

				let method_attr = find_attr(&method.attrs, "method");
				let sub_attr = find_attr(&method.attrs, "subscription");

				if method_attr.is_some() && sub_attr.is_some() {
					return Err(syn::Error::new_spanned(
						method,
						"Element cannot be both subscription and method at the same time",
					));
				}

				if let Some(attr) = sub_attr {
					let sub_data = RpcSubscription::from_item(attr.clone(), method.clone())?;
					subscriptions.push(sub_data);
				} else {
					// Treat as a method (with or without #[method] attribute)
					let method_data = RpcMethod::from_item(method_attr.cloned(), method.clone())?;
					methods.push(method_data);
				}
			} else {
				return Err(syn::Error::new_spanned(entry, "Only methods allowed in RPC traits"));
			}
		}

		if methods.is_empty() && subscriptions.is_empty() {
			return Err(syn::Error::new_spanned(&item, "RPC cannot be empty"));
		}

		strip_rpc_attrs(&mut item);
		rewrite_async_methods(&mut item);

		Ok(Self {
			needs_server,
			needs_client,
			namespace,
			namespace_separator,
			trait_def: item,
			methods,
			subscriptions,
		})
	}

	pub fn render(self) -> Result<TokenStream2, syn::Error> {
		let trait_def = &self.trait_def;
		let server_impl = if self.needs_server { self.render_server()? } else { TokenStream2::new() };
		let client_impl = if self.needs_client { self.render_client()? } else { TokenStream2::new() };

		Ok(quote! {
			#trait_def
			#server_impl
			#client_impl
		})
	}

	/// Based on the namespace and separator, renders the full name of the RPC method/subscription.
	/// Examples:
	/// For namespace `foo`, method `makeSpam`, and separator `_`, result will be `foo_makeSpam`.
	/// For separator `.`, result will be `foo.makeSpam`.
	/// For no namespace, returns just `makeSpam`.
	pub(crate) fn rpc_identifier<'a>(&self, method: &'a str) -> Cow<'a, str> {
		if let Some(ns) = &self.namespace {
			let sep = self.namespace_separator.as_deref().unwrap_or("_");
			format!("{ns}{sep}{method}").into()
		} else {
			Cow::Borrowed(method)
		}
	}
}

fn parse_aliases(arg: Result<Argument, MissingArgument>) -> syn::Result<Vec<String>> {
	let aliases = optional(arg, Argument::value::<Aliases>)?;

	Ok(aliases.map(|a| a.list.into_iter().map(|lit| lit.value()).collect()).unwrap_or_default())
}

fn parse_subscribe(arg: Result<Argument, MissingArgument>) -> syn::Result<Option<String>> {
	let unsub = optional(arg, Argument::string)?;

	Ok(unsub)
}

fn find_attr<'a>(attrs: &'a [Attribute], ident: &str) -> Option<&'a Attribute> {
	attrs.iter().find(|a| a.path().is_ident(ident))
}

fn build_unsubscribe_method(method: &str) -> Option<String> {
	method.strip_prefix("subscribe").map(|s| format!("unsubscribe{s}"))
}

/// Converts snake_case to camelCase.
/// Examples: "block_number" -> "blockNumber", "get_block_by_hash" -> "getBlockByHash"
fn snake_to_camel(s: &str) -> String {
	let mut result = String::new();
	let mut capitalize_next = false;

	for ch in s.chars() {
		if ch == '_' {
			capitalize_next = true;
		} else if capitalize_next {
			result.push(ch.to_ascii_uppercase());
			capitalize_next = false;
		} else {
			result.push(ch);
		}
	}

	result
}

fn strip_rpc_attrs(item: &mut syn::ItemTrait) {
	for entry in item.items.iter_mut() {
		if let syn::TraitItem::Fn(method) = entry {
			method
				.attrs
				.retain(|attr| !attr.path().is_ident("method") && !attr.path().is_ident("subscription"));
			for input in method.sig.inputs.iter_mut() {
				if let syn::FnArg::Typed(arg) = input {
					arg.attrs.retain(|attr| !attr.path().is_ident("argument"));
				}
			}
		}
	}
}

fn rewrite_async_methods(item: &mut syn::ItemTrait) {
	for entry in &mut item.items {
		let syn::TraitItem::Fn(method) = entry else { continue };

		if method.sig.asyncness.is_none() {
			continue;
		}

		method.sig.asyncness = None;
		let output = match &method.sig.output {
			syn::ReturnType::Default => syn::parse_quote!(()),
			syn::ReturnType::Type(_, ty) => *ty.clone(),
		};
		method.sig.output =
			syn::parse_quote!(-> impl ::core::future::Future<Output = #output> + Send);
	}
}
