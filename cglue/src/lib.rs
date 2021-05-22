pub mod arc;
pub mod callback;
pub mod option;
pub mod repr_cstring;
pub mod trait_group;

//#[cfg(test)]
mod tests {

    use crate::trait_group::*;
    use cglue_macro::*;

    use core::convert::TryFrom;

    //#[cglue_derive(TestGroup)]
    struct SA {}
    //#[cglue_derive(TestGroup)]
    struct SB {}

    #[cglue_trait]
    trait TA {
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
    trait TB {
        extern "C" fn tb_1(&self);
    }

    impl TB for SB {
        extern "C" fn tb_1(&self) {}
    }

    #[cglue_trait]
    trait TC {
        fn tc_1(&self);
        extern "C" fn tc_2(&mut self);
    }

    impl TC for SA {
        fn tc_1(&self) {}
        extern "C" fn tc_2(&mut self) {}
    }

    fn brrr() {
        let mut a = SA {};
        let obj = CGlueTraitObj::<_, CGlueVtblTC<_>>::from(&mut a).into_opaque();
    }

    #[test]
    fn call_a() {
        let mut a = SA {};
        let mut b = SB {};

        let obja = CGlueTraitObj::<_, CGlueVtblTA<_>>::from(&mut a).into_opaque();
        let objb = CGlueTraitObj::<_, CGlueVtblTA<_>>::from(&mut b).into_opaque();

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
