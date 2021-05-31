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
//!     let obj = trait_obj!(&mut info as InfoPrinter);
//!
//!     use_info_printer(&obj);
//! }
//! ```
//!
//! A CGlue object is ABI-safe, meaning it can be used across FFI-boundary - C code, or dynamically loaded Rust libraries. While Rust does not guarantee your code will work with 2 different compiler versions clashing, CGlue glues it all together in a way that works.
//!
//! This is done by generating wrapper vtables (virtual function tables) for the specified trait, and creating an opaque object with matching table. Here is what's behind the `trait_obj` macro:
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
pub mod boxed;
pub mod callback;
pub mod option;
pub mod repr_cstring;
pub mod result;
pub mod trait_group;

//#[cfg(test)]
pub mod tests {

    use cglue_macro::*;

    pub struct SA {}
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
        extern "C" fn tb_1(&self, val: usize) -> usize;
        fn tb_2(&self, val: usize) -> usize;
    }

    impl TB for SB {
        extern "C" fn tb_1(&self, val: usize) -> usize {
            val * 2
        }

        fn tb_2(&self, val: usize) -> usize {
            val * val
        }
    }

    #[cglue_trait]
    #[int_result]
    pub trait TC {
        fn tc_1(&self);
        extern "C" fn tc_2(&mut self);
        fn tc_3(&mut self, slc: &[usize], npo_option: Option<&u128>, wrap_option: Option<u128>);
        fn tc_4(&self, _npo_opt: Option<&usize>) {}
        fn tc_5(&self, _opt: Option<usize>) {}
        fn tc_6(&self, val: usize) -> Result<usize, ()> {
            Ok(val)
        }
    }

    impl TC for SA {
        fn tc_1(&self) {}
        extern "C" fn tc_2(&mut self) {}
        fn tc_3(&mut self, _slc: &[usize], _npo_option: Option<&u128>, _wrap_option: Option<u128>) {
        }
    }

    #[test]
    fn call_a() {
        let a = SA {};
        let mut b = SB {};
        let c = SB {};

        let obja = trait_obj!(&a as TA);
        let objb = trait_obj!(&mut b as TA);
        let objc = trait_obj!(c as TA);

        assert_eq!(obja.ta_1() + objb.ta_1() + objc.ta_1(), 17);
    }

    cglue_trait_group!(TestGroup, TA, { TB, TC });
    cglue_impl_group!(SA, TestGroup, { TC });
    cglue_impl_group!(SB, TestGroup, { TB });

    #[test]
    fn get_b() {
        let b = SB {};

        let objb = trait_obj!(b as TB);

        assert_eq!(objb.tb_2(objb.tb_1(10)), 400);
    }

    #[test]
    fn test_group() {
        let a = SA {};

        let _ = group_obj!(&a as TestGroup);

        let group = group_obj!(a as TestGroup);

        {
            let group = as_ref!(group impl TC).unwrap();
            group.tc_1();
        }

        assert!(!check!(group impl TB));

        let cast = cast!(group impl TC).unwrap();

        let mut group = TestGroup::from(cast);

        assert!(as_mut!(group impl TB).is_none());
    }

    #[test]
    fn test_group_2() {
        let mut b = SB {};

        let group = group_obj!(&mut b as TestGroup);
        assert!(check!(group impl TB));

        let group = group_obj!(&b as TestGroup);
        assert!(check!(group impl TB));

        let group = group_obj!(b as TestGroup);
        assert!(check!(group impl TB));
    }

    #[no_mangle]
    pub extern "C" fn brrr(
        _a: CGlueOpaqueTraitObjTB,
        _b: CGlueRefOpaqueTraitObjTB,
        _c: CGlueMutOpaqueTraitObjTB,
        _d: TestGroupOpaqueRef,
        _e: TestGroupOpaqueMut,
        _f: TestGroupOpaqueBox,
    ) {
    }
}
