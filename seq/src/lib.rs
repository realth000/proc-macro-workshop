use proc_macro::{TokenStream, TokenTree};

use proc_macro2::{Ident, Literal, Span};
use quote::{quote, ToTokens, TokenStreamExt};
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

    let variable = Ident::new(&token_vec[0].1, Span::from(token_vec[0].0.span()));
    let start = match token_vec[2].1.parse::<i32>() {
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
    let end = match token_vec[5].1.parse::<i32>() {
        Ok(v) => v,
        Err(e) => {
            return compile_error!(
                proc_macro2::Span::from(token_vec[5].0.span()),
                "failed to parse as i32 type: {}",
                e
            );
        }
    };

    // Parse and expand loop body.
    let loop_body = match parse_str::<ExprBlock>(&token_vec[6].1) {
        Ok(ExprBlock { block, .. }) => match first_group(block.to_token_stream()) {
            Some(group) => match apply_loop(start, end, &variable, &group.stream()) {
                Some(v) => v,
                None => {
                    return compile_error!(
                        proc_macro2::Span::from(token_vec[6].0.span()),
                        "failed to apply loop"
                    );
                }
            },
            None => {
                return compile_error!(
                    proc_macro2::Span::from(token_vec[6].0.span()),
                    "failed to parse code block group"
                );
            }
        },
        Err(e) => {
            return compile_error!(
                proc_macro2::Span::from(token_vec[6].0.span()),
                "failed to parse code as block: {}",
                e
            );
        }
    };
    // panic!(
    //     "[test] {:#?}<-",
    //     proc_macro2::TokenStream::from(loop_body.to_token_stream())
    // );

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

fn apply_loop(
    start: i32,
    end: i32,
    variable: &Ident,
    token_stream: &proc_macro2::TokenStream,
) -> Option<proc_macro2::TokenStream> {
    let mut ret = proc_macro2::TokenStream::new();
    // Repeat expanding code (end-start) times, apply variable to i in every expand.
    for i in start..end {
        // For every `TokenTree` in token_stream, check, expand and append it to the tail of output.
        // Seem clone() is required: https://stackoverflow.com/questions/73994927/
        for tt in token_stream.clone() {
            ret.extend(replace_ident(variable, i, &tt.to_token_stream()));
        }
    }
    Some(ret)
}

// Check token_stream, apply every variable to real value.
fn replace_ident(
    variable: &Ident,
    value: i32,
    token_stream: &proc_macro2::TokenStream,
) -> proc_macro2::TokenStream {
    let target_literal = Literal::i32_unsuffixed(value);
    let mut ret = proc_macro2::TokenStream::new();
    for tt in token_stream.clone() {
        match &tt {
            proc_macro2::TokenTree::Group(group) => {
                let group = proc_macro2::Group::new(
                    group.delimiter(),
                    replace_ident(variable, value, &group.stream()),
                );
                ret.append(group);
            }
            proc_macro2::TokenTree::Ident(ident) => {
                if *ident == *variable {
                    ret.append(target_literal.clone());
                } else {
                    ret.append(ident.clone());
                }
            }
            _ => ret.append(tt),
        }
    }
    ret
}
