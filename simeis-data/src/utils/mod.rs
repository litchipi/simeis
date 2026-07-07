mod shardeddata;
use std::{
    future::Future,
    pin::Pin,
    task::{Context, Poll},
    time::{Duration, Instant},
};

pub type BoxFuture<'a, T> = std::pin::Pin<Box<dyn Future<Output = T> + Send + 'a>>;

pub use shardeddata::ShardedLockedData;

pub struct AsyncSleepFuture {
    start: Instant,
    dur: Duration,
}

impl std::future::Future for AsyncSleepFuture {
    type Output = ();

    fn poll(self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<Self::Output> {
        if self.start.elapsed() >= self.dur {
            Poll::Ready(())
        } else {
            cx.waker().wake_by_ref();
            Poll::Pending
        }
    }
}

pub fn sleep(dur: Duration) -> AsyncSleepFuture {
    AsyncSleepFuture {
        dur,
        start: std::time::Instant::now(),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::tests::block_on;

    #[test]
    fn test_sleep_waits_at_least_the_duration() {
        let dur = Duration::from_millis(5);
        let start = Instant::now();
        block_on(sleep(dur));
        assert!(start.elapsed() >= dur);
    }
}
