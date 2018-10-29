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
    let mut fields: HashMap<syn::Ident, _> = HashMap::default();
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
                let (ref mut get, _) = *fields.entry(getter.member.clone()).or_insert((None, None));
                *get = Some(getter);
            } else if attr.path == set {
                let setter = match Member::parse(attr.tts.clone().into(), method) {
                    Ok(setter) => setter,
                    Err(err) => return TokenStream::from(err.to_compile_error()),
                };
                let (_, ref mut set) = *fields.entry(setter.member.clone()).or_insert((None, None));
                *set = Some(setter);
            }
        }
        method.attrs.retain(|attr| ![&function, &get, &set].contains(&&attr.path));
    }

    let members: Vec<_> = fields.into_iter()
        .map(|(member, (get, set))| (member, get, set))
        .collect();

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
                Parameter::World => quote! { world },
                Parameter::Value => quote! { arguments[#i] },
                Parameter::Convert => quote! { arguments[#i].try_into().unwrap_or_default() },
                Parameter::Variadic => quote! { &arguments[#i..] },
                _ => unreachable!(),
            }
        });
        quote! { #(#argument),* }
    });
    let variadic = functions.iter().map(|function| function.variadic);

    let member = members.iter().map(|(member, _, _)| member);

    let get = members.iter().map(|(_, get, _)| get);
    let get_opt = get.clone().map(|get| get.as_ref().map_or_else(
        || quote! { None },
        |Member { ident, .. }| quote! { Some(Self::#ident) },
    ));
    let get = get.filter_map(|get| get.as_ref());
    let get_1 = get.clone().map(|Member { ident, .. }| ident);
    let get_2 = get.clone().map(|Member { ident, .. }| ident);
    let get_arguments = get.clone().map(|Member { parameters, .. }| {
        let argument = parameters.iter().map(|&parameter| {
            match parameter {
                Parameter::World => quote! { world },
                Parameter::Entity => quote! { entity },
                Parameter::Index => quote! { index },
                _ => unreachable!(),
            }
        });
        quote! { #(#argument),* }
    });

    let set = members.iter().map(|(_, _, set)| set);
    let set_opt = set.clone().map(|set| set.as_ref().map_or_else(
        || quote! { None },
        |Member { ident, .. }| quote! { Some(Self::#ident) },
    ));
    let set = set.filter_map(|set| set.as_ref());
    let set_1 = set.clone().map(|Member { ident, .. }| ident);
    let set_2 = set.clone().map(|Member { ident, .. }| ident);
    let set_arguments = set.clone().map(|Member { parameters, .. }| {
        let argument = parameters.iter().map(|&parameter| {
            match parameter {
                Parameter::World => quote! { world },
                Parameter::Entity => quote! { entity },
                Parameter::Index => quote! { index },
                Parameter::Value => quote! { value },
                Parameter::Convert => quote! { value.try_into().unwrap_or_default() },
                _ => unreachable!(),
            }
        });
        quote! { #(#argument),* }
    });

    let output = quote! {
        pub trait #api {
            fn state(&self) -> (&gml::vm::World, &#self_ty);
            fn state_mut(&mut self) -> (&mut gml::vm::World, &mut #self_ty);

            fn register(items: &mut std::collections::HashMap<Symbol, gml::Item<Self>>) where
                Self: Sized
            {
                #(items.insert(
                    gml::symbol::Symbol::intern(stringify!(#function_1)),
                    gml::Item::Native(Self::#function_2, #arity, #variadic),
                );)*

                #(items.insert(
                    gml::symbol::Symbol::intern(stringify!(#member)),
                    gml::Item::Member(#get_opt, #set_opt),
                );)*
            }

            #(fn #function_3(&mut self, arguments: &[gml::vm::Value]) ->
                Result<gml::vm::Value, gml::vm::ErrorKind>
            {
                #![allow(unused_imports, unused)]
                use std::convert::TryInto;

                let (world, state) = self.state_mut();
                let ret = state.#function_4(#arguments)?;
                Ok(ret.into())
            })*

            #(fn #get_1(&self, entity: gml::vm::Entity, index: usize) -> gml::vm::Value {
                #![allow(unused)]

                let (world, state) = self.state();
                let value = state.#get_2(#get_arguments);
                value.into()
            })*

            #(fn #set_1(&mut self, entity: gml::vm::Entity, index: usize, value: gml::vm::Value) {
                #![allow(unused_imports, unused)]
                use std::convert::TryInto;

                let (world, state) = self.state_mut();
                state.#set_2(#set_arguments);
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

#[derive(Copy, Clone, Eq, PartialEq)]
enum Parameter {
    World,
    Entity,
    Index,
    Value,
    Convert,
    Variadic,
}

struct Function {
    ident: syn::Ident,
    parameters: Vec<Parameter>,
    variadic: bool,
}

impl Function {
    fn parse(method: &syn::ImplItemMethod) -> syn::parse::Result<Self> {
        let ident = method.sig.ident.clone();
        let mut parameters = vec![];
        let mut variadic = false;

        let self_mut: syn::FnArg = syn::parse_quote! { &mut self };
        let world: syn::Type = syn::parse_quote! { &vm::World };
        let world_mut: syn::Type = syn::parse_quote! { &mut vm::World };
        let value: syn::Type = syn::parse_quote! { vm::Value };
        let values: syn::Type = syn::parse_quote! { &[vm::Value] };

        let mut inputs = method.sig.decl.inputs.iter();
        match inputs.next() {
            Some(arg) if *arg == self_mut => {}
            _ => return Err(Error::new(method.sig.span(), "expected `&mut self`")),
        }
        while let Some(parameter) = inputs.next() {
            let ty = match *parameter {
                syn::FnArg::Captured(syn::ArgCaptured { ref ty, .. }) => ty,
                _ => return Err(Error::new(parameter.span(), "unsupported parameter")),
            };
            let parameter = match *ty {
                _ if *ty == world || *ty == world_mut => Parameter::World,
                _ if *ty == value => Parameter::Value,
                _ if *ty == values => Parameter::Variadic,
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
    member: syn::Ident,
    ident: syn::Ident,
    parameters: Vec<Parameter>,
}

impl Member {
    fn parse(attr: TokenStream, method: &syn::ImplItemMethod) -> syn::parse::Result<Self> {
        let attr: MemberName = syn::parse(attr)?;
        let member = attr.name;

        let ident = method.sig.ident.clone();
        let mut parameters = vec![];

        let self_ref: syn::FnArg = syn::parse_quote! { &self };
        let self_mut: syn::FnArg = syn::parse_quote! { &mut self };
        let world: syn::Type = syn::parse_quote! { &vm::World };
        let world_mut: syn::Type = syn::parse_quote! { &mut vm::World };
        let entity: syn::Type = syn::parse_quote! { vm::Entity };
        let index: syn::Type = syn::parse_quote! { usize };
        let value: syn::Type = syn::parse_quote! { vm::Value };

        let mut inputs = method.sig.decl.inputs.iter().peekable();
        match inputs.next() {
            Some(arg) if *arg == self_ref || *arg == self_mut => {}
            _ => return Err(Error::new(method.sig.span(), "expected `&self` or `&mut self`")),
        }
        match inputs.peek() {
            Some(&syn::FnArg::Captured(syn::ArgCaptured { ref ty, .. })) if
                *ty == world || *ty == world_mut
            => {
                parameters.push(Parameter::World);
                inputs.next();
            }
            _ => {},
        }
        match inputs.peek() {
            Some(&syn::FnArg::Captured(syn::ArgCaptured { ref ty, .. })) if *ty == entity => {
                parameters.push(Parameter::Entity);
                inputs.next();
            }
            _ => {},
        }
        match inputs.peek() {
            Some(&syn::FnArg::Captured(syn::ArgCaptured { ref ty, .. })) if *ty == index => {
                parameters.push(Parameter::Index);
                inputs.next();
            }
            _ => {},
        }
        match inputs.peek() {
            Some(&syn::FnArg::Captured(syn::ArgCaptured { ref ty, .. })) => {
                if *ty == value {
                    parameters.push(Parameter::Value);
                } else {
                    parameters.push(Parameter::Convert);
                }
                inputs.next();
            }
            _ => {},
        }
        if let Some(parameter) = inputs.next() {
            return Err(Error::new(parameter.span(), "unexpected parameter"));
        }

        Ok(Member { member, ident, parameters })
    }
}

struct MemberName {
    name: syn::Ident,
}

impl Parse for MemberName {
    fn parse(input: ParseStream) -> syn::parse::Result<Self> {
        let content;
        syn::parenthesized!(content in input);
        let name = content.parse()?;
        Ok(MemberName { name })
    }
}
