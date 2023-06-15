use proc_macro::TokenStream;

use quote::quote;
use syn::{parse_macro_input, Data, DeriveInput, Expr, Fields, Lit, Meta};

macro_rules! compile_error {
    ($span: expr, $($arg: tt)*) => {
        syn::Error::new($span, format!($($arg)*))
            .to_compile_error()
            .into()
    };
}

macro_rules! ident_token_str {
    ($ident: expr) => {
        format!("\"{}\"", $ident.to_string().as_str())
            .parse::<proc_macro2::TokenStream>()
            .unwrap()
    };
}

#[proc_macro_derive(CustomDebug, attributes(debug))]
pub fn derive(input: TokenStream) -> TokenStream {
    let ast = parse_macro_input!(input as DeriveInput);
    let ident = &ast.ident;
    let mut field_vec: Vec<proc_macro2::TokenStream> = vec![];
    if let Data::Struct(s) = &ast.data {
        if let Fields::Named(named_fields) = &s.fields {
            for named_field in named_fields.named.iter() {
                let mut debug_attr_format: Option<String> = None;
                for attr in &named_field.attrs {
                    let p = attr.meta.path().segments.first();
                    if p.is_none() || p.unwrap().ident != "debug" {
                        continue;
                    }
                    if let Meta::NameValue(meta) = &attr.meta {
                        if let Expr::Lit(lit) = &meta.value {
                            if let Lit::Str(s) = &lit.lit {
                                debug_attr_format =
                                    Some(s.token().to_string().trim_matches('"').to_string());
                                // eprintln!("lit: {}", s.token());
                                // eprintln!("type:{:?}", debug_attr_format);
                            }
                        }
                    }
                }
                let i = &named_field.ident.clone().unwrap();
                let s = ident_token_str!(i);
                if debug_attr_format.is_some() {
                    // Here use ident_token_str! macro rules to make an literal ident.
                    let format_token = ident_token_str!(debug_attr_format.as_ref().unwrap());
                    field_vec.push(quote!(
                        .field(#s, &format_args!(#format_token, &self.#i))
                    ));
                } else {
                    field_vec.push(quote!(
                        .field(#s, &self.#i)
                    ));
                }
            }
        }
    } else {
        return compile_error!(ident.span(), "invalid struct type");
    }
    let s = ident_token_str!(ident);
    quote!(
        impl std::fmt::Debug for #ident {
           fn fmt(&self, f: &mut std::fmt::Formatter<'_>)  -> std::fmt::Result {
                f.debug_struct(#s)
                #(#field_vec)*
                .finish()
            }
        }
    )
    .into()
}
