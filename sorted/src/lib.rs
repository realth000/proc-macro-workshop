use proc_macro::TokenStream;

use syn::{parse_macro_input, Item};

#[proc_macro_attribute]
pub fn sorted(args: TokenStream, input: TokenStream) -> TokenStream {
    let _ = args;
    let _ = input;

    eprintln!("args: {:#?}", args);
    eprintln!("input: {:#?}", input);
    let item = parse_macro_input!(input as Item);
    eprintln!("item: {:#?}", item);

    TokenStream::new()
}
