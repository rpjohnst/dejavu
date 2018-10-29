#![recursion_limit = "128"]
#![feature(box_patterns)]

extern crate proc_macro;
extern crate syn;
extern crate quote;

use std::collections::HashMap;
use proc_macro::TokenStream;
use syn::parse::{Parse, ParseStream, Error};
use syn::spanned::Spanned;
use quote::quote;

#[proc_macro_attribute]
pub fn bind(attr: TokenStream, input: TokenStream) -> TokenStream {
    // Parse the impl and collect method attributes.

    let api: syn::Ident = syn::parse_macro_input!(attr);
    let mut input: ItemBind = syn::parse_macro_input!(input);

    let function: syn::Path = syn::parse_quote! { gml::function };
    let get: syn::Path = syn::parse_quote! { gml::get };
    let set: syn::Path = syn::parse_quote! { gml::set };

    let mut functions = vec![];
    let mut fields: HashMap<syn::LitStr, _> = HashMap::default();
    for item in &mut input.items {
        let method = match *item {
            syn::ImplItem::Method(ref mut method) => method,
            _ => continue,
        };

        for attr in &method.attrs {
            if attr.path == function {
                let function = match Function::parse(method) {
                    Ok(function) => function,
                    Err(err) => return TokenStream::from(err.to_compile_error()),
                };
                functions.push(function);
            } else if attr.path == get {
                let getter = match Member::parse(attr.tts.clone().into(), method) {
                    Ok(getter) => getter,
                    Err(err) => return TokenStream::from(err.to_compile_error()),
                };
                let (ref mut get, _) = *fields.entry(getter.member).or_insert((None, None));
                *get = Some(getter.ident);
            } else if attr.path == set {
                let setter = match Member::parse(attr.tts.clone().into(), method) {
                    Ok(setter) => setter,
                    Err(err) => return TokenStream::from(err.to_compile_error()),
                };
                let (_, ref mut set) = *fields.entry(setter.member).or_insert((None, None));
                *set = Some(setter.ident);
            }
        }
        method.attrs.retain(|attr| ![&function, &get, &set].contains(&&attr.path));
    }

    let mut members = vec![];
    for (member, (get, set)) in fields {
        match (get, set) {
            (Some(get), Some(set)) => members.push((member, get, set)),
            _ => {
                let error = Error::new(member.span(), "member requires a getter and a setter");
                return TokenStream::from(error.to_compile_error());
            }
        }
    }

    // Generate the API glue trait.
    // TODO: remove these _n clones with https://github.com/dtolnay/quote/issues/8.

    let self_ty = &input.self_ty;
    let items = &input.items;

    let function = functions.iter().map(|method| &method.ident);
    let function_1 = function.clone();
    let function_2 = function.clone();
    let function_3 = function.clone();
    let function_4 = function.clone();
    let arity = functions.iter().map(|method| method.parameters.len());
    let arguments = functions.iter().map(|method| {
        let argument = method.parameters.iter().enumerate().map(|(i, &parameter)| {
            match parameter {
                Parameter::Direct => quote! { arguments[#i] },
                Parameter::Convert => quote! { arguments[#i].try_into().unwrap_or_default() },
                Parameter::Variadic => quote! { &arguments[#i..] },
            }
        });
        quote! { #(#argument),* }
    });
    let variadic = functions.iter().map(|method| method.variadic);

    let member = members.iter().map(|(member, _, _)| member);
    let get = members.iter().map(|(_, get, _)| get);
    let get_1 = get.clone();
    let get_2 = get.clone();
    let get_3 = get.clone();
    let set = members.iter().map(|(_, _, set)| set);
    let set_1 = set.clone();
    let set_2 = set.clone();
    let set_3 = set.clone();

    let output = quote! {
        pub trait #api {
            fn state(&self) -> &#self_ty;
            fn state_mut(&mut self) -> &mut #self_ty;

            fn register(items: &mut std::collections::HashMap<Symbol, gml::Item<Self>>) where
                Self: Sized
            {
                #(items.insert(
                    gml::symbol::Symbol::intern(stringify!(#function_1)),
                    gml::Item::Native(Self::#function_2, #arity, #variadic),
                );)*

                #(items.insert(
                    gml::symbol::Symbol::intern(#member),
                    gml::Item::Member(Self::#get_1, Self::#set_1),
                );)*
            }

            #(fn #function_3(&mut self, arguments: &[gml::vm::Value]) ->
                Result<gml::vm::Value, gml::vm::ErrorKind>
            {
                #[allow(unused_imports)]
                use std::convert::TryInto;

                let state = self.state_mut();
                let ret = state.#function_4(#arguments)?;
                Ok(ret.into())
            })*

            #(fn #get_2(&self, entity: gml::vm::Entity, index: usize) -> gml::vm::Value {
                let state = self.state();
                let value = state.#get_3(entity, index);
                value.into()
            })*

            #(fn #set_2(&mut self, entity: gml::vm::Entity, index: usize, value: gml::vm::Value) {
                #[allow(unused_imports)]
                use std::convert::TryInto;

                let state = self.state_mut();
                state.#set_3(entity, index, value.try_into().unwrap_or_default());
            })*
        }

        impl #self_ty {
            #(#items)*
        }
    };
    output.into()
}

struct ItemBind {
    self_ty: syn::Type,
    items: Vec<syn::ImplItem>,
}

impl Parse for ItemBind {
    fn parse(input: ParseStream) -> syn::parse::Result<Self> {
        let _: syn::Token![impl] = input.parse()?;
        let self_ty: syn::Type = input.parse()?;

        let content;
        syn::braced!(content in input);

        let mut items = vec![];
        while !content.is_empty() {
            items.push(content.parse()?);
        }

        Ok(ItemBind { self_ty, items })
    }
}

struct Function {
    ident: syn::Ident,
    parameters: Vec<Parameter>,
    variadic: bool,
}

#[derive(Copy, Clone, Eq, PartialEq)]
enum Parameter {
    Direct,
    Convert,
    Variadic,
}

impl Function {
    fn parse(method: &syn::ImplItemMethod) -> syn::parse::Result<Self> {
        let ident = method.sig.ident.clone();
        let mut parameters = vec![];
        let mut variadic = false;

        let value: syn::Path = syn::parse_quote! { vm::Value };

        let mut inputs = method.sig.decl.inputs.iter();
        match inputs.next() {
            Some(&syn::FnArg::SelfRef(syn::ArgSelfRef { mutability: Some(_), .. })) => {}
            _ => return Err(Error::new(method.sig.span(), "expected `&mut self`")),
        }
        while let Some(parameter) = inputs.next() {
            let ty = match *parameter {
                syn::FnArg::Captured(syn::ArgCaptured { ref ty, .. }) => ty,
                _ => return Err(Error::new(parameter.span(), "unsupported parameter")),
            };
            let parameter = match *ty {
                syn::Type::Path(syn::TypePath { qself: None, ref path }) if *path == value =>
                    Parameter::Direct,

                syn::Type::Reference(syn::TypeReference {
                    mutability: None,
                    elem: box syn::Type::Slice(syn::TypeSlice {
                        elem: box syn::Type::Path(syn::TypePath { qself: None, ref path }),
                        ..
                    }),
                    ..
                }) if *path == value =>
                    Parameter::Variadic,

                _ => Parameter::Convert,
            };
            parameters.push(parameter);

            if parameter == Parameter::Variadic {
                variadic = true;
                break;
            }
        }
        if let Some(parameter) = inputs.next() {
            return Err(Error::new(parameter.span(), "unexpected parameter"));
        }

        Ok(Function { ident, parameters, variadic })
    }
}

struct Member {
    member: syn::LitStr,
    ident: syn::Ident,
}

struct MemberArg {
    member: syn::LitStr,
}

impl Member {
    fn parse(attr: TokenStream, method: &syn::ImplItemMethod) -> syn::parse::Result<Self> {
        let attr: MemberArg = syn::parse(attr)?;
        let member = attr.member;

        let ident = method.sig.ident.clone();

        Ok(Member { member, ident })
    }
}

impl Parse for MemberArg {
    fn parse(input: ParseStream) -> syn::parse::Result<Self> {
        let content;
        syn::parenthesized!(content in input);
        let member = content.parse()?;
        Ok(MemberArg { member })
    }
}
