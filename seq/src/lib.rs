use proc_macro::TokenStream;
use std::error::Error;

use proc_macro2::{Ident, Literal, Span};
use quote::__private::ext::RepToTokensExt;
use quote::{quote, ToTokens, TokenStreamExt};
use syn::ext::IdentExt;
use syn::parse::{Parse, ParseStream};
use syn::{braced, parse_macro_input, Block, ExprBlock, LitInt, Stmt, Token};

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
        // Ok(SeqContent {
        //     variable: input.parse()?,
        //     in_mark: input.parse()?,
        //     start: input.parse()?,
        //     range_split_mark: input.parse()?,
        //     end: input.parse()?,
        //     content: input.parse()?,
        // })
        let variable: Ident = input.parse()?;
        let in_mark: Token![in] = input.parse()?;
        let start: LitInt = input.parse()?;
        let range_split_mark: Token![..] = input.parse()?;
        let end: LitInt = input.parse()?;

        let mut content: ExprBlock = ExprBlock {
            attrs: vec![],
            label: None,
            block: Block {
                brace_token: Default::default(),
                stmts: vec![],
            },
        };

        // Read braced
        let ahead;
        let _ = braced!(ahead in input);

        let mut ret = proc_macro2::TokenStream::new();

        let mut expect_prefix = false;
        let mut seq_punct: Option<proc_macro2::TokenTree> = None;

        for t in (ahead.parse::<proc_macro2::TokenStream>()?)
            .clone()
            .into_iter()
        {
            match &t {
                proc_macro2::TokenTree::Ident(ident) => {
                    let t_next = t.next().unwrap();
                    match t_next {
                        proc_macro2::TokenTree::Punct(punct) => {
                            if punct.to_string() == "~" {
                                let t_next_next = t_next.next();
                                if t_next_next.is_some() {
                                    match t_next_next.unwrap() {
                                        proc_macro2::TokenTree::Ident(ident) => {
                                            if ident == &variable {
                                                // Parsed f~N
                                            }
                                        }
                                        _ => continue,
                                    }
                                } else {
                                }
                            } else {
                                continue;
                            }
                        }
                        _ => panic!("not valid place"),
                    }
                    if expect_prefix {
                        if ident != &variable {
                            return Err(syn::Error::new(
                                ident.span(),
                                format!("expected loop variable `{}` here", variable),
                            ));
                        } else {
                        }
                    }
                    ret.append(ident.clone());
                }
                proc_macro2::TokenTree::Group(group) => {
                    ret.append(group.clone());
                }
                proc_macro2::TokenTree::Literal(literal) => {
                    ret.append(literal.clone());
                }
                proc_macro2::TokenTree::Punct(punct) => {
                    if punct.to_string() == "~" {
                        seq_punct = Some(proc_macro2::TokenTree::Punct(punct.clone()));
                        expect_prefix = true;
                    } else {
                        ret.append(punct.clone());
                    }
                }
            }
        }

        if ahead.peek(Ident::peek_any) {
            let a = Ident::parse_any(&ahead)?;
            // let a = ahead.parse::<Ident::parse_any()>()?;
            if a == "fn" {
                panic!("AAAAAAAAAAAAAAAAAAAAAAAAAAAAA");
            } else {
                panic!("1111111 {:#?}", ahead);
            }
        }
        panic!("{:#?}", ahead);

        loop {
            // if input.peek(syn::Ident) && input.peek2(syn::Ident) {
            if ahead.peek(syn::Ident::peek_any) && ahead.peek2(Token![~]) {
                let variable_prefix = ahead.parse::<Ident>()?;
                let _ = ahead.parse::<Token![~]>()?;
                let got_variable = ahead.parse::<Ident>()?;
                if got_variable != variable {
                    return Err(syn::Error::new(
                        got_variable.span(),
                        format!("expected loop variable `{}` here", variable),
                    ));
                }
                // TODO: Extension feature for flexible suffix `foo~N~suffix`

                // loop {
                //     if input.peek(Token![;]) {
                //         break;
                //     } else if input.peek(syn::Ident) {
                //         let _ = input.parse::<Ident>()?;
                //     } else if input.peek(syn::Lit) {
                //         let _ = input.parse::<syn::Lit>()?;
                //     } else if input.peek(syn::Expr) {
                //         let _ = input.parse::<syn::Expr>()?;
                //     }
                // }
                let s = ahead.parse::<Stmt>()?;
            } else if ahead.is_empty() {
                break;
            } else {
                content.block.stmts.push(ahead.parse::<Stmt>()?);
            }
        }
        panic!("finish! {:#?}", content);

        Ok(SeqContent {
            variable,
            in_mark,
            start,
            range_split_mark,
            end,
            content,
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
    // panic!("{:#?}", input);
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
