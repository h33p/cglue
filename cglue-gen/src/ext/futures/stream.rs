use proc_macro2::TokenStream;
use quote::{format_ident, quote};
use std::collections::HashMap;
use syn::{Ident, Path};

pub fn get_impl(parent_path: &Path, out: &mut Vec<(Path, TokenStream)>) {
    let cur_path = super::super::join_paths(parent_path, format_ident!("stream"));

    let crate_path = crate::util::crate_path();

    out.push((
        cur_path,
        quote! {
            pub trait Stream {
                type Item;

                #[custom_impl(
                    // Types within the C interface other than self and additional wrappers.
                    {
                        cx: &#crate_path::task::CRefWaker,
                        out: &mut ::core::mem::MaybeUninit<Self::Item>,
                    },
                    // Unwrapped return type
                    u8,
                    // Conversion in trait impl to C arguments (signature names are expected).
                    {
                        let mut out_v = ::core::mem::MaybeUninit::uninit();
                        let out = &mut out_v;
                        let cx = #crate_path::task::CRefWaker::from(cx.waker());
                        let cx = &cx;
                    },
                    // This is the body of C impl minus the automatic wrapping.
                    {
                        cx.with_waker(|waker| {
                            let mut cx = ::core::task::Context::from_waker(waker);
                            match this.poll_next(&mut cx) {
                                ::core::task::Poll::Ready(Some(v)) => {
                                    out.write(v);
                                    1
                                }
                                ::core::task::Poll::Ready(None) => 2,
                                _ => 0
                            }
                        })
                    },
                    // This part is processed in the trait impl after the call returns (impl_func_ret).
                    {
                        if ret == 1 {
                            ::core::task::Poll::Ready(Some(unsafe { out_v.assume_init() }))
                        } else if ret == 2 {
                            ::core::task::Poll::Ready(None)
                        } else {
                            ::core::task::Poll::Pending
                        }
                    },
                )]
                fn poll_next(self: ::core::pin::Pin<&mut Self>, cx: &mut ::core::task::Context) -> ::core::task::Poll<Option<Self::Item>>;
            }
        },
    ));
}

pub fn get_exports(parent_path: &Path, exports: &mut HashMap<Ident, Path>) {
    let cur_path = super::super::join_paths(parent_path, format_ident!("stream"));
    exports.insert(format_ident!("Stream"), cur_path);
}
