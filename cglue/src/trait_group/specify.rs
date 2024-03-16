/// Allows specifying type tuple type parameters.
///
/// Only allows specifying tuple elements that have void at the given position.
///
/// This trait is used in trait groups in order to aid specification of associated types.
///
/// # Example
///
/// ```
/// use cglue::trait_group::specify::*;
///
/// // Create a mixed type
/// type Mixed = ((), u8, String, (), &'static str);
/// type Specified = <<Mixed as Specify<U0, u64>>::Type as Specify<U3, f32>>::Type;
///
/// fn take_specified(v: Option<(u64, u8, String, f32, &'static str)>) {}
///
/// let v: Option<Specified> = None;
/// take_specified(v);
/// ```
pub trait Specify<I, T> {
    type Type;
}

/// Actually implements `Specify` trait.
macro_rules! impl_specify0 {
    ($(($pre:ident, $pre_num:ident),)* | ($cur:ident, $cur_num:ident) | $(($next0:ident, $next_num0:ident),)* | ) => {
        impl<$($pre,)* $cur, $($next0),*> Specify<$cur_num, $cur> for ($($pre,)* (), $($next0),*) {
            type Type = ($($pre,)* $cur, $($next0),*);
        }
    };
    ($(($pre:ident, $pre_num:ident),)* | ($cur:ident, $cur_num:ident) | $(($next0:ident, $next_num0:ident),)* | ($next1:ident, $next_num1:ident), $(($next2:ident, $next_num2:ident),)*) => {
        impl<$($pre,)* $cur, $($next0),*> Specify<$cur_num, $cur> for ($($pre,)* (), $($next0),*) {
            type Type = ($($pre,)* $cur, $($next0),*);
        }

        impl_specify0!($(($pre, $pre_num),)* | ($cur, $cur_num) | $(($next0, $next_num0),)* ($next1, $next_num1), | $(($next2, $next_num2),)*);
    }
}

/// Dispatches `Specify` trait implementation.
macro_rules! impl_specify {
    ($(($pre:ident, $pre_num:ident),)* | ($cur:ident, $cur_num:ident) | $(($next:ident, $next_num:ident),)*) => {
        pub struct $cur_num(());

        impl_specify0!($(($pre, $pre_num),)* | ($cur, $cur_num) | | $(($next, $next_num),)*);
        impl_specify!($(($pre, $pre_num),)* ($cur, $cur_num), | | $(($next, $next_num),)*);
    };
    ($(($pre:ident, $pre_num:ident),)* | | ) => {  };
    ($(($pre:ident, $pre_num:ident),)* | | ($cur:ident, $cur_num:ident), $(($next:ident, $next_num:ident),)*) => {
        impl_specify!($(($pre, $pre_num),)* | ($cur, $cur_num) | $(($next, $next_num),)*);
    };
}

impl_specify! {
    | |
    (T0, U0),
    (T1, U1),
    (T2, U2),
    (T3, U3),
    (T4, U4),
    (T5, U5),
    (T6, U6),
    (T7, U7),
    (T8, U8),
    (T9, U9),
    (T10, U10),
    (T11, U11),
    (T12, U12),
    (T13, U13),
    (T14, U14),
    (T15, U15),
    (T16, U16),
}
