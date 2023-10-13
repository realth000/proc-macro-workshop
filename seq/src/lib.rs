use proc_macro::TokenStream;
use std::error::Error;

use proc_macro2::{Group, Ident, Literal, Punct, Span};
use quote::__private::ext::RepToTokensExt;
use quote::{quote, TokenStreamExt};
use syn::parse::{Parse, ParseStream};
use syn::{parse_macro_input, LitInt, Token};

use derive_debug::CustomDebug;

macro_rules! compile_error {
    ($span: expr, $($arg: tt)*) => {
        syn::Error::new($span, format!($($arg)*))
            .to_compile_error()
            .into()
    };
}

// Use proc_macro2::Group for parsing
// https://docs.rs/syn/latest/syn/parse/trait.Parse.html
#[derive(CustomDebug)]
struct SeqContent {
    variable: Ident,
    in_mark: Token![in],
    start: LitInt,
    range_split_mark: Token![..],
    end: LitInt,
    content: Group,
}

impl Parse for SeqContent {
    fn parse(input: ParseStream) -> syn::Result<Self> {
        Ok(Self {
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
            ret.extend(replace_ident(&self.variable, i, &self.content.stream()));
        }
        Ok(ret)
    }

    pub fn span(&self) -> Span {
        self.variable.span()
    }
}

#[allow(
    clippy::too_many_lines,
    clippy::missing_errors_doc,
    clippy::missing_panics_doc
)]
#[proc_macro]
pub fn seq(input: TokenStream) -> TokenStream {
    // When parsing ParseStream, compiler expects no syntax error because otherwise it can not
    // understand what kinds of tokens are in the ParseStream .
    //
    // Therefore convert invalid syntax to valid ones to pass parse.
    // In this conversion, make that invalid syntax "special" so that when expanding code later,
    // we can still find out where those "~N" are.
    // One method is act like a C++ compiler (maybe linker?): add specific prefix or
    // suffix as mark to help recognizing them.
    //
    // Here should use ${prefix} to represent captured text called "prefix".
    // And in format! macro, should use "{{" to represent "{".
    // TODO: Handle general variable, not specified `N`.
    let seq_content = parse_macro_input!(input as SeqContent);
    // panic!("{seq_content:#?}");

    let d = match seq_content.apply_seq() {
        Ok(v) => v,
        Err(e) => return compile_error!(seq_content.span(), "{}", e),
    };
    let expand = quote!(#d);
    expand.into()
}

// Check token_stream, apply every variable to real value.
fn replace_ident(
    variable: &Ident,
    value: i32,
    token_stream: &proc_macro2::TokenStream,
) -> proc_macro2::TokenStream {
    // Flag marks whether current is waiting for the target variable.
    // e.g. "F ~ N": when parsing punct '~', set flag to true and expecting for the corresponding
    // target variable N.
    // * If the next token is ident 'N', replace the full name and save in token stream.
    // * Otherwise push the original tokens in token stream.
    let mut waiting_list: Vec<proc_macro2::TokenStream> = vec![];
    let mut waiting_target = false;
    let target_literal = Literal::i32_unsuffixed(value);
    let mut ret = proc_macro2::TokenStream::new();
    let full_box: Vec<_> = token_stream.clone().into_iter().collect();
    for (index, tt) in token_stream.clone().into_iter().enumerate() {
        match &tt {
            proc_macro2::TokenTree::Group(group) => {
                let mut g = Group::new(
                    group.delimiter(),
                    replace_ident(variable, value, &group.stream()),
                );
                // Set span here, or will get wrong error message mark position.
                g.set_span(group.span());
                ret.append(g);
            }
            // Optimize, using match guard.
            proc_macro2::TokenTree::Ident(ident) if *ident == *variable => {
                let mut i = target_literal.clone();
                i.set_span(ident.span());
                ret.append(i);
            }
            proc_macro2::TokenTree::Ident(ident) => {
                if let Some(proc_macro2::TokenTree::Punct(next_punct)) = full_box.get(index + 1) {
                    if next_punct.to_string() != "~" {
                        ret.append(ident.clone());
                        continue;
                    }
                    if let Some(proc_macro2::TokenTree::Ident(next_ident)) = full_box.get(index + 2)
                    {
                        if *next_ident != *variable {
                            ret.append(ident.clone());
                            continue;
                        }
                        // Catch "F ~ N"
                        panic!(">>> {ident:#?}{value:#?}");
                        ret.append(Ident::new(
                            format!("{ident}{value}").as_str(),
                            next_ident.span(),
                        ));
                    }
                }
            }
            proc_macro2::TokenTree::Punct(punct) if punct.to_string() == "~" => {}
            // // Handle f~N
            // proc_macro2::TokenTree::Ident(ident)
            //     if ident
            //         .to_string()
            //         .ends_with(format!("{SEQ_MARK_TEXT}{variable}").as_str()) =>
            // {
            //     let old_ident = ident.to_string();
            //     let new_ident = old_ident.replace(
            //         format!("{SEQ_MARK_TEXT}{variable}").as_str(),
            //         format!("{value}").as_str(),
            //     );
            //     let target_literal = Ident::new(new_ident.as_str(), ident.span());
            //     ret.append(target_literal);
            // }
            _ => ret.append(tt),
        }
    }
    ret
}
