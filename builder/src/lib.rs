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
            pub fn build(&mut self) -> Result<#ident, std::boxed::Box<dyn std::error::Error>> {
                if self.executable.is_none() {
                    return Err(std::boxed::Box::<dyn std::error::Error>::from(format!("{} not set", "executable")));
                }
                if self.args.is_none() {
                    return Err(std::boxed::Box::<dyn std::error::Error>::from(format!("{} not set", "args")));
                }
                if self.env.is_none() {
                    return Err(std::boxed::Box::<dyn std::error::Error>::from(format!("{} not set", "env")));
                }
                if self.current_dir.is_none() {
                    return Err(std::boxed::Box::<dyn std::error::Error>::from(format!("{} not set", "current_dir")));
                }
                Ok(#ident{
                    executable: self.executable.clone().unwrap(),
                    args: self.args.clone().unwrap(),
                    env: self.env.clone().unwrap(),
                    current_dir: self.current_dir.clone().unwrap(),
                })

            }

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
    ).into()
}
