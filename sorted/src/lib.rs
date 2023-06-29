use proc_macro::TokenStream;

use syn::{parse_macro_input, Ident, Item};

macro_rules! compile_error {
    ($span: expr, $($arg: tt)*) => {
        syn::Error::new($span, format!($($arg)*))
            .to_compile_error()
            .into()
    };
}

#[proc_macro_attribute]
pub fn sorted(args: TokenStream, input: TokenStream) -> TokenStream {
    let _ = args;
    let item = parse_macro_input!(input as Item);
    let item_enum = if let Item::Enum(item_enum) = item {
        item_enum
    } else {
        return compile_error!(
            proc_macro2::Span::call_site(),
            "expected enum or match expression"
        );
    };

    // panic!("{:#?}", _ast.variants);

    let mut all_ident: Vec<Ident> = item_enum.variants.iter().map(|e| e.ident.clone()).collect();
    let all_ident_orig = all_ident.clone();
    all_ident.sort();
    if let Some((orig, sorted)) = all_ident_orig
        .iter()
        .zip(all_ident.iter())
        .find(|(orig, sorted)| orig != sorted)
    {
        return compile_error!(sorted.span(), "{} should sort before {}", sorted, orig);
    }

    // panic!("all ident: {:#?}", all_ident);
    TokenStream::new()
}
