use proc_macro2::TokenStream;
use quote::{format_ident, quote};
use std::collections::HashMap;
use syn::{Ident, Path};

pub fn get_impl(parent_path: &Path, out: &mut Vec<(Path, TokenStream)>) {
    let cur_path = super::super::join_paths(parent_path, format_ident!("sink"));

    let crate_path = crate::util::crate_path();

    out.push((
        cur_path,
        quote! {
            pub trait Sink<Item> {
                type Error;

                #[custom_impl(
                    // Types within the C interface other than self and additional wrappers.
                    {
                        cx: &#crate_path::task::CRefWaker,
                        out: &mut ::core::mem::MaybeUninit<Self::Error>,
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
                            match this.poll_ready(&mut cx) {
                                ::core::task::Poll::Ready(Ok(())) => 1,
                                ::core::task::Poll::Ready(Err(e)) => {
                                    out.write(e);
                                    2
                                }
                                _ => 0
                            }
                        })
                    },
                    // This part is processed in the trait impl after the call returns (impl_func_ret).
                    {
                        if ret == 1 {
                            ::core::task::Poll::Ready(Ok(()))
                        } else if ret == 2 {
                            ::core::task::Poll::Ready(Err(unsafe { out_v.assume_init() }))
                        } else {
                            ::core::task::Poll::Pending
                        }
                    },
                )]
                fn poll_ready(self: ::core::pin::Pin<&mut Self>, cx: &mut ::core::task::Context) -> ::core::task::Poll<Result<(), Self::Error>>;

                #[custom_impl(
                    // Types within the C interface other than self and additional wrappers.
                    {
                        item: Item,
                        out: &mut ::core::mem::MaybeUninit<Self::Error>,
                    },
                    // Unwrapped return type
                    u8,
                    // Conversion in trait impl to C arguments (signature names are expected).
                    {
                        let mut out_v = ::core::mem::MaybeUninit::uninit();
                        let out = &mut out_v;
                    },
                    // This is the body of C impl minus the automatic wrapping.
                    {
                        match this.start_send(item) {
                            Ok(()) => 0,
                            Err(e) => {
                                out.write(e);
                                1
                            }
                        }
                    },
                    // This part is processed in the trait impl after the call returns (impl_func_ret).
                    {
                        if ret == 0 {
                            Ok(())
                        } else {
                            Err(unsafe { out_v.assume_init() })
                        }
                    },
                )]
                fn start_send(self: ::core::pin::Pin<&mut Self>, item: Item) -> Result<(), Self::Error>;

                #[custom_impl(
                    // Types within the C interface other than self and additional wrappers.
                    {
                        cx: &#crate_path::task::CRefWaker,
                        out: &mut ::core::mem::MaybeUninit<Self::Error>,
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
                            match this.poll_flush(&mut cx) {
                                ::core::task::Poll::Ready(Ok(())) => 1,
                                ::core::task::Poll::Ready(Err(e)) => {
                                    out.write(e);
                                    2
                                }
                                _ => 0
                            }
                        })
                    },
                    // This part is processed in the trait impl after the call returns (impl_func_ret).
                    {
                        if ret == 1 {
                            ::core::task::Poll::Ready(Ok(()))
                        } else if ret == 2 {
                            ::core::task::Poll::Ready(Err(unsafe { out_v.assume_init() }))
                        } else {
                            ::core::task::Poll::Pending
                        }
                    },
                )]
                fn poll_flush(self: ::core::pin::Pin<&mut Self>, cx: &mut ::core::task::Context) -> ::core::task::Poll<Result<(), Self::Error>>;

                #[custom_impl(
                    // Types within the C interface other than self and additional wrappers.
                    {
                        cx: &#crate_path::task::CRefWaker,
                        out: &mut ::core::mem::MaybeUninit<Self::Error>,
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
                            match this.poll_close(&mut cx) {
                                ::core::task::Poll::Ready(Ok(())) => 1,
                                ::core::task::Poll::Ready(Err(e)) => {
                                    out.write(e);
                                    2
                                }
                                _ => 0
                            }
                        })
                    },
                    // This part is processed in the trait impl after the call returns (impl_func_ret).
                    {
                        if ret == 1 {
                            ::core::task::Poll::Ready(Ok(()))
                        } else if ret == 2 {
                            ::core::task::Poll::Ready(Err(unsafe { out_v.assume_init() }))
                        } else {
                            ::core::task::Poll::Pending
                        }
                    },
                )]
                fn poll_close(self: ::core::pin::Pin<&mut Self>, cx: &mut ::core::task::Context) -> ::core::task::Poll<Result<(), Self::Error>>;

            }
        },
    ));
}

pub fn get_exports(parent_path: &Path, exports: &mut HashMap<Ident, Path>) {
    let cur_path = super::super::join_paths(parent_path, format_ident!("sink"));
    exports.insert(format_ident!("Sink"), cur_path);
}
