extern crate proc_macro;

use proc_macro2::{Ident, Span};
use quote::{quote, quote_spanned, ToTokens};
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

    let field_ty = Ident::new(&format!("{}Field", name), Span::call_site());

    let field_ty_impl = quote! {
        #vis enum #field_ty<T> {
            Optional(T),
            Required(T),
        }
    };

    let builder_ty = Ident::new(&format!("{}Builder", name), Span::call_site());

    let builder_fields = {
        let recurse = fields.named.iter().map(|f| {
            let name = &f.ident;
            let ty = &inner_for_option(&f.ty).unwrap_or_else(|| f.ty.clone());
            quote_spanned! {f.span()=>
                #name: #field_ty<Option<#ty>>,
            }
        });
        quote! {
            #(#recurse)*
        }
    };

    let builder_init = {
        let recurse = fields.named.iter().map(|f| {
            let name = &f.ident;
            let ty = &f.ty;
            let is_optional = {
                let ty_str = ty.to_token_stream().to_string();
                ty_str.starts_with("Option")
            };
            let field = if is_optional {
                quote!(#field_ty::Optional(None))
            } else {
                quote!(#field_ty::Required(None))
            };

            quote_spanned! {f.span()=>
                #name: #field,
            }
        });
        quote! {
            #builder_ty {
                #(#recurse)*
            }
        }
    };

    let build_fn_body = {
        let check = fields.named.iter().map(|f| {
            let name = &f.ident;
            quote_spanned! {f.span()=>
                if let #field_ty::Required(None) = self.#name {
                    return Err("The fields are not completely set yet, building failed".into());
                }
            }
        });
        let field = fields.named.iter().map(|f| {
            let name = &f.ident;
            if let Some(_) = inner_for_option(&f.ty) {
                quote_spanned! {f.span()=>
                    #name: match self.#name {
                        #field_ty::Optional(ref mut mx) => std::mem::replace(mx, None),
                        _ => unimplemented!(),
                    },
                }
            } else {
                quote_spanned! {f.span()=>
                    #name: match self.#name {
                        #field_ty::Required(ref mut mx) => std::mem::replace(mx, None).unwrap(),
                        _ => unimplemented!(),
                    },
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
                    let field = match self.#name {
                        #field_ty::Optional(_) => #field_ty::Optional(Some(#name)),
                        #field_ty::Required(_) => #field_ty::Required(Some(#name)),
                    };
                    self.#name = field;
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
        #field_ty_impl

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
