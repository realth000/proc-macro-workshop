use proc_macro::TokenStream;

use proc_macro2::Ident;
use quote::{quote, ToTokens};
use syn::spanned::Spanned;
use syn::{
    parse_macro_input, BinOp, Expr, ExprBinary, ExprLit, Fields, FieldsNamed, Item, ItemStruct,
    Lit, LitInt, Type,
};

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
        let bits_type_ident: Ident;

        // Special check for `bool` type.
        // When using `bool`, use `B1` to impl `Specifier` trait
        // and getter/setter use `bool` as return/arg type.
        let mut use_bool = false;

        // Special check for custom type.
        // When using `B1` ~ `B64`, assume that use is using the enum generated in ../src/lib.rs and
        // not special.
        // When using enum with macro `#[derive(BitfieldSpecifier)]`, think it's a custom type
        // and getter/setter use the original type as return/arg type.
        let mut use_custom = false;
        match &named_field.ty {
            Type::Path(type_path) => {
                // Find the B* type in xxx::xxx::B*.
                let last_path = type_path.path.segments.last().unwrap();
                let bits_type = last_path.to_token_stream().to_string();
                // Use trait to get bits count later in compile, not here.
                bits_type_ident = if bits_type == "bool" {
                    use_bool = true;
                    Ident::new("B1", last_path.span())
                } else {
                    // TODO: Fix `B1` ~ `B64` type check.
                    // Here should use some thing like:
                    // const ALL_TYPE: [&str; 64] = seq!(N in 1..65 {"B~N",});
                    // But till now `seq!` does not support that.
                    use_custom = !bits_type.as_str().starts_with('B') && bits_type.len() > 1;
                    Ident::new(bits_type.as_str(), last_path.span())
                };
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

        if use_bool {
            field_method_vec.push(quote!(
                pub fn #bits_field_get_ident(&self) -> bool {
                    self.get_bits_value(#bits_sum, #bits_current) != 0
                }

                pub fn #bits_field_set_ident(&mut self, #bits_field_ident : bool) {
                    match self.set_bits_value(#bits_sum, #bits_current, #bits_field_ident as u64) {
                        Ok(_) => {},
                        Err(e) => eprintln!("failed to set {}:{}",#bits_field_name,e),
                    }
                }
            ));
        } else if use_custom {
            field_method_vec.push(quote!(
                pub fn #bits_field_get_ident(&self) -> #bits_type_ident {
                    #bits_type_ident::from_storage((self.get_bits_value(#bits_sum, #bits_current) as <#bits_type_ident as Specifier>::StorageType))
                }

                pub fn #bits_field_set_ident(&mut self, #bits_field_ident : #bits_type_ident) {
                    match self.set_bits_value(#bits_sum, #bits_current, #bits_field_ident as u64) {
                        Ok(_) => {},
                        Err(e) => eprintln!("failed to set {}:{}",#bits_field_name,e),
                    }
                }
            ));
        } else {
            field_method_vec.push(quote!(
            pub fn #bits_field_get_ident(&self) -> <#bits_type_ident as Specifier>::StorageType {
                self.get_bits_value(#bits_sum, #bits_current) as <#bits_type_ident as Specifier>::StorageType
            }

            pub fn #bits_field_set_ident(&mut self, #bits_field_ident : <#bits_type_ident as Specifier>::StorageType) {
                match self.set_bits_value(#bits_sum, #bits_current, #bits_field_ident as u64) {
                    Ok(_) => {},
                    Err(e) => eprintln!("failed to set {}:{}",#bits_field_name,e),
                }
            }
            ));
        }

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
        const _x : [usize; 1] = [0];
        const _ : usize = _x[#mod_result];
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

#[proc_macro_derive(BitfieldSpecifier)]
pub fn bitfield_specifier(input: TokenStream) -> TokenStream {
    let ast = parse_macro_input!(input as Item);
    let item_enum = if let Item::Enum(item_enum) = &ast {
        item_enum
    } else {
        return compile_error!(
            proc_macro2::Span::call_site(),
            "only support #[derive(BitfieldSpecifier)] on enum"
        );
    };

    let spe_ident = &item_enum.ident;
    let spe_ident_str_ident = spe_ident.to_string();

    let mut variant_vec: Vec<Option<Expr>> = vec![];

    // The following two vectors are used to construct a `match` clause to parse integer back to
    // enums.
    //
    // match v {
    //     x if x == MyEnum::A as i32 => MyEnum::A,
    // }
    //
    // Note that the `x` here should be a value, not a expr, otherwise will fail to compile.
    // So use a tmp value `variant_int_value_tmp_name` to store.
    let mut variant_try_from_vec: Vec<proc_macro2::TokenStream> = vec![];
    let mut variant_try_from_v_vec: Vec<proc_macro2::TokenStream> = vec![];

    // Record last `Path` type index.
    // For auto increase value:
    // 1. enum { Foo1 = F, Foo2, Foo3}: Foo2 = F + 1, so we need to record index of F.
    // 2. enum { Foo1, Foo2, Foo3}: be `None` means it's value equals to index.
    let mut last_variant_pos: Option<usize> = None;
    // Record distance from last `Path` type index.
    //
    // const F : usize = 3;
    // const G : usize = 0;
    //
    // enum Foo {
    //     Foo1 = F, // Last `Path` is here.
    //     Foo2,     // Distance is 1.
    //     Foo3,     // Distance is 2.
    //     Bar1 = G, // Last `Path` update to `G`.
    //     Bar2,     // Distance is 1.
    //     Bar3,     // Distance is 2.
    // }
    //
    let mut last_variant_distance = 1;

    // Check if enum elements count is 2, 4, 8, 16, 32...
    let variant_bits_width = match calculate_2_power(item_enum.variants.len()) {
        Some(v) => v,
        None => {
            return compile_error!(
                item_enum.span(),
                "enum {} 's number of variants should be 1/2/4/8/16/32.. ",
                item_enum.ident.to_string()
            );
        }
    };

    let variant_equal_b_ident = Ident::new(
        format!("B{}", variant_bits_width).as_str(),
        item_enum.span(),
    );

    for (index, variant) in item_enum.variants.iter().enumerate() {
        let v_ident = &variant.ident;
        let variant_int_value_tmp_name = Ident::new(
            to_snake_case(format!("_{}0", v_ident)).as_str(),
            v_ident.span(),
        );

        match &variant.discriminant {
            Some((_, expr)) => {
                match expr {
                    Expr::Lit(lit) => {
                        let t = match &lit.lit {
                            Lit::Int(t) => t,
                            _ => return compile_error!(lit.span(), "expected int value here"),
                        };
                        let variant_value = t.base10_digits();
                        variant_try_from_v_vec.push(quote!(
                            let #variant_int_value_tmp_name = #variant_value.parse::<<#variant_equal_b_ident as Specifier>::StorageType>()
                        ));

                        variant_try_from_vec.push(quote!(
                            #variant_int_value_tmp_name if #variant_int_value_tmp_name == #spe_ident::#v_ident as <#variant_equal_b_ident as Specifier>::StorageType => #spe_ident::#v_ident
                        ));
                        variant_vec.push(Some(Expr::Lit(lit.clone())));
                    }
                    Expr::Path(path) => {
                        last_variant_pos = Some(index);
                        last_variant_distance = 1;
                        variant_try_from_v_vec.push(quote!(
                            let #variant_int_value_tmp_name = #path
                        ));

                        variant_try_from_vec.push(quote!(
                            #variant_int_value_tmp_name if #variant_int_value_tmp_name == #spe_ident::#v_ident as <#variant_equal_b_ident as Specifier>::StorageType => #spe_ident::#v_ident
                        ));
                        variant_vec.push(Some(Expr::Path(path.clone())));
                    }
                    _ => {
                        return compile_error!(
                            expr.span(),
                            "expected value or literal integer here"
                        );
                    }
                };
            }
            None => {
                // Push a `None` to keep value with true index.
                variant_vec.push(None);
                match last_variant_pos {
                    /*
                    Expr::Binary {
                        attrs: [],
                        left: Expr::Path {
                            attrs: [],
                            qself: None,
                            path: Path {
                                leading_colon: None,
                                segments: [
                                    PathSegment {
                                        ident: Ident {
                                            ident: "F",
                                            span: #0 bytes(1133..1134),
                                        },
                                        arguments: PathArguments::None,
                                    },
                                ],
                            },
                        },
                        op: BinOp::Add(
                            Plus,
                        ),
                        right: Expr::Lit {
                            attrs: [],
                            lit: Lit::Int {
                                token: 2,
                            },
                        },
                    },
                     */
                    Some(v) => {
                        // Have value before, push a Expr::Binary looks like `F + 1`.
                        let former_value = variant_vec[v].as_ref().unwrap();
                        let expr_binary = Expr::Binary(ExprBinary {
                            attrs: vec![],
                            left: Box::new(former_value.clone()),
                            op: BinOp::Add(syn::token::Plus {
                                spans: [variant.span()],
                            }),
                            right: Box::new(Expr::Lit(ExprLit {
                                attrs: vec![],
                                lit: Lit::Int(LitInt::new(
                                    format!("{}", last_variant_distance).as_str(),
                                    variant.span(),
                                )),
                            })),
                        });
                        variant_try_from_v_vec.push(quote!(
                            let #variant_int_value_tmp_name = #expr_binary
                        ));
                        variant_try_from_vec.push(quote!(
                            #variant_int_value_tmp_name if #variant_int_value_tmp_name == #spe_ident::#v_ident as <#variant_equal_b_ident as Specifier>::StorageType => #spe_ident::#v_ident
                        ));
                        last_variant_distance += 1;
                    }
                    None => {
                        // If no value before.
                        // let x = ExprLit {
                        //     attrs: vec![],
                        //     lit: Lit::Int(LitInt::new(0), ..),
                        // };
                        let expr_lit = Expr::Lit(ExprLit {
                            attrs: vec![],
                            lit: Lit::Int(LitInt::new(
                                format!("{}", index).as_str(),
                                variant.span(),
                            )),
                        });
                        variant_try_from_vec.push(quote!(
                            let #variant_int_value_tmp_name = #expr_lit;
                        ));
                        variant_try_from_vec.push(quote!(
                            #variant_int_value_tmp_name if #variant_int_value_tmp_name == #spe_ident::#v_ident as <#variant_equal_b_ident as Specifier>::StorageType() => #spe_ident::v_ident
                        ));
                    }
                }
            }
        };
    }

    let mut expand = proc_macro2::TokenStream::new();

    // Implement `Specifier` trait.
    expand.extend(quote!(
        impl Specifier for #spe_ident {
            const BITS: i32 = #variant_bits_width as i32;
            type StorageType = <#variant_equal_b_ident as Specifier>::StorageType;
        }

        impl #spe_ident {
            pub fn from_storage(i: <#variant_equal_b_ident as Specifier>::StorageType) -> Self {
                #(#variant_try_from_v_vec;)*
                match i {
                    #(#variant_try_from_vec,)*
                     _ => panic!("bitfield: invalid enum {} value {}", #spe_ident_str_ident, i)
                }
            }
        }
    ));

    expand.extend(quote!());

    expand.into()
}

fn calculate_2_power(value: usize) -> Option<u32> {
    if value == 0 || (value & (value - 1)) != 0 {
        return None;
    }
    let mut times = 0;
    let mut v = value;
    while v > 1 {
        v >>= 1;
        times += 1;
    }
    Some(times)
}

fn to_snake_case(s: String) -> String {
    let mut ret = String::new();
    for (index, ch) in s.chars().enumerate() {
        if ch.is_uppercase() {
            let chl = ch.to_lowercase().collect::<Vec<_>>()[0];
            if index == 0 {
                ret.push(chl);
            } else {
                ret.push('_');
                ret.push(chl);
            }
        } else {
            ret.push(ch);
        }
    }
    ret
}
