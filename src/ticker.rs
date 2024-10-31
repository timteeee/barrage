use std::{
    future::poll_fn,
    pin::Pin,
    task::{Context, Poll},
};

use futures::{ready, Future, Stream};
use tokio::time::{Duration, Instant, Sleep};

pub struct JitterInterval {
    sleep: Pin<Box<Sleep>>,
    base_duration: Duration,
    factor: f64,
}

impl JitterInterval {
    pub fn new(base_duration: Duration, factor: f64) -> Self {
        let duration = jitter(base_duration, factor);
        let sleep = Box::pin(tokio::time::sleep(duration));

        Self {
            sleep,
            base_duration,
            factor,
        }
    }

    pub async fn tick(&mut self) -> Instant {
        let instant = poll_fn(|cx| self.poll_tick(cx));

        instant.await
    }

    pub(crate) fn poll_tick(&mut self, cx: &mut Context<'_>) -> Poll<Instant> {
        ready!(Pin::new(&mut self.sleep).poll(cx));

        let next_duration = jitter(self.base_duration, self.factor);
        let now = Instant::now();

        self.sleep.as_mut().reset(now + next_duration);

        Poll::Ready(now)
    }
}

impl Stream for JitterInterval {
    type Item = Instant;

    fn poll_next(mut self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<Option<Self::Item>> {
        self.as_mut().poll_tick(cx).map(Some)
    }

    fn size_hint(&self) -> (usize, Option<usize>) {
        (usize::MAX, None)
    }
}

fn jitter(duration: Duration, factor: f64) -> Duration {
    duration.mul_f64(rand::random::<f64>() + factor)
}
