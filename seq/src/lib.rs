use proc_macro::TokenStream;

#[proc_macro]
pub fn seq(input: TokenStream) -> TokenStream {
    TokenStream::new()
}
