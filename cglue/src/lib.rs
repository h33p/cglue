//!
//! # CGlue
//!
//! If all code is glued together, our glue is the safest in the market.
//!
//! ## FFI-safe trait generation, helper structures, and more!
//!
//! CGlue offers an easy way to ABI (application binary interface) safety. Just a few annotations and your trait is ready to go!
//!
//! ```rust
//! use cglue_macro::*;
//!
//! #[cglue_trait]
//! pub trait InfoPrinter {
//!     fn print_info(&self);
//! }
//!
//! struct Info {
//!     value: usize
//! }
//!
//! impl InfoPrinter for Info {
//!     fn print_info(&self) {
//!         println!("Info struct: {}", self.value);
//!     }
//! }
//!
//! fn use_info_printer(printer: &impl InfoPrinter) {
//!     println!("Printing info:");
//!     printer.print_info();
//! }
//!
//! fn main() {
//!     let mut info = Info {
//!         value: 5
//!     };
//!
//!     let obj = cglue_obj!(info as InfoPrinter);
//!
//!     use_info_printer(&obj);
//! }
//! ```
//!
//! A CGlue object is ABI-safe, meaning it can be used across FFI-boundary - C code, or dynamically loaded Rust libraries. While Rust does not guarantee your code will work with 2 different compiler versions clashing, CGlue glues it all together in a way that works.
//!
//! This is done by generating wrapper vtables (virtual function tables) for the specified trait, and creating an opaque object with matching table. Here is what's behind the `cglue_obj` macro:
//!
//! ```ignore
//! let obj = CGlueTraitObjInfoPrinter::from(&mut info).into_opaque();
//! ```
//!
//! `cglue_trait` annotation generates a `CGlueVtblInfoPrinter` structure, and all the code needed to construct it for a type implementing the `InfoPrinter` trait. Then, a `CGlueTraitObj` is constructed that wraps the input object and implements the `InfoPrinter` trait.
//!
//! But that's not all, you can also group traits together!
//!
//! ```rust
//!
//! ```

pub mod arc;
pub mod callback;
pub mod option;
pub mod repr_cstring;
pub mod trait_group;

//#[cfg(test)]
pub mod tests {

    use cglue_macro::*;

    //#[cglue_derive(TestGroup)]
    struct SA {}
    //#[cglue_derive(TestGroup)]
    struct SB {}

    #[cglue_trait]
    pub trait TA {
        extern "C" fn ta_1(&self) -> usize;
    }

    impl TA for SA {
        extern "C" fn ta_1(&self) -> usize {
            5
        }
    }

    impl TA for SB {
        extern "C" fn ta_1(&self) -> usize {
            6
        }
    }

    #[cglue_trait]
    pub trait TB {
        extern "C" fn tb_1(&self);
    }

    impl TB for SB {
        extern "C" fn tb_1(&self) {}
    }

    #[cglue_trait]
    pub trait TC {
        fn tc_1(&self);
        extern "C" fn tc_2(&mut self);
    }

    impl TC for SA {
        fn tc_1(&self) {}
        extern "C" fn tc_2(&mut self) {}
    }

    #[test]
    fn call_a() {
        let mut a = SA {};
        let mut b = SB {};

        let obja = cglue_obj!(a as TA);
        let objb = CGlueTraitObjTA::from(&mut b).into_opaque();

        assert_eq!(obja.ta_1() + objb.ta_1(), 11);
    }

    cglue_trait_group!(TestGroup, TA, { TB, TC });
    //cglue_impl_group!(SA, TestGroup, TA, { TB, TC });
    //cglue_impl_group!(SB, TestGroup, TA, { TB, TC });

    #[test]
    fn get_b() {
        let a = SA {};
        let b = SB {};
    }
}
