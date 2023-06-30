use proc_macro::TokenStream;

use quote::{quote, ToTokens};
use syn::visit_mut::{visit_expr_match_mut, VisitMut};
use syn::{parse_macro_input, Arm, ExprMatch, Ident, Item, ItemFn, Pat, Path};

use derive_debug::CustomDebug;

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

    // Following code needed by 05-match-expr, checking #[sorted] attr in functions.
    // Should return the original stream though it is a derive macro and maybe cause duplicate
    // definition, otherwise 04-variants-with-data can not pass because of unused import warnings.
    let mut ret: proc_macro2::TokenStream = input.into();

    let mut all_ident: Vec<Ident> = item_enum.variants.iter().map(|e| e.ident.clone()).collect();
    let all_ident_orig = all_ident.clone();
    all_ident.sort();
    if let Some((orig, sorted)) = all_ident_orig
        .iter()
        .zip(all_ident.iter())
        .find(|(orig, sorted)| orig != sorted)
    {
        ret.extend(
            syn::Error::new(
                sorted.span(),
                format!("{} should sort before {}", sorted, orig),
            )
            .to_compile_error(),
        );
    }
    ret.into()
}

#[proc_macro_attribute]
pub fn check(args: TokenStream, input: TokenStream) -> TokenStream {
    let _ = args;
    let input_clone = input.clone();
    let mut ast = parse_macro_input!(input_clone as ItemFn);
    // panic!("{:#?}", ast);

    // Call visit_item_fn_mut and our overloaded visit_expr_match_mut will be called when
    // caught a match expression.
    let mut tm = TraceMatch {
        not_sorted: None,
        not_support: None,
    };
    tm.visit_item_fn_mut(&mut ast);
    let mut ret: proc_macro2::TokenStream = quote!(#ast);
    // After checking, the modified (removed #[sorted] attr on functions) ast is here, use it as
    // the basic result token stream.

    match &tm.not_sorted {
        Some((orig, sorted)) => {
            // panic!("{:#?}", &sorted.pat);
            let err = match &sorted.pat {
                Pat::Path(e) => wrap_error_stream(PathPat::Path(e.path.clone()), sorted, orig),
                Pat::TupleStruct(e) => {
                    wrap_error_stream(PathPat::Path(e.path.clone()), sorted, orig)
                }
                Pat::Struct(e) => wrap_error_stream(PathPat::Path(e.path.clone()), sorted, orig),
                _ => wrap_error_stream(PathPat::Pat(sorted.pat.clone()), sorted, orig),
            };
            // TODO: Better name resolving by parsing pat.
            ret.extend(err);
        }
        None => {}
    }
    match &tm.not_support {
        Some(arm) => {
            ret.extend(
                syn::Error::new_spanned(&arm.pat, "unsupported by #[sorted]").to_compile_error(),
            );
        }
        None => {}
    }
    ret.into()
}

// Record whether found "not sorted match arms" in checking.
// If so, not_sorted.0 is which arm currently here and not_sorted.1 is which arm should in here.
#[derive(CustomDebug)]
struct TraceMatch {
    not_sorted: Option<(Arm, Arm)>,
    not_support: Option<Arm>,
}

impl VisitMut for TraceMatch {
    // Override `visit_expr_match_mut`.
    // When calling TraceMatch.visit_item_fn_mut, that function will automatically call this
    // overloaded function whenever matched a `ExprMatch`.
    fn visit_expr_match_mut(&mut self, i: &mut ExprMatch) {
        let mut found_sorted_attr = false;
        let mut sorted_attr_index = 0;
        for (pos, attr) in i.attrs.iter().enumerate() {
            if attr.meta.path().to_token_stream().to_string() == "sorted" {
                found_sorted_attr = true;
                sorted_attr_index = pos;
                break;
            }
        }
        // Directly remove #[sorted] attr here because:
        // If sorted, need to remove #[sorted] attr.
        // If not sorted, a compile error returned so removing #[sorted] attr does not matter.
        i.attrs.remove(sorted_attr_index);
        if found_sorted_attr {
            // Found #[sorted]
            let mut arm_vec = vec![];
            for arm in &i.arms {
                // Check Pat type, only support Pat::Path, Pat::TupleStruct, Pat::Struct, and
                // Pat::Ident and Path::Wild (required by 08-underscore).
                // In fact, whether a Pat type is supported is in our control.
                match arm.pat {
                    // Though multiple condition types in the same arm is not supported because
                    // when capturing the inner variable will occur more than one error, when we
                    // do not need the inner variable, using underscore allow to do so.
                    Pat::Path(_)
                    | Pat::TupleStruct(_)
                    | Pat::Struct(_)
                    | Pat::Ident(_)
                    | Pat::Wild(_) => {
                        arm_vec.push(arm);
                    }
                    _ => {
                        self.not_support = Some(arm.clone());
                        return;
                    }
                }
            }
            let arm_vec_orig = arm_vec.clone();
            arm_vec.sort_by(|arm, arm2| pat_to_string(&arm.pat).cmp(&pat_to_string(&arm2.pat)));
            match arm_vec_orig
                .iter()
                .zip(arm_vec.iter())
                .find(|(orig, sorted)| orig != sorted)
            {
                Some((orig, sorted)) => {
                    self.not_sorted = Some(((**orig).clone(), (**sorted).clone()))
                }
                None => visit_expr_match_mut(self, i),
            }
        }
    }
}

// Used to generate error message.
// Remove argument and braces.
// Error::IO(e) => "Error::IO"
fn pat_to_string(pat: &Pat) -> String {
    let orig_str = pat.to_token_stream().to_string();
    match orig_str.find('(') {
        Some(u) => &orig_str[..u],
        None => &orig_str,
    }
    .replace(' ', "")
}

// Combine `Path` and `Pat` together so that we can use them generate compile error
// in the same function.
enum PathPat {
    Path(Path),
    Pat(Pat),
}

// Wrap `Path` or `Pat` to a compile error converted `TokeStream`
// Note that the output is required to be the full path:
// e.g.
// Error::IO(e) => xxx,
// ^^^^^^^^^
// So use `Error::new_spanned` with the `Path` span.
// Actually all types in 06-pattern-path are going in `Path` type, `Pat` is only fallback.
fn wrap_error_stream(e: PathPat, sorted: &Arm, orig: &Arm) -> proc_macro2::TokenStream {
    match e {
        PathPat::Path(p) => syn::Error::new_spanned(
            p,
            format!(
                "{} should sort before {}",
                pat_to_string(&sorted.pat),
                pat_to_string(&orig.pat),
            ),
        ),
        PathPat::Pat(p) => syn::Error::new_spanned(
            p,
            format!(
                "{} should sort before {}",
                pat_to_string(&sorted.pat),
                pat_to_string(&orig.pat),
            ),
        ),
    }
    .to_compile_error()
}
