use proc_macro::TokenStream;
use std::error::Error;

use proc_macro2::{Ident, Literal, Span};
use quote::{quote, ToTokens, TokenStreamExt};
use syn::parse::{Parse, ParseStream};
use syn::{parse_macro_input, ExprBlock, LitInt, Token};

use derive_debug::CustomDebug;

macro_rules! compile_error {
    ($span: expr, $($arg: tt)*) => {
        syn::Error::new($span, format!($($arg)*))
            .to_compile_error()
            .into()
    };
}

#[derive(CustomDebug)]
struct SeqContent {
    variable: Ident,
    in_mark: Token![in],
    start: LitInt,
    range_split_mark: Token![..],
    end: LitInt,
    content: ExprBlock,
}

impl Parse for SeqContent {
    fn parse(input: ParseStream) -> syn::Result<Self> {
        Ok(SeqContent {
            variable: input.parse()?,
            in_mark: input.parse()?,
            start: input.parse()?,
            range_split_mark: input.parse()?,
            end: input.parse()?,
            content: input.parse()?,
        })
    }
}

impl SeqContent {
    pub fn apply_seq(&self) -> Result<proc_macro2::TokenStream, Box<dyn Error>> {
        let a = self.apply_loop()?;
        Ok(a)
    }

    fn apply_loop(&self) -> Result<proc_macro2::TokenStream, Box<dyn Error>> {
        let start = self.start.token().to_string().parse::<i32>()?;
        let end = self.end.token().to_string().parse::<i32>()?;

        let mut ret = proc_macro2::TokenStream::new();
        // Repeat expanding code (end-start) times, apply variable to i in every expand.
        for i in start..end {
            // For every `TokenTree` in token_stream, check, expand and append it to the tail of output.
            // Seem clone() is required: https://stackoverflow.com/questions/73994927/
            for stmt in self.content.clone().block.stmts {
                ret.extend(replace_ident(&self.variable, i, &stmt.to_token_stream()));
            }
        }
        Ok(ret)
    }

    pub fn span(&self) -> Span {
        self.variable.span()
    }
}

#[proc_macro]
pub fn seq(input: TokenStream) -> TokenStream {
    let seq_content = parse_macro_input!(input as SeqContent);
    // panic!("{:#?}", seq_content.apply_seq());

    let d = match seq_content.apply_seq() {
        Ok(v) => v,
        Err(e) => return compile_error!(seq_content.span(), "{}", e),
    };
    quote!(#d).into()
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
