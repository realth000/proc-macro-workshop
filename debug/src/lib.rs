use proc_macro::TokenStream;

use quote::quote;
use syn::{
    parse_macro_input, parse_quote, Data, DeriveInput, Expr, ExprLit, Fields, GenericParam,
    Generics, Lit, Meta, MetaNameValue, PredicateType, WhereClause, WherePredicate,
};

macro_rules! compile_error {
    ($span: expr, $($arg: expr)*) => {
        syn::Error::new($span, format!($($arg)*))
            .to_compile_error()
            .into()
    };
}

macro_rules! ident_token_str {
    ($ident: expr) => {
        format!("\"{}\"", $ident.to_string().as_str())
            .parse::<proc_macro2::TokenStream>()
            .unwrap()
    };
}

#[proc_macro_derive(CustomDebug, attributes(debug))]
pub fn derive(input: TokenStream) -> TokenStream {
    let ast = parse_macro_input!(input as DeriveInput);
    let ident = &ast.ident;
    let mut field_vec: Vec<proc_macro2::TokenStream> = vec![];

    let data_struct = if let Data::Struct(data_struct) = &ast.data {
        data_struct
    } else {
        return compile_error!(ident.span(), "invalid struct type");
    };
    let named_fields = if let Fields::Named(named_fields) = &data_struct.fields {
        named_fields
    } else {
        return TokenStream::new();
    };

    // From 05-phantom-data on, add trait bound per field, not per generic type.
    // So _generics is not used.
    let _generics = add_trait_bounds(ast.generics.clone());

    let (impl_generics, ty_generics, where_clause) = &ast.generics.split_for_impl();
    let mut where_clause_ex = match where_clause {
        Some(v) => WhereClause {
            where_token: v.where_token,
            predicates: v.predicates.clone(),
        },
        None => WhereClause {
            where_token: Default::default(),
            predicates: Default::default(),
        },
    };

    for named_field in named_fields.named.iter() {
        let mut debug_attr_format: Option<String> = None;
        for attr in &named_field.attrs {
            let p = attr.meta.path().segments.first();
            if p.is_none() || p.unwrap().ident != "debug" {
                continue;
            }

            // The following code may be a prettier version of nested `if let` expressions like this:
            //
            // ```
            // if let syn::Meta::NameValue(meta) = &attr.meta {
            //     if let syn::Expr::Lit(lit) = &meta.value {
            //         if let syn::Lit::Str(s) = &lit.lit {
            //             debug_attr_format =
            //                 Some(s.token().to_string().trim_matches('"').to_string());
            //             // eprintln!("lit: {}", s.token());
            //             // eprintln!("type:{:?}", debug_attr_format);
            //         }
            //     }
            // }
            // ```
            if let Meta::NameValue(MetaNameValue {
                value:
                    Expr::Lit(ExprLit {
                        lit: Lit::Str(ref s),
                        ..
                    }),
                ..
            }) = &attr.meta
            {
                debug_attr_format = Some(s.token().to_string().trim_matches('"').to_string());
                // eprintln!("lit: {}", s.token());
                // eprintln!("type:{:?}", debug_attr_format);
            }
        }

        let i = &named_field.ident.as_ref().unwrap();
        let s = ident_token_str!(i);

        where_clause_ex
            .predicates
            .push(WherePredicate::Type(PredicateType {
                lifetimes: None,
                bounded_ty: named_field.ty.clone(),
                colon_token: Default::default(),
                bounds: parse_quote!(std::fmt::Debug),
            }));

        let field_print = match debug_attr_format {
            Some(v) => {
                // Here use ident_token_str! macro rules to make an literal ident.
                let format_token = ident_token_str!(v);
                quote!(
                    .field(#s, &format_args!(#format_token, &self.#i))
                )
            }
            None => {
                quote!(
                    .field(#s, &self.#i)
                )
            }
        };

        field_vec.push(field_print);
    }

    let s = ident_token_str!(ident);

    quote!(
        impl #impl_generics std::fmt::Debug for #ident #ty_generics #where_clause_ex {
            fn fmt(&self, f: &mut std::fmt::Formatter<'_>)  -> std::fmt::Result {
                f.debug_struct(#s)
                #(#field_vec)*
                .finish()
            }
        }
    )
    .into()
}

// Comments in 04-type-parameter said that this macro should add trait bound.
// Implement like:
// https://github.com/dtolnay/syn/blob/master/examples/heapsize/heapsize_derive/src/lib.rs
fn add_trait_bounds(mut generics: Generics) -> Generics {
    for param in &mut generics.params {
        if let GenericParam::Type(ref mut type_param) = *param {
            type_param.bounds.push(parse_quote!(std::fmt::Debug));
        }
    }
    generics
}
