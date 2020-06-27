extern crate proc_macro;

use proc_macro2::{Ident, Span};
use quote::{quote, quote_spanned};
use syn::spanned::Spanned;
use syn::{parse_macro_input, Data, DeriveInput, Fields, Type};

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
            let ty = &inner_for_option(&f.ty).unwrap_or_else(|| f.ty.clone());
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

    let build_fn_body = {
        let check = fields
            .named
            .iter()
            .filter(|f| inner_for_option(&f.ty).is_none())
            .map(|f| {
                let name = &f.ident;
                quote_spanned! {f.span()=>
                    if let None = self.#name {
                        return Err("The fields are not completely set yet, building failed".into());
                    }
                }
            });
        let field = fields.named.iter().map(|f| {
            let name = &f.ident;
            if let Some(_) = inner_for_option(&f.ty) {
                quote_spanned! {f.span()=>
                    #name: std::mem::replace(&mut self.#name, None),
                }
            } else {
                quote_spanned! {f.span()=>
                    #name: std::mem::replace(&mut self.#name, None).unwrap(),
                }
            }
        });
        quote! {
            #(#check)*

            Ok(#name {
                #(#field)*
            })
        }
    };

    let builder_impl = {
        let recurse = fields.named.iter().map(|f| {
            let name = &f.ident;
            let ty = &inner_for_option(&f.ty).unwrap_or_else(|| f.ty.clone());
            quote_spanned! {f.span()=>
                fn #name(&mut self, #name: #ty) -> &mut Self {
                    self.#name = Some(#name);
                    self
                }
            }
        });
        quote! {
            #(#recurse)*

            #vis fn build(&mut self) -> Result<#name, Box<dyn std::error::Error>> {
                #build_fn_body
            }
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
        }
    };

    // eprintln!("{:#?}", expanded);

    // Hand the output tokens back to the compiler.
    proc_macro::TokenStream::from(expanded)
}

fn inner_for_option(ty: &Type) -> Option<Type> {
    match ty {
        Type::Path(syn::TypePath {
            path: syn::Path { segments, .. },
            ..
        }) if segments[0].ident == "Option" => {
            let segment = &segments[0];

            match &segment.arguments {
                syn::PathArguments::AngleBracketed(generic) => {
                    match generic.args.first().unwrap() {
                        syn::GenericArgument::Type(ty) => Some(ty.clone()),
                        _ => None,
                    }
                }
                _ => None,
            }
        }

        _ => None,
    }
}
