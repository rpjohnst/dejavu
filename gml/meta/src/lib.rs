extern crate proc_macro;

use std::iter;
use std::collections::HashMap;
use indexmap::IndexMap;
use proc_macro::TokenStream;
use proc_macro2;
use syn::{
    self, parse_quote, parenthesized, punctuated,
    ItemImpl, ImplItemMethod, Attribute, Signature, FnArg, PatType, ReturnType,
    Type, TypeReference, Path, Ident
};
use syn::parse::{Parse, ParseStream, Result, Error};
use syn::visit_mut::VisitMut;
use syn::spanned::Spanned;
use quote::quote;

struct ItemBindings {
    functions: Vec<Function>,
    members: HashMap<Ident, Member>,

    /// Ordered map from receiver types to their variable names.
    ///
    /// The order needs to be consistent because it determines the order of things in the output,
    /// like tuple type elements.
    receivers: IndexMap<Receiver, Ident>,
}

struct Function {
    name: Ident,
    receivers: Vec<Receiver>,
    parameters: Vec<Parameter>,
    rest: Option<()>,
    output: Return,
}

#[derive(Default)]
struct Member {
    getter: Option<Property>,
    setter: Option<Property>,
}

struct Property {
    name: Ident,
    receivers: Vec<Receiver>,
    index: Option<()>,
    value: Option<Parameter>,
}

/// A "receiver" type for a method or property.
#[derive(Clone, Eq, PartialEq, Hash)]
enum Receiver {
    /// A world or assets module (including the bound API's self type).
    Reference(Type),
    /// The GML-level `self` entity for the call.
    Entity,
}

#[derive(Copy, Clone)]
enum Parameter {
    Direct,
    Convert,
}

#[derive(Copy, Clone)]
enum Return {
    Value,
    Result,
}

impl ItemBindings {
    fn parse(item: &mut ItemImpl) -> std::result::Result<Self, Vec<Error>> {
        let mut bindings = ItemBindings {
            functions: Vec::new(),
            members: HashMap::new(),

            receivers: IndexMap::default(),
        };

        let mut errors = Vec::new();
        {
            let mut visit = VisitBindings {
                bindings: &mut bindings,
                errors: &mut errors,

                self_ty: (*item.self_ty).clone(),

                function: parse_quote!(gml::function),
                get: parse_quote!(gml::get),
                set: parse_quote!(gml::set),
            };
            syn::visit_mut::visit_item_impl_mut(&mut visit, item);
        }
        if !errors.is_empty() {
            return Err(errors);
        }

        Ok(bindings)
    }
}

struct VisitBindings<'a> {
    bindings: &'a mut ItemBindings,
    errors: &'a mut Vec<Error>,

    self_ty: Type,

    function: Path,
    get: Path,
    set: Path,
}

impl VisitMut for VisitBindings<'_> {
    fn visit_impl_item_method_mut(&mut self, item: &mut ImplItemMethod) {
        let sig = &item.sig;
        item.attrs.retain(|attr| !self.process_attribute(attr, &sig));
    }
}

impl VisitBindings<'_> {
    fn process_attribute(&mut self, attr: &Attribute, sig: &Signature) -> bool {
        if attr.path == self.function {
            let function = match Function::parse(&self.self_ty, sig) {
                Ok(function) => function,
                Err(err) => {
                    self.errors.push(err);
                    return true;
                }
            };

            self.extend_receivers(&function.receivers[..]);

            self.bindings.functions.push(function);
            true
        } else if attr.path == self.get || attr.path == self.set {
            let meta: PropertyMeta = match syn::parse2(attr.tokens.clone()) {
                Ok(meta) => meta,
                Err(err) => {
                    self.errors.push(err);
                    return true;
                }
            };
            let property = match Property::parse(&self.self_ty, sig) {
                Ok(property) => property,
                Err(err) => {
                    self.errors.push(err);
                    return true;
                }
            };

            self.extend_receivers(&property.receivers[..]);

            let member = self.bindings.members.entry(meta.name.clone()).or_default();
            if attr.path == self.get {
                if member.getter.is_some() {
                    let message = "getter is defined multiple times";
                    self.errors.push(Error::new(meta.name.span(), message));
                    return true;
                }
                member.getter = Some(property);
            } else if attr.path == self.set {
                if member.setter.is_some() {
                    let message = "setter is defined multiple times";
                    self.errors.push(Error::new(meta.name.span(), message));
                    return true;
                }
                member.setter = Some(property);
            }
            true
        } else {
            false
        }
    }

    fn extend_receivers(&mut self, receivers: &[Receiver]) {
        for receiver in receivers {
            let id = self.bindings.receivers.len();
            let span = proc_macro2::Span::call_site();
            self.bindings.receivers.entry(receiver.clone()).or_insert_with(move || match receiver {
                Receiver::Reference(_) => Ident::new(&format!("receiver_{}", id), span),
                Receiver::Entity => Ident::new("entity", span),
            });
        }
    }
}

struct PropertyMeta {
    name: Ident,
}

impl Parse for PropertyMeta {
    fn parse(input: ParseStream<'_>) -> Result<Self> {
        let content;
        parenthesized!(content in input);
        Ok(PropertyMeta { name: content.parse()? })
    }
}

impl Function {
    fn parse(self_ty: &Type, sig: &Signature) -> Result<Self> {
        let name = sig.ident.clone();
        let mut inputs = sig.inputs.iter().peekable();

        let receivers = parse_receivers(self_ty, &mut inputs);
        let mut parameters = Vec::new();
        let mut rest = None;

        let value = parse_quote!(vm::ValueRef);
        let values = parse_quote!(&[vm::Value]);
        while let Some(&param) = inputs.peek() {
            match *param {
                FnArg::Typed(PatType { ref ty, .. }) if *ty == value =>
                    parameters.push(Parameter::Direct),

                FnArg::Typed(PatType { ref ty, .. }) if *ty == values => {
                    rest = Some(());
                    inputs.next();
                    break;
                }

                _ => parameters.push(Parameter::Convert),
            }
            inputs.next();
        }

        if let Some(param) = inputs.next() {
            return Err(Error::new(param.span(), "unexpected parameter"));
        }

        let result: Ident = parse_quote!(Result);
        let output = match sig.output {
            ReturnType::Default => Return::Value,
            ReturnType::Type(_, ref ty) => match **ty {
                Type::Path(ref ty) if ty.path.segments[0].ident == result => Return::Result,
                _ => Return::Value,
            },
        };

        Ok(Function { name, receivers, parameters, rest, output })
    }
}

impl Property {
    fn parse(self_ty: &Type, sig: &Signature) -> Result<Self> {
        let name = sig.ident.clone();
        let mut inputs = sig.inputs.iter().peekable();

        let receivers = parse_receivers(self_ty, &mut inputs);
        let mut index = None;
        let mut value = None;

        let usize_ty = parse_quote!(usize);
        while let Some(&param) = inputs.peek() {
            match *param {
                FnArg::Typed(PatType { ref ty, .. }) if *ty == usize_ty =>
                    index = Some(()),

                _ => break,
            }
            inputs.next();
        }

        let value_ty = parse_quote!(vm::ValueRef);
        while let Some(&param) = inputs.peek() {
            match *param {
                FnArg::Typed(PatType { ref ty, .. }) if *ty == value_ty =>
                    value = Some(Parameter::Direct),

                FnArg::Typed(PatType { .. }) =>
                    value = Some(Parameter::Convert),

                _ => break,
            }
            inputs.next();
            break;
        }

        if let Some(param) = inputs.next() {
            return Err(Error::new(param.span(), "unexpected parameter"));
        }

        Ok(Property { name, receivers, index, value })
    }
}

fn parse_receivers(
    self_ty: &Type, inputs: &mut iter::Peekable<punctuated::Iter<'_, FnArg>>
) -> Vec<Receiver> {
    let mut receivers = Vec::default();

    let entity = parse_quote!(vm::Entity);
    while let Some(&param) = inputs.peek() {
        match *param {
            FnArg::Receiver(_) => {
                receivers.push(Receiver::Reference(self_ty.clone()));
            }

            FnArg::Typed(PatType { ref ty, .. }) if **ty == entity => {
                receivers.push(Receiver::Entity);
            }

            FnArg::Typed(PatType { ref ty, .. }) => {
                let target = match **ty {
                    Type::Reference(TypeReference { ref elem, .. }) => elem,
                    _ => break,
                };
                match **target {
                    Type::Path(_) => {}
                    _ => break
                };

                receivers.push(Receiver::Reference((**target).clone()));
            }
        }
        inputs.next();
    }

    receivers
}

#[proc_macro_attribute]
pub fn bind(attr: TokenStream, input: TokenStream) -> TokenStream {
    // Parse the impl and collect method attributes.

    let trait_name: Ident = syn::parse_macro_input!(attr);
    let mut input: ItemImpl = syn::parse_macro_input!(input);
    let bindings = match ItemBindings::parse(&mut input) {
        Ok(bindings) => bindings,
        Err(errors) => {
            let errors: proc_macro2::TokenStream = errors.iter()
                .flat_map(Error::to_compile_error)
                .collect();
            return TokenStream::from(errors);
        }
    };

    // Generate the API glue trait.

    let self_ty = &input.self_ty;
    let receiver_tys = bindings.receivers.iter().filter_map(|(receiver, _)| match *receiver {
        Receiver::Reference(ref ty) => Some(quote! { #ty }),
        _ => None,
    });
    let receiver_idents = {
        let receivers = bindings.receivers.iter().filter_map(|(receiver, ident)| match *receiver {
            Receiver::Reference(_) => Some(ident),
            _ => None,
        });
        quote! { #(#receivers,)* }
    };

    let api = bindings.functions.iter().map(|function| &function.name);
    let api_binding = api.clone();
    let api_arity = bindings.functions.iter().map(|function| function.parameters.len());
    let api_variadic = bindings.functions.iter().map(|function| function.rest.is_some());
    let api_receivers = bindings.functions.iter().map(|function| {
        let receivers = function.receivers.iter().map(|receiver| &bindings.receivers[receiver]);
        quote! { #(#receivers,)* }
    });
    let api_arguments = bindings.functions.iter().map(|function| {
        let api_arguments = function.parameters.iter().enumerate().map(|(i, &param)| match param {
            Parameter::Direct => quote! { arguments[#i].borrow() },
            Parameter::Convert => quote! { arguments[#i].borrow().try_into().unwrap_or_default() },
        });
        quote! { #(#api_arguments,)* }
    });
    let api_rest = bindings.functions.iter().map(|function| {
        let arity = function.parameters.len();
        let rest = function.rest.iter().map(|()| quote! { &arguments[#arity..] });
        quote! { #(#rest,)* }
    });
    let api_try = bindings.functions.iter().map(|function| {
        match function.output {
            Return::Value => None,
            Return::Result => Some(quote! { ? }),
        }
    });

    let member = bindings.members.iter().map(|(name, _)| name);

    let getter = bindings.members.iter().map(|(_, member)| member.getter.as_ref());
    let get_option = getter.clone().map(|getter| getter.map_or_else(
        || quote! { None },
        |&Property { ref name, .. }| quote! { Some(Self::#name) },
    ));
    let get = getter.clone().flatten().map(|getter| &getter.name);
    let get_receivers = getter.clone().flatten().map(|getter| {
        let receivers = getter.receivers.iter().map(|receiver| &bindings.receivers[receiver]);
        quote! { #(#receivers,)* }
    });
    let get_index = getter.clone().flatten().map(|getter| {
        let index = getter.index.iter().map(|()| quote! { index });
        quote! { #(#index,)* }
    });

    let setter = bindings.members.iter().map(|(_, member)| member.setter.as_ref());
    let set_option = setter.clone().map(|setter| setter.map_or_else(
        || quote! { None },
        |&Property { ref name, .. }| quote! { Some(Self::#name) },
    ));
    let set = setter.clone().flatten().map(|setter| &setter.name);
    let set_receivers = setter.clone().flatten().map(|setter| {
        let receivers = setter.receivers.iter().map(|receiver| &bindings.receivers[receiver]);
        quote! { #(#receivers,)* }
    });
    let set_index = setter.clone().flatten().map(|setter| {
        let index = setter.index.iter().map(|()| quote! { index });
        quote! { #(#index,)* }
    });
    let set_value = setter.clone().flatten().map(|setter| {
        setter.value.map(|param| match param {
            Parameter::Direct => quote! { value },
            Parameter::Convert => quote! { value.try_into().unwrap_or_default() },
        })
    });

    let output = quote! {
        pub trait #trait_name<'a, A: 'a> {
            fn fields<'r>(&'r mut self, assets: &'r mut A) -> (#(&'r mut #receiver_tys,)*);

            fn register(
                items: &mut std::collections::HashMap<gml::symbol::Symbol, gml::Item<Self, A>>
            ) where
                Self: Sized
            {
                #(items.insert(
                    gml::symbol::Symbol::intern(stringify!(#api_binding).as_bytes()),
                    gml::Item::Native(Self::#api_binding, #api_arity, #api_variadic),
                );)*

                #(items.insert(
                    gml::symbol::Symbol::intern(stringify!(#member).as_bytes()),
                    gml::Item::Member(#get_option, #set_option),
                );)*
            }

            #(unsafe fn #api(
                &mut self, assets: &mut A,
                thread: &mut vm::Thread, arguments: std::ops::Range<usize>,
            ) -> Result<vm::Value, Box<vm::Error>> {
                #![allow(unused_imports, unused)]
                use std::convert::TryInto;

                let (#receiver_idents) = #trait_name::fields(self, assets);
                let entity = thread.self_entity();
                let arguments = thread.arguments(arguments);
                let ret = #self_ty::#api(#api_receivers #api_arguments #api_rest) #api_try;
                Ok(ret.into())
            })*

            #(fn #get(&mut self, assets: &mut A, entity: vm::Entity, index: usize) -> vm::Value {
                #![allow(unused)]

                let (#receiver_idents) = #trait_name::fields(self, assets);
                let value = #self_ty::#get(#get_receivers #get_index);
                value.into()
            })*

            #(fn #set(&mut self, assets: &mut A, entity: vm::Entity, index: usize, value: vm::ValueRef) {
                #![allow(unused_imports, unused)]
                use std::convert::TryInto;

                let (#receiver_idents) = #trait_name::fields(self, assets);
                #self_ty::#set(#set_receivers #set_index #set_value);
            })*
        }

        #input
    };
    output.into()
}
