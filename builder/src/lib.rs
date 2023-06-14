use proc_macro::TokenStream;

use proc_macro2::Ident;
use quote::{quote, ToTokens};
use syn::{parse_macro_input, Data, DeriveInput, Fields, Meta, Type};

static OPTION_MODIFIER: [&str; 4] = ["<", ">", "(", ")"];

#[proc_macro_derive(Builder, attributes(builder))]
pub fn derive(input: TokenStream) -> TokenStream {
    let ast = parse_macro_input!(input as DeriveInput);
    let ident = &ast.ident;
    let builder_ident = Ident::new(
        &format!("{}Builder", ident.to_string().as_str()),
        ident.span(),
    );

    let mut field_vec: Vec<proc_macro2::TokenStream> = vec![];
    let mut empty_field_vec: Vec<proc_macro2::TokenStream> = vec![];
    let mut field_method_vec: Vec<proc_macro2::TokenStream> = vec![];
    let mut field_build_vec: Vec<proc_macro2::TokenStream> = vec![];

    if let Data::Struct(s) = &ast.data {
        if let Fields::Named(named_fields) = &s.fields {
            for named_field in named_fields.named.iter() {
                let mut each: Option<String> = None;
                for attr in &named_field.attrs {
                    let p = attr.meta.path().segments.first();
                    if p.is_none() || p.unwrap().ident != "builder" {
                        continue;
                    }
                    if let Meta::List(meta) = &attr.meta {
                        for x in meta.tokens.clone().into_iter() {
                            match x {
                                proc_macro2::TokenTree::Ident(i) => {
                                    if i != "each" {
                                        // Here we can use format! to show custom message but in
                                        // test 08 it requires a const one.
                                        // format!("unknown attribute {}", i)
                                        //
                                        // Test 08 requires the error message to mark all tokens
                                        // inside the unrecognized attribute, thus use new_spanned()
                                        // instead of new(), the latter one only mark the current
                                        // span.
                                        //
                                        return syn::Error::new_spanned(
                                            &attr.meta,
                                            "expected `builder(each = \"...\")`".to_string(),
                                        )
                                        .to_compile_error()
                                        .into();
                                    }
                                }
                                proc_macro2::TokenTree::Literal(l) => {
                                    each = Some(String::from(l.to_string().trim_matches('"')));
                                    break;
                                }
                                _ => continue,
                            }
                        }
                    }
                    break;
                }
                let i = &named_field.ident;
                let t = &named_field.ty;
                if let Type::Path(path) = t {
                    let segments = &path.path.segments;
                    if !segments.is_empty() && segments[0].ident.to_string().as_str() == "Option" {
                        field_vec.push(quote!(
                            #i: #t
                        ));
                        field_build_vec.push(quote!(
                            #i: self.#i.clone()
                        ));
                        if let Some(type_name) = segments[0]
                            .arguments
                            .to_token_stream()
                            .to_string()
                            .split(' ')
                            .find(|e| !OPTION_MODIFIER.contains(e))
                        {
                            let real_type = Ident::new(type_name, ident.span());
                            field_method_vec.push(quote!(
                                pub fn #i(&mut self, #i: #real_type) -> &mut Self {
                                    self.#i = Some(#i);
                                    self
                                }
                            ));
                        } else {
                            panic!("can not find type in Option")
                        }
                    } else {
                        field_vec.push(quote!(
                            #i: std::option::Option<#t>
                        ));
                        field_build_vec.push(quote!(
                            #i: self.#i.clone().unwrap_or_default()
                        ));
                        if let Some(..) = each {
                            if segments.is_empty() || segments[0].ident != "Vec" {
                                panic!("builder(each={}) used on a not Vec<> field", each.unwrap())
                            }
                            if let Some(vec_type) = segments[0]
                                .arguments
                                .to_token_stream()
                                .to_string()
                                .split(' ')
                                .find(|e| !OPTION_MODIFIER.contains(e))
                            {
                                let each_ident = Ident::new(each.unwrap().as_str(), ident.span());
                                let vec_type_ident = Ident::new(vec_type, ident.span());
                                field_method_vec.push(quote!(
                                    pub fn #each_ident(&mut self, #each_ident: #vec_type_ident) -> &mut Self {
                                        if self.#i.is_none() {
                                            self.#i = Some(Vec::new())
                                        }
                                        if let Some(ref mut v) =  self.#i {
                                            v.push(#each_ident);
                                        }
                                        self
                                    }
                                ));
                            } else {
                                panic!(
                                    "can not find what T in {} when parsing as Vec<T>",
                                    segments[0].to_token_stream()
                                )
                            }
                        } else {
                            field_method_vec.push(quote!(
                                pub fn #i(&mut self, #i: #t) -> &mut Self {
                                    self.#i = Some(#i);
                                    self
                                }
                            ));
                        }
                    }
                    empty_field_vec.push(quote!(
                        #i: None
                    ));
                }
                // eprintln!("named_field: {:#?}", named_field);
            }
        }
    } else {
        panic!("invalid derive type: not a struct")
    }

    // eprintln!("field_vec: {:#?}", field_vec);

    quote!(
        impl #ident {
            pub fn builder() -> #builder_ident {
                #builder_ident {
                    #(#empty_field_vec,)*
                }
            }
        }

        pub struct #builder_ident {
            #(#field_vec,)*
        }

        impl #builder_ident {
            pub fn build(&mut self) -> Result<#ident, std::boxed::Box<dyn std::error::Error>> {
                Ok(#ident{
                    #(#field_build_vec,)*
                })
            }
            #(#field_method_vec)*
        }
    )
    .into()
}
