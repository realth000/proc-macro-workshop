use proc_macro::TokenStream;

use syn::{parse_macro_input, Block, Expr, Ident, Item, ItemFn, Stmt};

macro_rules! compile_error {
    ($span: expr, $($arg: tt)*) => {
        syn::Error::new($span, format!($($arg)*))
            .to_compile_error()
            .into()
    };
}

#[proc_macro_attribute]
pub fn sorted(args: TokenStream, input: TokenStream) -> TokenStream {
    let xx = input.clone();
    let _ = args;
    let input2 = input.clone();
    let item = parse_macro_input!(input2 as Item);
    let item_enum = if let Item::Enum(item_enum) = item {
        item_enum
    } else {
        return compile_error!(
            proc_macro2::Span::call_site(),
            "expected enum or match expression"
        );
    };

    // Should return the original stream though it is a derive macro and maybe cause duplicate
    // definition, otherwise 04-variants-with-data can not pass because of unused import warnings.
    let mut x: proc_macro2::TokenStream = input.into();

    let mut all_ident: Vec<Ident> = item_enum.variants.iter().map(|e| e.ident.clone()).collect();
    let all_ident_orig = all_ident.clone();
    all_ident.sort();
    if let Some((orig, sorted)) = all_ident_orig
        .iter()
        .zip(all_ident.iter())
        .find(|(orig, sorted)| orig != sorted)
    {
        x.extend(
            syn::Error::new(
                sorted.span(),
                format!("{} should sort before {}", sorted, orig),
            )
            .to_compile_error(),
        );
    }
    x.into()
}

#[proc_macro_attribute]
pub fn check(args: TokenStream, input: TokenStream) -> TokenStream {
    let _ = args;
    let match_stmt = if let (ItemFn {
        block: block_box, ..
    }) = parse_macro_input!(input as ItemFn)
    {
        for stmt in (*block_box).stmts {
            match stmt {
                Stmt::Expr(Expr::Match(m, ..), _) => m,
                _ => continue,
            }
        }
    };
    panic!("check: {:#?}",);
}
