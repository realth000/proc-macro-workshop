use proc_macro::TokenStream;

use syn::{parse_macro_input, Item};

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
    let _ast = if let Item::Enum(item_enum) = item {
        item_enum
    } else {
        return compile_error!(
            proc_macro2::Span::call_site(),
            "expected enum or match expression"
        );
    };

    TokenStream::new()
}
