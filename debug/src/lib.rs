use proc_macro::TokenStream;
use proc_macro2::{Ident, Span};
use quote::quote;
use std::clone::Clone;
use std::collections::HashMap;
use syn::punctuated::Punctuated;
use syn::token::{Colon, Where};
use syn::{
    parse_macro_input, parse_quote, Data, DeriveInput, Expr, ExprLit, Fields, GenericArgument,
    GenericParam, Generics, Lit, Meta, MetaNameValue, Path, PathArguments, PathSegment,
    PredicateType, Token, TraitBound, TraitBoundModifier, Type, TypeParamBound, TypePath,
    WhereClause, WherePredicate,
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

#[allow(
    clippy::too_many_lines,
    clippy::missing_errors_doc,
    clippy::missing_panics_doc,
    clippy::cognitive_complexity
)]
#[proc_macro_derive(CustomDebug, attributes(debug))]
pub fn derive(input: TokenStream) -> TokenStream {
    let ast = parse_macro_input!(input as DeriveInput);
    let ident = &ast.ident;
    // if ident == "Field" {
    //     return TokenStream::new();
    // } else {
    //     panic!("{:#?}", ast);
    // }
    let mut field_vec: Vec<proc_macro2::TokenStream> = vec![];
    // For `PhantomData` type, if the generic type (such as `T`) only exists in `PhantomData`, do
    // not generate trait bound for `T`.
    // Here uses a `HashMap` to record whether a generic type only exists in `PhantomData`.
    // If true, Ident only exists in `PhantomData`.
    // If false, Ident not exists in `PhantomData` or both in/out `PhantomData`.
    let mut phantom_generic_map: HashMap<Ident, (bool, Option<Type>)> = HashMap::new();

    let Data::Struct(data_struct) = &ast.data else {
        return compile_error!(ident.span(), "invalid struct type");
    };
    let Fields::Named(named_fields) = &data_struct.fields else {
        return TokenStream::new();
    };

    // 08-escape-hatch
    // Find and record `#[debug(bound = "T::Value: Debug")]`
    let mut outer_bound: Option<String> = None;
    for attr in &ast.attrs {
        if let Meta::List(meta_list) = &attr.meta {
            if !meta_list.path.segments.is_empty()
                && meta_list.path.segments.first().unwrap().ident == "debug"
            {
                match trait_bound_token(&meta_list.tokens) {
                    Some(bound) => {
                        outer_bound = Some(bound);
                    }
                    None => continue,
                }
            }
        }
    }

    // This is a bad solution:
    // From 05-phantom-data on, add trait bound per field, not per generic type.
    // So _generics is not used.
    //
    // Why? as 06-bound-trouble says, rust compile may refuse to compile some too deep nested generic
    // types.
    //
    // What's the best practise?
    // Follow the guide in 05-phantom-data, rules are:
    // 1. Generate trait bound for every generic type, not field:
    //   BAD:  Foo<T> : Debug
    //   GOOD: T : Debug
    // 2. When a generic type ident ONLY exists in `PhantomData`, do not add trait bound for it.
    //
    // So the solution is, lookup every field and find generics only exist in `PhantomData`,
    // finally generate trait bound for all other generic types.
    let _generics = add_trait_bounds(ast.generics.clone());

    // Record all generic type names.
    for param in &ast.generics.params {
        if let GenericParam::Type(gt) = param {
            phantom_generic_map.insert(gt.ident.clone(), (false, None));
        }
    }

    let (impl_generics, ty_generics, where_clause) = &ast.generics.split_for_impl();
    // panic!("impl_generics: {:#?}", impl_generics);
    let mut where_clause_ex = where_clause.as_ref().map_or_else(
        || WhereClause {
            where_token: Where::default(),
            predicates: Punctuated::default(),
        },
        |v| WhereClause {
            where_token: v.where_token,
            predicates: v.predicates.clone(),
        },
    );

    for named_field in &named_fields.named {
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

        if let Type::Path(TypePath {
            path: Path { segments, .. },
            ..
        }) = &named_field.ty
        {
            for segment in segments {
                // For 04-type-parameter, T.
                // struct S<T> {
                //   foo: T,
                // }
                if phantom_generic_map.contains_key(&segment.ident) {
                    break;
                }
                // For 05-phantom-data, T.
                // struct S<T> {
                //   foo: Bar<T>,
                // }
                if let PathArguments::AngleBracketed(ab) = &segment.arguments {
                    // For 05-phantom-data, PhantomData<T>.
                    // struct S<T> {
                    //   foo: PhantomData<T>,
                    // }
                    if ab.args.is_empty() {
                        continue;
                    }

                    for arg in &ab.args {
                        if let GenericArgument::Type(Type::Path(TypePath { path, .. })) = arg {
                            let ss = &path.segments;
                            if !phantom_generic_map.contains_key(&ss.first().unwrap().clone().ident)
                            {
                                continue;
                            }
                            if segment.ident == "PhantomData" {
                                phantom_generic_map
                                    .insert(ss.first().unwrap().clone().ident, (true, None));
                            } else {
                                // For 07-associated-type, T::Value, where T is a trait.
                                // struct Foo<T: Trait> {
                                //   values: Vec<T::Value>,
                                // }
                                // Here store full path to generate trait bound looks like T::Value : Debug.
                                phantom_generic_map.insert(
                                    ss.first().unwrap().clone().ident,
                                    (
                                        false,
                                        Some(Type::Path(TypePath {
                                            qself: None,
                                            path: path.clone(),
                                        })),
                                    ),
                                );
                            }
                        }
                    }
                }
            }
        }

        // panic!("where: {:#?}", where_clause_ex.predicates);

        let field_print = debug_attr_format.map_or_else(
            || {
                quote!(
                    .field(#s, &self.#i)
                )
            },
            |v| {
                // Here use ident_token_str! macro rules to make an literal ident.
                let format_token = ident_token_str!(v);
                quote!(
                    .field(#s, &format_args!(#format_token, &self.#i))
                )
            },
        );

        field_vec.push(field_print);
    }

    // Avoid add trait bound to generics only used in `PhantomData`.
    for (generic_ident, (only_in_phantom, ident_vec)) in &phantom_generic_map {
        if *only_in_phantom {
            continue;
        }
        where_clause_ex
            .predicates
            .push(WherePredicate::Type(PredicateType {
                lifetimes: None,
                bounded_ty: ident_vec
                    .as_ref()
                    .map_or_else(|| parse_quote!(#generic_ident), Clone::clone),
                colon_token: Colon::default(),
                bounds: parse_quote!(std::fmt::Debug),
            }));
    }

    let s = ident_token_str!(ident);

    let body = quote!(
            fn fmt(&self, f: &mut std::fmt::Formatter<'_>)  -> std::fmt::Result {
                f.debug_struct(#s)
                #(#field_vec)*
                .finish()
            }
    );

    let expanded: TokenStream = outer_bound.map_or_else(|| quote!(
            impl #impl_generics std::fmt::Debug for #ident #ty_generics #where_clause_ex {
                #body
            }
        )
        .into(), |bound| {
            let outer_bound_where_clause = str_to_where_clause(bound.as_str(), ident.span());
            quote!(
                impl #impl_generics std::fmt::Debug for #ident #ty_generics #outer_bound_where_clause {
                    #body
                }
            )
                .into()
        });

    expanded
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

// FIXME: Now only check type, sort not checked.
fn trait_bound_token(token: &proc_macro2::TokenStream) -> Option<String> {
    if token.is_empty() {
        return None;
    }
    for tt in token.clone() {
        match tt {
            proc_macro2::TokenTree::Ident(i) => {
                if i != "bound" {
                    return None;
                }
            }
            proc_macro2::TokenTree::Punct(p) => {
                if p.as_char() != '=' {
                    return None;
                }
            }
            proc_macro2::TokenTree::Literal(l) => {
                return Some(String::from(l.to_string().trim_matches('"')));
            }
            proc_macro2::TokenTree::Group(_) => continue,
        }
    }
    None
}

fn str_to_where_clause(str: &str, span: Span) -> Option<WhereClause> {
    let Some(pos) = str.rfind(':') else {
        return None;
    };

    let s1 = str[..pos].to_string();
    let s2 = str[pos + 1..].to_string();

    let ident = s1.trim();
    let bound = s2.trim();

    let mut wc = WhereClause {
        where_token: Where::default(),
        predicates: Punctuated::default(),
    };

    // let mut path_segment_punctuated: Punctuated<PathSegment, Token![::]> = Punctuated::new();
    //
    // for part in ident.split("::") {
    //     path_segment_punctuated.push(PathSegment {
    //         ident: Ident::new(part, span),
    //         arguments: Default::default(),
    //     });
    // }

    let mut bounds_punctuated: Punctuated<TypeParamBound, Token![+]> = Punctuated::new();
    bounds_punctuated.push(TypeParamBound::Trait(TraitBound {
        paren_token: None,
        modifier: TraitBoundModifier::None,
        lifetimes: None,
        path: Path {
            leading_colon: None,
            segments: build_path_segment_punctuated(bound, span),
        },
    }));

    wc.predicates.push(WherePredicate::Type(PredicateType {
        lifetimes: None,
        bounded_ty: Type::Path(TypePath {
            qself: None,
            path: Path {
                leading_colon: None,
                segments: build_path_segment_punctuated(ident, span),
            },
        }),
        colon_token: Colon::default(),
        bounds: bounds_punctuated,
    }));

    Some(wc)
}

fn build_path_segment_punctuated(s: &str, span: Span) -> Punctuated<PathSegment, Token![::]> {
    let mut path_segment_punctuated: Punctuated<PathSegment, Token![::]> = Punctuated::new();
    for part in s.split("::") {
        path_segment_punctuated.push(PathSegment {
            ident: Ident::new(part, span),
            arguments: PathArguments::default(),
        });
    }
    path_segment_punctuated
}
