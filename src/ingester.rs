use std::collections::VecDeque;

use futures_util::{Stream, stream::try_unfold};

use crate::client::HttpClient;
use crate::rate_limit::RateLimit;
use crate::{Endpoint, Error, Response, client, rate_limit};

/// Operates an [`Endpoint`]: sends its requests through an [`HttpClient`],
/// obeys a [`RateLimit`], and parses the responses into a stream of items.
///
/// `C` is the [`client::Backend`] and `B` is the [`rate_limit::Backend`].
/// The constructors accept the backends directly and put them in an
/// [`HttpClient`] / a [`RateLimit`] for you.
///
/// The usual start point is [`ingest`](crate::ingest). It uses the default
/// reqwest client and no rate limit. Use [`with_client`](Self::with_client)
/// and [`with_rate_limit`](Self::with_rate_limit) to change the
/// configuration:
///
/// ```no_run
/// # use ingester::ingest;
/// # fn doc(end: impl ingester::Endpoint<Item = String>) {
/// let ingester = ingest(end); // .with_rate_limit(...).with_client(...)
/// # }
/// ```
pub struct Ingester<E, C, B> {
    endpoint: E,
    client: HttpClient<C>,
    rate_limit: RateLimit<B>,
}

impl<E: Endpoint, C: client::Backend, B: rate_limit::Backend> Ingester<E, C, B> {
    /// Make an `Ingester` for `endpoint` with the given client and rate
    /// limit.
    ///
    /// The two parameters accept a backend (a [`client::Backend`] or a
    /// [`rate_limit::Backend`]) or an [`HttpClient`] / a [`RateLimit`] that
    /// contains one.
    pub fn new(
        endpoint: E,
        client: impl Into<HttpClient<C>>,
        rate_limit: impl Into<RateLimit<B>>,
    ) -> Self {
        Self {
            endpoint,
            client: client.into(),
            rate_limit: rate_limit.into(),
        }
    }

    /// Replace the HTTP client with a different [`client::Backend`] (or
    /// [`HttpClient`]).
    pub fn with_client<C2: client::Backend>(
        self,
        client: impl Into<HttpClient<C2>>,
    ) -> Ingester<E, C2, B> {
        Ingester::new(self.endpoint, client, self.rate_limit)
    }

    /// Replace the rate limit with a different [`rate_limit::Backend`] (or
    /// [`RateLimit`]).
    pub fn with_rate_limit<B2: rate_limit::Backend>(
        self,
        rate_limit: impl Into<RateLimit<B2>>,
    ) -> Ingester<E, C, B2> {
        Ingester::new(self.endpoint, self.client, rate_limit)
    }

    /// Operate the endpoint and give each parsed item as a stream.
    ///
    /// If an error occurs, the stream gives one `Err` item and then stops.
    pub fn into_stream(self) -> impl Stream<Item = Result<E::Item, Error>> + Send {
        type State<E, C, B> = (
            E,
            HttpClient<C>,
            RateLimit<B>,
            Option<Response>,
            VecDeque<<E as Endpoint>::Item>,
        );

        let state: State<_, _, _> = (
            self.endpoint,
            self.client,
            self.rate_limit,
            None,
            VecDeque::new(),
        );

        try_unfold(
            state,
            |(mut endpoint, client, rate_limit, mut last, mut buf)| async move {
                loop {
                    if let Some(item) = buf.pop_front() {
                        return Ok(Some((item, (endpoint, client, rate_limit, last, buf))));
                    }

                    let Some(request) = endpoint.next_request(last.as_ref()) else {
                        return Ok(None);
                    };
                    rate_limit.wait().await;
                    let response = client.execute(request).await?;
                    buf = endpoint.parse(&response)?.into_iter().collect();
                    last = Some(response);
                }
            },
        )
    }

    /// Operate the endpoint until it stops, and collect all the items.
    pub async fn collect(self) -> Result<Vec<E::Item>, Error> {
        use futures_util::TryStreamExt;
        self.into_stream().try_collect().await
    }
}
