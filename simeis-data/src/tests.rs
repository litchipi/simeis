use rand::{rngs::SmallRng, RngExt, SeedableRng};

/// Minimal single-threaded executor used to drive the crate's `async` functions
/// in unit tests without pulling in a full async runtime dependency.
///
/// The `mea` locks used throughout the crate resolve immediately when
/// uncontended, so a busy-poll loop with a no-op waker is enough to run them
/// to completion inside synchronous `#[test]` functions.
pub fn block_on<F: std::future::Future>(fut: F) -> F::Output {
    use std::task::{Context, Poll, RawWaker, RawWakerVTable, Waker};

    fn noop(_: *const ()) {}
    fn clone(_: *const ()) -> RawWaker {
        RawWaker::new(std::ptr::null(), &VTABLE)
    }
    static VTABLE: RawWakerVTable = RawWakerVTable::new(clone, noop, noop, noop);

    let raw = RawWaker::new(std::ptr::null(), &VTABLE);
    let waker = unsafe { Waker::from_raw(raw) };
    let mut cx = Context::from_waker(&waker);
    let mut fut = Box::pin(fut);
    loop {
        match fut.as_mut().poll(&mut cx) {
            Poll::Ready(v) => return v,
            Poll::Pending => std::hint::spin_loop(),
        }
    }
}

pub fn create_property_based_test<T: Fn(&mut SmallRng)>(niter: usize, reg: &[u64], f: T) {
    let mut seed_rng = rand::rng();
    for i in 0..niter {
        let seed = if let Some(seed) = reg.get(i) {
            *seed
        } else {
            seed_rng.random()
        };

        let mut rng = SmallRng::seed_from_u64(seed);
        println!("\n{i}/{niter}, seed {seed}");
        f(&mut rng);
    }
}
