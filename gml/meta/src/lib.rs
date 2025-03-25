use std::iter;
use std::collections::HashMap;
use proc_macro::TokenStream;
use syn::{
    self, parse_quote, punctuated,
    ItemImpl, ImplItemFn, Attribute, Signature, FnArg, PatType,
    Type, TypeReference, Path, Ident
};
use syn::parse::{Result, Error};
use syn::visit_mut::VisitMut;
use quote::quote;

#[derive(Default)]
struct ItemBindings {
    apis: Vec<Function>,
    fields: HashMap<Ident, Field>,
}

#[derive(Default)]
struct Field {
    getter: Option<Function>,
    setter: Option<Function>,
}

struct Function {
    name: Ident,
    receivers: Vec<Type>,
}

impl ItemBindings {
    fn parse(item: &mut ItemImpl) -> std::result::Result<Self, Vec<Error>> {
        let mut bindings = ItemBindings::default();

        let mut errors = Vec::new();
        {
            let mut visit = VisitBindings {
                bindings: &mut bindings,
                errors: &mut errors,

                self_ty: (*item.self_ty).clone(),

                api: parse_quote!(gml::api),
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

    api: Path,
    get: Path,
    set: Path,
}

impl VisitMut for VisitBindings<'_> {
    fn visit_impl_item_fn_mut(&mut self, item: &mut ImplItemFn) {
        let sig = &item.sig;
        item.attrs.retain(|attr| !self.process_attribute(attr, &sig));
    }
}

impl VisitBindings<'_> {
    fn process_attribute(&mut self, attr: &Attribute, sig: &Signature) -> bool {
        if *attr.path() == self.api {
            match Function::parse(&self.self_ty, sig) {
                Ok(function) => { self.bindings.apis.push(function); }
                Err(err) => { self.errors.push(err); }
            }
            true
        } else if *attr.path() == self.get || *attr.path() == self.set {
            let (span, member) = match attr.parse_args::<Ident>() {
                Ok(name) => {
                    (name.span(), self.bindings.fields.entry(name).or_default())
                }
                Err(err) => {
                    self.errors.push(err);
                    return true;
                }
            };
            let field = if *attr.path() == self.get {
                if member.getter.is_some() {
                    self.errors.push(Error::new(span, "getter is defined multiple times"));
                    return true;
                }
                &mut member.getter
            } else {
                if member.setter.is_some() {
                    self.errors.push(Error::new(span, "setter is defined multiple times"));
                    return true;
                }
                &mut member.setter
            };

            match Function::parse(&self.self_ty, sig) {
                Ok(function) => { *field = Some(function); }
                Err(err) => { self.errors.push(err); }
            }
            true
        } else {
            false
        }
    }
}

impl Function {
    fn parse(self_ty: &Type, sig: &Signature) -> Result<Self> {
        let name = sig.ident.clone();
        let mut inputs = sig.inputs.iter().peekable();

        let receivers = parse_receivers(self_ty, &mut inputs)?;

        Ok(Function { name, receivers })
    }
}

fn parse_receivers(
    self_ty: &Type, inputs: &mut iter::Peekable<punctuated::Iter<'_, FnArg>>
) -> Result<Vec<Type>> {
    let mut receivers = Vec::default();

    let thread = parse_quote!(&mut vm::Thread);
    while let Some(&param) = inputs.peek() {
        match *param {
            FnArg::Typed(PatType { ref ty, .. }) if **ty == thread => break,

            FnArg::Receiver(_) => {
                receivers.push(self_ty.clone());
            }

            _ => match param_reference_type(param) {
                Some(target) => { receivers.push(target.clone()); }
                None => { break; }
            }
        }
        inputs.next();
    }

    Ok(receivers)
}

fn param_reference_type(param: &FnArg) -> Option<&Type> {
    match *param {
        FnArg::Typed(PatType { ref ty, .. }) => match **ty {
            Type::Reference(TypeReference { ref elem, .. }) => match **elem {
                Type::Path(_) => Some(&**elem),
                _ => None,
            }
            _ => None,
        }
        _ => None,
    }
}

#[proc_macro_attribute]
pub fn bind(_attr: TokenStream, input: TokenStream) -> TokenStream {
    let output = proc_macro2::TokenStream::from(input.clone());

    // Parse the impl and collect method attributes.

    let mut item: ItemImpl = syn::parse_macro_input!(input);
    let bindings = match ItemBindings::parse(&mut item) {
        Ok(bindings) => bindings,
        Err(errors) => {
            let errors: proc_macro2::TokenStream = errors.iter()
                .flat_map(Error::to_compile_error)
                .collect();
            return TokenStream::from(errors);
        }
    };

    // Generate the API glue.

    let self_ty = &item.self_ty;

    let api_binding = bindings.apis.iter().map(|function| &function.name);
    let api_context1 = bindings.apis.iter().map(|function| {
        let receivers = function.receivers.iter();
        quote! { #(&'r mut #receivers,)* }
    });
    let api_context2 = api_context1.clone();
    let api_context3 = bindings.apis.iter().map(|function| {
        let receivers = function.receivers.iter();
        quote! { #(&mut #receivers,)* }
    });

    let member = bindings.fields.iter().map(|(name, _)| name);

    let getter = bindings.fields.iter().map(|(_, member)| member.getter.as_ref());
    let get_binding = getter.clone().map(|getter| getter.map_or_else(
        || quote! { None },
        |&Function { ref name, .. }| quote! {
            Some(|cx: &mut W, entity, i| {
                let bind = vm::Bind(#self_ty::#name, std::marker::PhantomData);
                vm::GetBind::call(bind, cx, entity, i)
            })
        },
    ));
    let get_context = getter.clone().flatten().map(|getter| {
        let receivers = getter.receivers.iter();
        quote! { #(&'r mut #receivers,)* }
    });

    let setter = bindings.fields.iter().map(|(_, member)| member.setter.as_ref());
    let set_binding = setter.clone().map(|setter| setter.map_or_else(
        || quote! { None },
        |&Function { ref name, .. }| quote! {
            Some(|cx: &mut W, entity, i, value| {
                let bind = vm::Bind(#self_ty::#name, std::marker::PhantomData);
                vm::SetBind::call(bind, cx, entity, i, value);
            })
        }
    ));
    let set_context = setter.clone().flatten().map(|setter| {
        let receivers = setter.receivers.iter();
        quote! { #(&'r mut #receivers,)* }
    });

    TokenStream::from(quote! {
        #output

        impl #self_ty {
            pub fn register<W>(
                items: &mut std::collections::HashMap<gml::symbol::Symbol, gml::Item<W>>
            ) where
                #(W: for<'r> gml::vm::Project<'r, (#api_context1)>,)*
                #(W: for<'r> gml::vm::Project<'r, (#get_context)>,)*
                #(W: for<'r> gml::vm::Project<'r, (#set_context)>,)*
            {
                use gml::{symbol::Symbol, vm};
                use std::marker::PhantomData;

                #({
                    // Inferring the types here is the slowest part of compiling this function.
                    // Give the compiler as much help as possible by specifying what we do know.
                    fn bind<'t, W>() -> impl vm::FnBind<'t, W> where
                        W: for<'r> vm::Project<'r, (#api_context2)>
                    { vm::Bind::<_, (#api_context3), _, _>(#self_ty::#api_binding, PhantomData) }

                    let symbol = Symbol::intern(stringify!(#api_binding).as_bytes());
                    let api = |cx: &mut W, thread: &mut vm::Thread, range| unsafe {
                        vm::FnBind::call(bind(), cx, thread, range)
                    };
                    let bind = bind();
                    let arity = vm::bind::arity::<_, W>(&bind);
                    let variadic = vm::bind::variadic::<_, W>(&bind);
                    let item = gml::Item::Native(api, arity, variadic);
                    items.insert(symbol, item);
                })*

                #({
                    let symbol = Symbol::intern(stringify!(#member).as_bytes());
                    let item = gml::Item::Member(#get_binding, #set_binding);
                    items.insert(symbol, item);
                })*
            }
        }
    })
}

#[proc_macro_attribute]
pub fn api(_attr: TokenStream, input: TokenStream) -> TokenStream { input }
#[proc_macro_attribute]
pub fn get(_attr: TokenStream, input: TokenStream) -> TokenStream { input }
#[proc_macro_attribute]
pub fn set(_attr: TokenStream, input: TokenStream) -> TokenStream { input }
