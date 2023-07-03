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

    let mut bits_sum = quote!(0);
    let mut bits_current = proc_macro2::TokenStream::new();
    // Suppress warning: unused variables.
    let _ = bits_current;
    for named_field in named_fields {
        match &named_field.ty {
            Type::Path(type_path) => {
                // Find the B* type in xxx::xxx::B*.
                let last_path = type_path.path.segments.last().unwrap();
                let bits_type = last_path.to_token_stream().to_string();
                let bits_type_ident = Ident::new(bits_type.as_str(), last_path.span());
                // Use trait to get bits count later in compile, not here.
                bits_current = quote!(<#bits_type_ident as Specifier>::BITS as usize);
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

        bits_sum = quote!(#bits_sum + #bits_current);
    }

    let mut expand = proc_macro2::TokenStream::new();

    // (<A as Specifier>::BITS +  <B as Specifier>::BITS) % 8
    let mod_result = quote!((#bits_sum) % 8 );

    // Bits to Bytes.
    // Notice that #bits_sum is joined like 1 + 2 + 3,
    // so when converting to Bytes, use (#bits_sum) / 8 which actually is (1 + 2 + 3) /8.
    bits_sum = quote!((#bits_sum) / 8);

    expand.extend(quote!(
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
        const x : [usize; 1] = [0];
        const _ : usize = x[#mod_result];
        // const _ : EightMod8 = if #mod_result == 0 { EightMod8 } else { SevenMod8 };
        // const _ : () = { let check : EightMod8 = if #mod_result == 0 { EightMod8 } else { SevenMod8 };};
        // const _ : () = {
        //     let x = [Box::new(EightMod8)];
        //     let check : Box<dyn TotalSizeIsMultipleOfEightBits> = x[#mod_result];
        // };
    ));

    // Q:
    // 1. How to handle type alias in procedural macro?
    //
    // 2. How to generic an error when some condition triggered and also support type alias?
    // According to builder/05-phantom-data.rs, name resolution (know what type is an ident) is later
    // than macro expansion in rust compiling, makes it impossible to handle type alias.
    //
    // A:
    // 1. Reserve some value like expression in macro.
    //    For example, we want to see how many bits each field consumes in #[bitfield]. Normally we
    //    can calculate that just with type name: `B1` use 1 bit, `B2` use 2 bits...
    //    With type alias, like `type A = B1` in 04-type-parameter, we do not know what `A` actually
    //    stand for, so just use trait system and `BITS` const in `Specifier` trait:
    //      `let bits_sum = (A as Specifier)::BITS + (B as Specifier)::BITS + ...`
    //    In fact the expression above equals to what we calculate from the type name, only difference
    //    is it is calculated later in compiling.
    // 2. Trick is here: https://github.com/zjp-CN/proc-macro-workshop/commit/644637fd5343312b27ff24caa7ece225e25622d0
    //    1) Use `compile_error!` macro does not help because name resolution is after macro expand,
    //       actual value is unknown when compiling `compile_error!` line.
    //    2) Use `panic!` does not help because it only works in running and will not cause compile
    //       to fail.
    //    So use an anonymous const variant `const _ : type = xxx` to achieve this.
    //    Also notice that a const in trait implementation **in `quote!`** does not help this,
    //    maybe because compiling steps.

    expand.into()
}
