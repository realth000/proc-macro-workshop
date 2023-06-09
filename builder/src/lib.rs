use proc_macro::TokenStream;

use proc_macro2::Ident;
use quote::quote;
use syn::{parse_macro_input, DeriveInput};

#[proc_macro_derive(Builder)]
pub fn derive(input: TokenStream) -> TokenStream {
    let ast = parse_macro_input!(input as DeriveInput);
    let ident = ast.ident;
    let builder_ident = Ident::new(
        &format!("{}Builder", ident.to_string().as_str()),
        ident.span(),
    );

    quote!(
        impl #ident {
            pub fn builder() -> #builder_ident {
                #builder_ident {
                    executable: None,
                    args: None,
                    env: None,
                    current_dir: None,
                }
            }
        }

        pub struct #builder_ident {
            executable: Option<String>,
            args: Option<Vec<String>>,
            env: Option<Vec<String>>,
            current_dir: Option<String>,
        }

        impl #builder_ident {
            pub fn executable(&mut self, executable: String) {
                self.executable = Some(executable);
            }

            pub fn args(&mut self, args: Vec<String>) {
                self.args = Some(args);
            }

            pub fn env(&mut self, env: Vec<String>) {
                self.env = Some(env);
            }

            pub fn current_dir(&mut self, current_dir: String) {
                self.current_dir = Some(current_dir);
            }
        }
    )
    .into()
}
