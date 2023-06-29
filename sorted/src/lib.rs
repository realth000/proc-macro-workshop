use proc_macro::TokenStream;

use quote::{quote, ToTokens};
use syn::spanned::Spanned;
use syn::visit_mut::{visit_expr_match_mut, VisitMut};
use syn::{parse_macro_input, Arm, ExprMatch, Ident, Item, ItemFn};

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

    // Call visit_item_fn_mut and our overloaded visit_expr_match_mut will be called when
    // caught a match expression.
    let mut tm = TraceMatch { not_sorted: None };
    tm.visit_item_fn_mut(&mut ast);
    let mut ret: proc_macro2::TokenStream = quote!(#ast);
    // After checking, the modified (removed #[sorted] attr on functions) ast is here, use it as
    // the basic result token stream.
    match &tm.not_sorted {
        Some((orig, sorted)) => {
            // TODO: Better name resolving by parsing pat.
            let u1 = sorted.pat.to_token_stream().to_string().find('(').unwrap();
            let u2 = orig.pat.to_token_stream().to_string().find('(').unwrap();
            ret.extend(
                syn::Error::new(
                    sorted.span(),
                    format!(
                        "{} should sort before {}",
                        &sorted.pat.to_token_stream().to_string()[..u1],
                        &orig.pat.to_token_stream().to_string()[..u2]
                    ),
                )
                .to_compile_error(),
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
                arm_vec.push(arm);
            }
            let arm_vec_orig = arm_vec.clone();
            arm_vec.sort_by(|arm, arm2| {
                // TODO: Better comparing by parsing pat.
                // Should use "Io" and "Fmt" to compare, now using "Io(e)" and "Fmt(e).
                let p = arm.pat.to_token_stream().to_string();
                let p2 = arm2.pat.to_token_stream().to_string();
                p.cmp(&p2)
            });
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
