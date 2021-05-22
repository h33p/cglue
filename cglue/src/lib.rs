pub mod arc;
pub mod callback;
pub mod option;
pub mod repr_cstring;

//#[cfg(test)]
mod tests {

    use cglue_macro::*;

    use core::convert::TryFrom;

    //#[cglue_derive(TestGroup)]
    struct SA {}
    //#[cglue_derive(TestGroup)]
    struct SB {}

    #[cglue_trait]
    trait TA {
        extern "C" fn ta_1(&self);
    }

    impl TA for SA {
        extern "C" fn ta_1(&self) {}
    }

    impl TA for SB {
        extern "C" fn ta_1(&self) {}
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
        extern "C" fn tc_2(&self);
    }

    impl TC for SA {
        fn tc_1(&self) {}
        extern "C" fn tc_2(&self) {}
    }

    #[test]
    fn call_a() {
        let a = SA {};
        a.ta_1();
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
