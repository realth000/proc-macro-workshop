use proc_macro::{TokenStream, TokenTree};

use proc_macro2::{Ident, Span};
use quote::{quote, ToTokens};
use syn::{parse_str, ExprBlock};

macro_rules! compile_error {
    ($span: expr, $($arg: tt)*) => {
        syn::Error::new($span, format!($($arg)*))
            .to_compile_error()
            .into()
    };
}

#[proc_macro]
pub fn seq(input: TokenStream) -> TokenStream {
    let token_vec: Vec<(TokenTree, String)> = input
        .into_iter()
        .map(|tt| {
            let s = tt.to_string();
            (tt, String::from(s.trim_matches('"')))
        })
        .collect();

    // Token should start with the following tt:
    // 0. Ident: variable name, usually is `N`.
    // 1. Ident `in`.
    // 2. Literal integer: start value.
    // 3. Punct char: '.'.
    // 4. Punct char: '.'.
    // 5. Literal integer: end value.
    // 6. Group: stream: TokenStream[..];
    // `token_vec`'s length is always >= 7 otherwise fail to compile.

    let variable_name = Ident::new(&token_vec[0].1, Span::from(token_vec[0].0.span()));
    let start_value = match token_vec[2].1.parse::<i32>() {
        // Ok(_) => Ident::new(token_vec[2].1.as_str(), Span::from(token_vec[2].0.span())),
        Ok(v) => v,
        Err(e) => {
            return compile_error!(
                proc_macro2::Span::from(token_vec[2].0.span()),
                "failed to parse {} as i32 type: {}",
                token_vec[2].1,
                e
            );
        }
    };
    let end_value_str = &token_vec[5].1;
    let end_value = match end_value_str.parse::<i32>() {
        Ok(v) => v,
        Err(e) => {
            return compile_error!(
                proc_macro2::Span::from(token_vec[5].0.span()),
                "failed to parse as i32 type: {}",
                e
            );
        }
    };

    let loop_body = match parse_str::<ExprBlock>(&token_vec[6].1) {
        Ok(ExprBlock { block, .. }) => match first_group(block.to_token_stream()) {
            Some(group) => group,
            None => {
                return compile_error!(
                    proc_macro2::Span::from(token_vec[6].0.span()),
                    "failed to parse code block group"
                );
            }
        },
        _ => {
            return compile_error!(
                proc_macro2::Span::from(token_vec[6].0.span()),
                "failed to parse code as block"
            );
        }
    };
    panic!("AAAA ->{:#?}<-", quote!(#loop_body).to_token_stream());

    quote!(
       #loop_body
    )
    .into()
}

fn first_group(token_stream: proc_macro2::TokenStream) -> Option<proc_macro2::Group> {
    if let Some(tt) = token_stream.into_iter().next() {
        return if let proc_macro2::TokenTree::Group(xx) = tt {
            Some(xx)
        } else {
            None
        };
    }
    None
}
