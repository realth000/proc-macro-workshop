use proc_macro::TokenStream;

use proc_macro2::Ident;
use quote::{quote, ToTokens};
use syn::spanned::Spanned;
use syn::{parse_macro_input, Fields, FieldsNamed, Item, ItemStruct, Type};

macro_rules! compile_error {
    ($span: expr, $($arg: tt)*) => {
        syn::Error::new($span, format!($($arg)*))
            .to_compile_error()
            .into()
    };
}

#[proc_macro_attribute]
pub fn bitfield(args: TokenStream, input: TokenStream) -> TokenStream {
    let _ = args;
    let _ = input;
    let ast = parse_macro_input!(input as Item);
    // panic!("{:#?}", ast);

    let item_struct = if let Item::Struct(item_struct) = &ast {
        item_struct
    } else {
        return compile_error!(
            proc_macro2::Span::call_site(),
            "#[bitfield] only support struct types"
        );
    };

    let name_ident = Ident::new(
        item_struct.ident.to_token_stream().to_string().as_str(),
        item_struct.span(),
    );

    let named_fields = if let ItemStruct {
        fields:
            Fields::Named(FieldsNamed {
                named: named_fields,
                ..
            }),
        ..
    } = &item_struct
    {
        named_fields
    } else {
        return compile_error!(
            proc_macro2::Span::call_site(),
            "expected named fields in this struct"
        );
    };

    let mut bits_sum: usize = 0;
    for named_field in named_fields {
        match &named_field.ty {
            Type::Path(type_path) => {
                // Find the B* type in xxx::xxx::B*.
                let last_path = type_path.path.segments.last().unwrap();
                let bits_type = last_path.to_token_stream().to_string();
                if !bits_type.starts_with('B') {
                    return compile_error!(last_path.ident.span(), "expected B* type here");
                }
                match bits_type[1..].parse::<usize>() {
                    Ok(v) => bits_sum += v,
                    Err(e) => {
                        return compile_error!(
                            last_path.ident.span(),
                            "failed to parse bits size{}",
                            e
                        );
                    }
                };
                // let bits = <bits_type as Specifier>::BITS;
            }
            _ => {
                continue;
            }
        }
    }

    // Bits to Bytes.
    bits_sum /= 8;

    quote!(
        pub struct #name_ident {
            data: [u8; #bits_sum],
        }
    )
    .into()
}
