use proc_macro::TokenStream;
use std::error::Error;

use proc_macro2::{Ident, Literal, Span};
use quote::{quote, ToTokens, TokenStreamExt};
use regex::Regex;
use syn::parse::{Parse, ParseStream};
use syn::{parse_macro_input, parse_str, ExprBlock, LitInt, Token};

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
        let variable: Ident = input.parse()?;
        let in_mark: Token![in] = input.parse()?;
        let start: LitInt = input.parse()?;
        let range_split_mark: Token![..] = input.parse()?;
        let end: LitInt = input.parse()?;

        // let content: ExprBlock = input.parse()?;
        //
        // let result = SeqContent {
        //     variable,
        //     in_mark,
        //     start,
        //     range_split_mark,
        //     end,
        //     content,
        // };
        // panic!("true content: {:#?}", result);
        // return Ok(result);

        // Handle "~N" which is invalid syntax in compile.
        // When parsing ParseStream, compiler expects no syntax error because otherwise it can not
        // understand what kinds of tokens are in the ParseStream .
        //
        // Therefore convert invalid syntax to valid ones to pass parse.
        // In this conversion, make that invalid syntax "special" so that when expanding code later,
        // we can still find out where those "~N" are.
        // One method is act like a C++ compiler (maybe linker?): add specific prefix or
        // suffix as mark to help recognizing them.
        //
        // TODO: Support flexible suffix.
        let re = Regex::new(format!("(?P<prefix>\\w+) ~ {}", variable).as_str()).unwrap();
        // Here should use ${prefix} to represent captured text called "prefix".
        // And in format! macro, should use "{{" to represent "{".
        let mark_text = format!("${{prefix}}___NEED_EXPAND___{}", variable);
        let tmp_str = input.to_string();
        let result = re.replace_all(tmp_str.as_str(), mark_text);
        // panic!("{}", result);
        // let content = syn::ExprBlock::parse(parse_str(result.into_owned().as_str())?)?;
        let content = if let syn::Expr::Block(block) =
            parse_str::<syn::Expr>(result.into_owned().as_str())?
        {
            block
        } else {
            return Err(syn::Error::new(input.span(), "not a valid ExprBlock"));
        };

        let result = SeqContent {
            variable,
            in_mark,
            start,
            range_split_mark,
            end,
            content,
        };

        panic!("wrong content: {:#?}", result);

        Ok(result)
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
    panic!("SUCCESS!!!!");

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
                let mut g = proc_macro2::Group::new(
                    group.delimiter(),
                    replace_ident(variable, value, &group.stream()),
                );
                // Set span here, or will get wrong error message mark position.
                g.set_span(group.span());
                ret.append(g);
            }
            // Optimize, using match guard.
            proc_macro2::TokenTree::Ident(ident) if *ident == *variable => {
                ret.append(target_literal.clone());
            }
            _ => ret.append(tt),
        }
    }
    ret
}
