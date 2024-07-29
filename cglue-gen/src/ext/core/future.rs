use proc_macro2::TokenStream;
use quote::{format_ident, quote};
use std::collections::HashMap;
use syn::{Ident, Path};

pub fn get_impl(parent_path: &Path, out: &mut Vec<(Path, TokenStream)>) {
    let cur_path = super::super::join_paths(parent_path, format_ident!("future"));

    let crate_path = crate::util::crate_path();

    out.push((
        cur_path,
        quote! {
            pub trait Future {
                type Output;

                #[custom_impl(
                    // Types within the C interface other than self and additional wrappers.
                    {
                        cx: &#crate_path::task::CRefWaker,
                        out: &mut ::core::mem::MaybeUninit<Self::Output>,
                    },
                    // Unwrapped return type
                    bool,
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
                            match this.poll(&mut cx) {
                                ::core::task::Poll::Ready(v) => {
                                    out.write(v);
                                    true
                                }
                                _ => false
                            }
                        })
                    },
                    // This part is processed in the trait impl after the call returns (impl_func_ret).
                    {
                        if ret {
                            ::core::task::Poll::Ready(unsafe { out_v.assume_init() })
                        } else {
                            ::core::task::Poll::Pending
                        }
                    },
                )]
                fn poll(self: ::core::pin::Pin<&mut Self>, cx: &mut ::core::task::Context) -> ::core::task::Poll<Self::Output>;
            }
        },
    ));
}

pub fn get_exports(parent_path: &Path, exports: &mut HashMap<Ident, Path>) {
    let cur_path = super::super::join_paths(parent_path, format_ident!("future"));
    exports.insert(format_ident!("Future"), cur_path);
}
