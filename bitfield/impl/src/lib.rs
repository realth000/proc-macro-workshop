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

// Notice that this is a attribute macro, it will replace the `input` TokenStream!
// See more in sorted/01-parse-enum.rs
#[proc_macro_attribute]
pub fn bitfield(args: TokenStream, input: TokenStream) -> TokenStream {
    let _ = args;
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

    let mut field_method_vec: Vec<proc_macro2::TokenStream> = vec![];

    let mut bits_sum: usize = 0;
    let mut bits_current: usize = 0;
    // Suppress warning: unused variables.
    let _ = bits_current;
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
                    Ok(v) => {
                        // bits_current = u8::try_from(v).unwrap();
                        bits_current = v;
                    }
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

        // Build getter and setter.
        let bits_field_name = named_field.ident.to_token_stream().to_string();
        let bits_field_ident = Ident::new(bits_field_name.as_str(), named_field.span());
        let bits_field_get_ident = Ident::new(
            format!("get_{}", bits_field_name.as_str()).as_str(),
            named_field.span(),
        );
        let bits_field_set_ident = Ident::new(
            format!("set_{}", bits_field_name.as_str()).as_str(),
            named_field.span(),
        );

        field_method_vec.push(quote!(
            pub fn #bits_field_get_ident(&self) -> u64 {
                self.get_bits_value(#bits_sum, #bits_current)
            }

            pub fn #bits_field_set_ident(&mut self, #bits_field_ident : u64) {
                match self.set_bits_value(#bits_sum, #bits_current,#bits_field_ident) {
                    Ok(_) => {},
                    Err(e) => eprintln!("failed to set {}:{}",#bits_field_name,e),
                }
            }
        ));

        bits_sum += bits_current;
    }

    // Bits to Bytes.
    bits_sum /= 8;

    quote!(
        #[repr(C)]
        pub struct #name_ident {
            data: [u8; #bits_sum],
        }

        impl #name_ident {
            pub fn new() -> Self {
                #name_ident {
                    data: [0; #bits_sum]
                }
            }

            #(#field_method_vec)*
        }

        impl BitParse for #name_ident {
            type Data = [u8; #bits_sum];

            fn get_data(&self) -> &Self::Data {
               &self.data
            }

            fn get_mut_data(&mut self) -> &mut Self::Data {
               &mut self.data
            }
        }
    )
    .into()
}
