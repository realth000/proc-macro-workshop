use proc_macro::TokenStream;
use std::error::Error;

use proc_macro2::{Delimiter, Group, Ident, Literal, Punct, Span};
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
    upper_equal: Option<Punct>,
    content: Group,
}

impl Parse for SeqContent {
    fn parse(input: ParseStream) -> syn::Result<Self> {
        Ok(Self {
            variable: input.parse()?,
            in_mark: input.parse()?,
            start: input.parse()?,
            range_split_mark: input.parse()?,
            upper_equal: input.parse()?,
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
        // upper_equal is the upper equal bound in "1..=20";
        let end = if self.upper_equal.is_some() {
            // "1..=20"
            self.end.token().to_string().parse::<i32>()? + 1
        } else {
            // "1..20"
            self.end.token().to_string().parse::<i32>()?
        };

        // First try to match partial repeat `#()*` in test 05.
        // If found, use that result.
        // If not found, treat the entire code as "need to repeat" as in test 01-04.
        //
        // Note that here we do not allow nested `#()*` or both repeating whole code and partial code.
        let ret = partial_match(&self.variable, start, end, self.content.stream()).map_or_else(
            || {
                // Here is just what we did in test 01-04, whole code need to repeat.
                let mut r = proc_macro2::TokenStream::new();
                for i in start..end {
                    // For every `TokenTree` in token_stream, check, expand and append it to the tail of output.
                    // Seem clone() is required: https://stackoverflow.com/questions/73994927/
                    r.extend(replace_ident(&self.variable, i, &self.content.stream()));
                }
                r
            },
            |partial_matched| partial_matched,
        );

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
    let target_literal = Literal::i32_unsuffixed(value);
    let mut ret = proc_macro2::TokenStream::new();
    let full_box: Vec<_> = token_stream.clone().into_iter().collect();
    let mut it = token_stream.clone().into_iter().enumerate();

    while let Some((index, tt)) = it.next() {
        match &tt {
            // When meeting a group, need to analyze the code inside it.
            // e.g.
            // In `fn foo() -> u64 { 0 }`, the `{ 0 }` is a block token tree.
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
                // Check if we have the "F" "~" "N" structure.
                // If we have, convert "F" "~" "N" to "F1", "F2"... and continue.
                // If not, just copy the token into `ret` stream.
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
                        // It seems both `ident.span()` and `next_ident.span()` here work, do not matter.
                        ret.append(Ident::new(format!("{ident}{value}").as_str(), ident.span()));
                        // Skip the iterator to the correct position.
                        // Here we have a "F" "~" "N", and `it` is on "F", call `it.nth(1)` will
                        // move `it` to `N`, and in next loop round, `it` will be in the correct
                        // position where is the next token after "N".
                        it.nth(1);
                        continue;
                    }
                }
                // Fallback, if the ident here not followed by "~" and "N", just append it to the stream.
                ret.append(ident.clone());
                continue;
            }
            proc_macro2::TokenTree::Punct(punct) if punct.to_string() == "~" => {}
            _ => ret.append(tt),
        }
    }
    ret
}

/// Check for situations in test 05, try to match `#()*` in `token_stream`, the code inside it need repeat.
///
/// If found, return `Some` contains the handled token stream.
/// If not found, return None.
fn partial_match(
    variable: &Ident,
    start: i32,
    end: i32,
    token_stream: proc_macro2::TokenStream,
) -> Option<proc_macro2::TokenStream> {
    // Flag indicating whether `#()*` found in token stream.
    let mut matched = false;

    let mut ret = proc_macro2::TokenStream::new();
    let full_box: Vec<_> = token_stream.clone().into_iter().collect();

    // Iterator used for checking `#()*` block, only repeat code inside this block.
    let mut it = token_stream.into_iter().enumerate();

    while let Some((index, tt)) = it.next() {
        match &tt {
            // If found a group, recursively check it.
            proc_macro2::TokenTree::Group(group) => {
                let sub_matched = partial_match(variable, start, end, group.stream());
                let mut g = sub_matched.map_or_else(
                    || Group::new(group.delimiter(), group.stream()),
                    |matched_group| {
                        // `#()*` found in sub group, set matched to true.
                        matched = true;
                        Group::new(group.delimiter(), matched_group)
                    },
                );
                g.set_span(group.span());
                ret.append(g);
            }
            proc_macro2::TokenTree::Punct(head_punct) if head_punct.as_char() == '#' => {
                if let Some(proc_macro2::TokenTree::Group(group)) = full_box.get(index + 1) {
                    if group.delimiter() != Delimiter::Parenthesis {
                        ret.append(tt);
                        continue;
                    }
                    if let Some(proc_macro2::TokenTree::Punct(tail_punct)) = full_box.get(index + 2)
                    {
                        if tail_punct.to_string() != "*" {
                            ret.append(tt);
                            continue;
                        }
                        // Here we matched `#()*`.
                        matched = true;
                        // Matched "#( {code} )*"
                        // Repeat expanding code (end-start) times, apply variable to i in every expand.
                        for i in start..end {
                            // For every `TokenTree` in token_stream, check, expand and append it to the tail of output.
                            // Seem clone() is required: https://stackoverflow.com/questions/73994927/
                            ret.extend(replace_ident(
                                variable,
                                i,
                                // &self.content.stream(),
                                &group.stream(),
                            ));
                        }

                        it.nth(1);
                        continue;
                    }
                }

                // Fallback.
                ret.append(tt);
            }
            _ => ret.append(tt),
        }
    }

    if matched {
        Some(ret)
    } else {
        None
    }
}
