//! `Repr(C)` tuples

macro_rules! make_tuple {
(
    attrs[$(#[$attr: meta])*]
    $name: ident[$( $param: ident ),* $(,)? ]
) => (
    #[derive(Debug, Clone, Copy, PartialEq, Eq, Ord, PartialOrd, Hash)]
    #[cfg_attr(feature = "abi_stable", derive(::abi_stable::StableAbi))]
    #[cfg_attr(feature = "serde", derive(::serde::Serialize, ::serde::Deserialize))]
    #[repr(C)]
    $(#[$attr])*
    pub struct $name< $($param,)* > (
        $(pub $param,)*
    );

    impl<$($param,)*> From<($($param,)*)> for $name<$($param,)*> {
        #[allow(non_snake_case)]
        fn from(($($param,)*): ($($param,)*)) -> Self {
            Self($($param,)*)
        }
    }

    impl<$($param,)*> From<$name<$($param,)*>> for ($($param,)*) {
        #[allow(non_snake_case)]
        fn from($name($($param,)*): $name<$($param,)*>) -> Self {
            ($($param,)*)
        }
    }

    impl< $($param,)* > $name<$($param,)*>{
        /// Convert this C tuple into Rust tuple.
        #[inline]
        pub fn into_tuple(self)-> ($($param,)*) {
            self.into()
        }
    }
)}

make_tuple!(
    attrs[
        /// FFI-safe 1 element tuple.
    ]
    CTup1[A]
);

make_tuple!(
    attrs[
        /// FFI-safe 2 element tuple.
    ]
    CTup2[A, B]
);

make_tuple!(
    attrs[
        /// FFI-safe 3 element tuple.
    ]
    CTup3[A, B, C]
);

make_tuple!(
    attrs[
        /// FFI-safe 4 element tuple.
    ]
    CTup4[A, B, C, D]
);
