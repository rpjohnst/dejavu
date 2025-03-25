use proc_macro::TokenStream;
use syn::{parse_macro_input, Error, DeriveInput, Data, DataStruct, Field, Fields, Visibility};
use quote::quote;

#[proc_macro_derive(Reflect)]
pub fn derive_type_layout(input: TokenStream) -> TokenStream {
    let DeriveInput { ident, generics, data, .. } = parse_macro_input!(input);
    let (impl_generics, type_generics, where_clause) = generics.split_for_impl();
    match data {
        Data::Struct(DataStruct { fields: Fields::Named(fields), .. }) => {
            let fields = fields.named.iter().flat_map(|Field { vis, ident: field, ty, .. }| {
                let Visibility::Public(_) = vis else { return None };
                let Some(field) = field else { unreachable!() };
                Some(quote! {
                    wasm::Field {
                        name: stringify!(#field).as_bytes(),
                        offset: core::mem::offset_of!(#ident, #field),
                        layout: &<#ty as wasm::Reflect>::LAYOUT,
                    }
                })
            });
            TokenStream::from(quote! {
                impl #impl_generics wasm::Reflect for #ident #type_generics #where_clause {
                    const LAYOUT: wasm::Layout = wasm::Layout::Struct {
                        fields: &[#(#fields),*],
                    };
                }
            })
        }
        _ => Error::new_spanned(ident, "Only structs with named fields are supported")
            .to_compile_error()
            .into()
    }
}
