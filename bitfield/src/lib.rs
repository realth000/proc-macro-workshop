// Crates that have the "proc-macro" crate type are only allowed to export
// procedural macros. So we cannot have one crate that defines procedural macros
// alongside other types of public APIs like traits and structs.
//
// For this project we are going to need a #[bitfield] macro but also a trait
// and some structs. We solve this by defining the trait and structs in this
// crate, defining the attribute macro in a separate bitfield-impl crate, and
// then re-exporting the macro from this crate so that users only have one crate
// that they need to import.
//
// From the perspective of a user of this crate, they get all the necessary APIs
// (macro, trait, struct) through the one bitfield crate.
pub use bitfield_checks::*;
pub use bitfield_impl::{bitfield, BitfieldSpecifier};
pub use bitfield_parse::BitParse;
use seq::seq;

// BITS is a constant for every B* form B1 to B64, shows actually how many bits used.
pub trait Specifier {
    const BITS: i32;
    type StorageType;
}

// Definite B1 to B64.
// Each B* should set the actual bits size.
seq!(N in 1..9 {
    pub enum B~N {
    }

    impl Specifier for B~N {
        const BITS: i32 = N;
        type StorageType = u8;
    }
});

seq!(N in 9..17 {
    pub enum B~N {
    }

    impl Specifier for B~N {
        const BITS: i32 = N;
        type StorageType = u16;
    }
});

seq!(N in 17..33 {
    pub enum B~N {
    }

    impl Specifier for B~N {
        const BITS: i32 = N;
        type StorageType = u32;
    }
});

seq!(N in 33..65 {
    pub enum B~N {
    }

    impl Specifier for B~N {
        const BITS: i32 = N;
        type StorageType = u64;
    }
});
