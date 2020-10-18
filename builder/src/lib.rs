extern crate proc_macro;

use proc_macro2::{Ident, Span};
use quote::{quote, quote_spanned};
use syn::spanned::Spanned;
use syn::{parse_macro_input, Data, DeriveInput, Fields, Type};

#[proc_macro_derive(Builder, attributes(builder))]
pub fn derive(input: proc_macro::TokenStream) -> proc_macro::TokenStream {
    // Parse the input tokens into a syntax tree.
    let input = parse_macro_input!(input as DeriveInput);

    // Used in the quasi-quotation below as `#name`.
    let ident = input.ident;
    let vis = input.vis;
    // let attrs = input.attrs;

    let fields = if let Data::Struct(syn::DataStruct {
        fields: Fields::Named(ref fields),
        ..
    }) = input.data
    {
        fields
    } else {
        unimplemented!()
    };

    let builder_ty = Ident::new(&format!("{}Builder", ident), Span::call_site());

    let builder_fields = {
        let builder_field = fields.named.iter().map(|f| {
            let name = &f.ident;
            let ty = &inner_for_option(&f.ty).unwrap_or_else(|| f.ty.clone());
            quote_spanned! {f.span()=>
                #name: Option<#ty>,
            }
        });
        quote! {
            #(#builder_field)*
        }
    };

    let each_setters = {
        let mut res = std::collections::HashMap::new();
        fields
            .named
            .iter()
            .filter(|&f| !f.attrs.is_empty())
            .for_each(|f| {
                let setter_attr = f.attrs.get(0).unwrap();
                let meta = setter_attr.parse_meta().unwrap();
                if let syn::Meta::List(syn::MetaList { nested, .. }) = meta {
                    if let Some(syn::NestedMeta::Meta(syn::Meta::NameValue(syn::MetaNameValue {
                        lit: syn::Lit::Str(each_setter_name),
                        ..
                    }))) = nested.first()
                    {
                        let each_setter_name = each_setter_name.value();
                        res.insert(each_setter_name, f);
                    }
                }
            });
        res
    };

    let builder_init = {
        let init_field = fields.named.iter().map(|f| {
            let name = &f.ident;
            if each_setters.values().any(|&f| f.ident == *name) {
                quote_spanned! {f.span()=>
                    #name: Some(vec![]),
                }
            } else {
                quote_spanned! {f.span()=>
                    #name: None,
                }
            }
        });
        quote! {
            #builder_ty {
                #(#init_field)*
            }
        }
    };

    let builder_impl = {
        let build_fn_body = {
            let check = fields
                .named
                .iter()
                .filter(|&f| inner_for_option(&f.ty).is_none())
                .map(|f| {
                    let name = &f.ident;
                    let name_str = format!("{}", name.clone().unwrap());
                    quote_spanned! {f.span()=>
                        if let None = self.#name {
                            return Err(format!("The field `{}` is not set yet, building failed", #name_str).into());
                        }
                    }
                });
            let field = fields.named.iter().map(|f| {
                let name = &f.ident;
                if inner_for_option(&f.ty).is_some() {
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

                Ok(#ident {
                    #(#field)*
                })
            }
        };

        let each_setter = each_setters.iter().map(|(each_setter_name, &f)| {
            let ty = &inner_for_vec(&f.ty).unwrap();
            let name = &f.ident;

            let each_setter_name = Ident::new(&each_setter_name, Span::call_site());

            quote_spanned! {f.span()=>
                fn #each_setter_name(&mut self, #name: #ty) -> &mut Self {
                    match &mut self.#name {
                        Some(v) => {
                            v.push(#name);
                            // println!("Got vec: {:?}", v);
                        },
                        None => {
                            let _out = std::mem::replace(&mut self.#name, Some(vec![#name]));
                            // println!("Swapped out: {:?}", _out);
                        },
                    }
                    self
                }
            }
        });

        let simple_setter = fields.named.iter().map(|f| {
            let name = &f.ident;
            if each_setters
                .keys()
                .any(|each_setter| each_setter == &format!("{}", name.clone().unwrap()))
            {
                quote! {}
            } else {
                let ty = &inner_for_option(&f.ty).unwrap_or_else(|| f.ty.clone());
                quote_spanned! {f.span()=>
                    fn #name(&mut self, #name: #ty) -> &mut Self {
                        self.#name = Some(#name);
                        self
                    }
                }
            }
        });

        quote! {
            #(#simple_setter)*

            #(#each_setter)*

            #vis fn build(&mut self) -> Result<#ident, Box<dyn std::error::Error>> {
                #build_fn_body
            }
        }
    };

    let expanded = quote! {
        impl #ident {
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

fn inner_for(container: &str, ty: &Type) -> Option<Type> {
    match ty {
        Type::Path(syn::TypePath {
            path: syn::Path { segments, .. },
            ..
        }) if segments[0].ident == container => {
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

fn inner_for_option(ty: &Type) -> Option<Type> {
    inner_for("Option", ty)
}

fn inner_for_vec(ty: &Type) -> Option<Type> {
    inner_for("Vec", ty)
}
