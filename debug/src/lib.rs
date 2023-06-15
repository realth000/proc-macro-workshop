use proc_macro::TokenStream;

use quote::quote;
use syn::{parse_macro_input, Data, DeriveInput, Expr, Fields, Lit, Meta};

macro_rules! compile_error {
    ($span: expr, $($arg: expr)*) => {
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

    let data_struct = if let Data::Struct(data_struct) = &ast.data {
        data_struct
    } else {
        return compile_error!(ident.span(), "invalid struct type");
    };
    let named_fields = if let Fields::Named(named_fields) = &data_struct.fields {
        named_fields
    } else {
        return TokenStream::new();
    };

    for named_field in named_fields.named.iter() {
        let mut debug_attr_format: Option<String> = None;
        for attr in &named_field.attrs {
            let p = attr.meta.path().segments.first();
            if p.is_none() || p.unwrap().ident != "debug" {
                continue;
            }

            // The following code may be a prettier version of nested `if let` expressions like this:
            //
            // ```
            // if let syn::Meta::NameValue(meta) = &attr.meta {
            //     if let syn::Expr::Lit(lit) = &meta.value {
            //         if let syn::Lit::Str(s) = &lit.lit {
            //             debug_attr_format =
            //                 Some(s.token().to_string().trim_matches('"').to_string());
            //             // eprintln!("lit: {}", s.token());
            //             // eprintln!("type:{:?}", debug_attr_format);
            //         }
            //     }
            // }
            // ```
            if let Meta::NameValue(syn::MetaNameValue {
                value:
                    Expr::Lit(syn::ExprLit {
                        lit: Lit::Str(ref s),
                        ..
                    }),
                ..
            }) = &attr.meta
            {
                debug_attr_format = Some(s.token().to_string().trim_matches('"').to_string());
                // eprintln!("lit: {}", s.token());
                // eprintln!("type:{:?}", debug_attr_format);
            }
        }
        let i = &named_field.ident.as_ref().unwrap();
        let s = ident_token_str!(i);

        let field_print = match debug_attr_format {
            Some(v) => {
                // Here use ident_token_str! macro rules to make an literal ident.
                let format_token = ident_token_str!(v);
                quote!(
                    .field(#s, &format_args!(#format_token, &self.#i))
                )
            }
            None => {
                quote!(
                    .field(#s, &self.#i)
                )
            }
        };

        field_vec.push(field_print);
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
