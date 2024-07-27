use cglue_macro::*;

#[test]
fn use_stream() {
    use futures::stream::StreamExt;

    let items = [42, 43, 42];

    let obj = trait_obj!(futures::stream::iter(items) as Stream);

    impl_stream(&obj);

    assert_eq!(pollster::block_on(obj.collect::<Vec<_>>()), items);
}

#[cfg(test)]
fn impl_stream(_: &impl ::futures::Stream) {}

#[test]
fn use_sink() {
    use futures::sink::SinkExt;

    let items = [42, 43, 42];

    let sink = futures::sink::unfold(0, |idx, elem: i32| async move {
        assert_eq!(elem, items[idx]);
        Ok::<_, usize>(idx + 1)
    });

    let mut obj = trait_obj!(sink as Sink);

    impl_sink(&obj);

    pollster::block_on(async {
        for i in items {
            obj.send(i).await.unwrap();
        }
    });

    // The logic of the sink should force a panic afterwards
    assert!(
        std::panic::catch_unwind(std::panic::AssertUnwindSafe(move || {
            pollster::block_on(obj.send(0))
        }))
        .is_err()
    );
}

#[cfg(test)]
fn impl_sink<T>(_: &impl ::futures::Sink<T>) {}
