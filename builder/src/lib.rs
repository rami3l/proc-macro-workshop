extern crate proc_macro;

use proc_macro2::{Ident, Span, TokenStream};
use quote::{quote, quote_spanned};
use syn::spanned::Spanned;
use syn::{
    parse_macro_input, parse_quote, Data, DeriveInput, Fields, GenericParam, Generics, Index,
};

#[proc_macro_derive(Builder)]
pub fn derive(input: proc_macro::TokenStream) -> proc_macro::TokenStream {
    // Parse the input tokens into a syntax tree.
    let input = parse_macro_input!(input as DeriveInput);

    // Used in the quasi-quotation below as `#name`.
    let name = input.ident;
    let vis = input.vis;
    let fields = if let Data::Struct(syn::DataStruct {
        fields: Fields::Named(ref fields),
        ..
    }) = input.data
    {
        fields
    } else {
        unimplemented!()
    };

    let builder_ty = Ident::new(&format!("{}Builder", name), Span::call_site());

    let builder_fields = {
        let recurse = fields.named.iter().map(|f| {
            let name = &f.ident;
            let ty = &f.ty;
            quote_spanned! {f.span()=>
                #name: Option<#ty>,
            }
        });
        quote! {
            #(#recurse)*
        }
    };

    let builder_init = {
        let recurse = fields.named.iter().map(|f| {
            let name = &f.ident;
            quote_spanned! {f.span()=>
                #name: None,
            }
        });
        quote! {
            #builder_ty {
                #(#recurse)*
            }
        }
    };

    let builder_impl = {
        let recurse = fields.named.iter().map(|f| {
            let name = &f.ident;
            let ty = &f.ty;
            quote_spanned! {f.span()=>
                fn #name(&mut self, #name: #ty) -> &mut Self {
                    self.#name = Some(#name);
                    self
                }
            }
        });
        quote! {
            #(#recurse)*
        }
    };

    let build_fn_body = {
        let check = fields.named.iter().map(|f| {
            let name = &f.ident;
            quote_spanned! {f.span()=>
                if self.#name.is_none() {
                    return Err("The fields are not completely set yet, building failed".into());
                }
            }
        });
        let field = fields.named.iter().map(|f| {
            let name = &f.ident;
            quote_spanned! {f.span()=>
                #name: self.#name.unwrap(),
            }
        });
        quote! {
            #(#check)*

            Ok(#name {
                #(#field)*
            })
        }
    };

    let expanded = quote! {
        impl #name {
            #vis fn builder() -> #builder_ty {
                #builder_init
            }
        }

        #vis struct #builder_ty {
            #builder_fields
        }

        impl #builder_ty {
            #builder_impl

            #vis fn build(self) -> Result<#name, Box<dyn std::error::Error>> {
                #build_fn_body
            }
        }
    };

    // Hand the output tokens back to the compiler.
    proc_macro::TokenStream::from(expanded)
}
