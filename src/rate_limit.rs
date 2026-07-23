use std::future::Future;

/// The backend of a [`RateLimit`]: an async gate. The
/// [`Ingester`](crate::Ingester) awaits the gate before each request.
///
/// Implement this trait to use a different rate limiter. With the `governor`
/// feature, each direct [`governor`](https://docs.rs/governor) rate limiter
/// is already a `Backend` — see `Governor`.
pub trait Backend: Send + Sync {
    /// Wait until the next request can continue.
    fn wait(&self) -> impl Future<Output = ()> + Send;
}

/// The rate limit of an [`Ingester`](crate::Ingester). The type parameter is
/// its [`Backend`].
///
/// `RateLimit<NoRateLimit>` (the default, also written plain `RateLimit`)
/// applies no limit. With the `governor` feature, `RateLimit<Governor>`
/// limits requests with a [`governor`](https://docs.rs/governor) rate
/// limiter.
///
/// ```
/// use ingester::{NoRateLimit, RateLimit};
///
/// // No rate limit: each request continues immediately.
/// let none = RateLimit::new(NoRateLimit);
/// ```
#[derive(Debug, Clone, Default)]
pub struct RateLimit<B = NoRateLimit> {
    backend: B,
}

impl<B: Backend> RateLimit<B> {
    /// Make a `RateLimit` that contains `backend`.
    pub fn new(backend: B) -> Self {
        Self { backend }
    }

    /// Wait until the next request can continue.
    pub async fn wait(&self) {
        self.backend.wait().await;
    }
}

/// Makes a `RateLimit` from each [`Backend`]. Thus, the
/// [`Ingester`](crate::Ingester) constructors accept backends directly.
impl<B: Backend> From<B> for RateLimit<B> {
    fn from(backend: B) -> Self {
        Self::new(backend)
    }
}

/// A [`Backend`] with no rate limit: each request continues immediately.
#[derive(Debug, Default, Clone, Copy)]
pub struct NoRateLimit;

impl Backend for NoRateLimit {
    async fn wait(&self) {}
}

/// The default direct rate limiter of the
/// [`governor`](https://docs.rs/governor) crate, re-exported for use as a
/// [`RateLimit`] backend: `RateLimit<Governor>`.
///
/// Make one with [`governor::RateLimiter::direct`] and give it to
/// [`RateLimit::new`], or use [`RateLimit::from_quota`]. To share one
/// limiter with other parts of your application, clone it. The clones share
/// the same state.
#[cfg(feature = "governor")]
pub use governor::DefaultDirectRateLimiter as Governor;

#[cfg(feature = "governor")]
mod governor_impl {
    use governor::middleware::RateLimitingMiddleware;
    use governor::state::{DirectStateStore, NotKeyed};
    use governor::{NotUntil, Quota, RateLimiter, clock};

    use super::{Backend, Governor, RateLimit};

    // Each direct governor limiter is a valid backend.
    impl<S, C, MW> Backend for RateLimiter<NotKeyed, S, C, MW>
    where
        S: DirectStateStore + Send + Sync,
        C: clock::ReasonablyRealtime + Send + Sync,
        C::Instant: Send,
        MW: RateLimitingMiddleware<C::Instant, NegativeOutcome = NotUntil<C::Instant>>
            + Send
            + Sync,
        MW::PositiveOutcome: Send,
    {
        async fn wait(&self) {
            self.until_ready().await;
        }
    }

    impl RateLimit<Governor> {
        /// Limit the requests to the given [`Quota`].
        ///
        /// ```no_run
        /// use std::num::NonZeroU32;
        /// use governor::Quota;
        /// use ingester::RateLimit;
        ///
        /// // A maximum of 5 requests each second.
        /// let limit = RateLimit::from_quota(Quota::per_second(NonZeroU32::new(5).unwrap()));
        /// ```
        pub fn from_quota(quota: Quota) -> Self {
            RateLimit::new(Governor::direct(quota))
        }
    }
}
