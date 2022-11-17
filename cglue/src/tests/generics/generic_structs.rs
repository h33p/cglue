use super::super::simple::trait_defs::*;
use crate::*;

#[derive(Clone, Default)]
pub struct GA<T> {
    val: T,
}

#[cglue_trait]
pub trait Getter<T> {
    fn get_val(&self) -> &T;
}

impl<T> Getter<T> for GA<T> {
    fn get_val(&self) -> &T {
        &self.val
    }
}

impl TA for GA<usize> {
    extern "C" fn ta_1(&self) -> usize {
        self.val
    }
}

#[derive(Clone)]
pub struct Lifetimed<'a, T> {
    val: &'a T,
}

impl<'a, T> Getter<T> for Lifetimed<'a, T> {
    fn get_val(&self) -> &T {
        self.val
    }
}

impl<'a> TA for Lifetimed<'a, usize> {
    extern "C" fn ta_1(&self) -> usize {
        *self.val
    }
}

cglue_trait_group!(GenGroup<T: Eq>, Getter<T>, { TA });
cglue_impl_group!(GA<T: Eq>, GenGroup<T>, { TA });
cglue_impl_group!(GA<T = u64>, GenGroup<T>, {});
// Internally, the macro prefers to emit 'cglue_a and both of these cases should "just work"
cglue_impl_group!(Lifetimed<'a, T: Eq>, GenGroup<T>, { TA });
cglue_impl_group!(Lifetimed<'cglue_a, T = u64>, GenGroup<T>, {});

#[test]
fn use_getter() {
    let ga = GA { val: 50usize };

    let obj = trait_obj!(ga as Getter);

    assert_eq!(*obj.get_val(), 50);
}

#[test]
fn gen_clone() {
    let ga = GA { val: 50usize };

    let obj = trait_obj!(ga as Clone);

    let _ = obj.clone();
}

#[test]
fn use_ta() {
    let ga = GA::default();

    let obj = trait_obj!(ga as TA);

    assert_eq!(obj.ta_1(), 0);
}

#[test]
fn use_group() {
    let ga = GA::<usize>::default();
    let group = group_obj!(ga as GenGroup);
    assert!(cast!(group impl TA).is_some());

    let ga = GA::<u64>::default();
    let group = group_obj!(ga as GenGroup);
    assert!(cast!(group impl TA).is_none());
}
