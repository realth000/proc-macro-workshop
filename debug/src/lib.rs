use proc_macro::TokenStream;

use quote::quote;
use syn::{parse_macro_input, Data, DeriveInput, Fields};

macro_rules! compile_error {
    ($span: expr, $($arg:tt,)*) => {
        syn::Error::new($span, format!($($arg)*))
            .to_compile_error()
            .into()
    };
    ($span: expr, $arg:tt) => {
        syn::Error::new($span, format!($arg))
            .to_compile_error()
            .into()
    }
}

macro_rules! ident_token_str {
    ($ident: tt) => {
        format!("\"{}\"", $ident.to_string().as_str())
            .parse::<proc_macro2::TokenStream>()
            .unwrap()
    };
}

#[proc_macro_derive(CustomDebug)]
pub fn derive(input: TokenStream) -> TokenStream {
    let ast = parse_macro_input!(input as DeriveInput);
    let ident = &ast.ident;
    let mut field_vec: Vec<proc_macro2::TokenStream> = vec![];
    if let Data::Struct(s) = &ast.data {
        if let Fields::Named(named_fields) = &s.fields {
            for named_field in named_fields.named.iter() {
                let i = &named_field.ident.clone().unwrap();
                let s = ident_token_str!(i);
                field_vec.push(quote!(
                    .field(#s, &self.#i)
                ));
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
