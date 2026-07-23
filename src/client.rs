use std::future::Future;

use crate::{Error, Request, Response};

/// The backend of an [`HttpClient`]: it executes each [`Request`] and
/// returns a [`Response`].
///
/// Implement this trait to use a different HTTP library, or a mock client in
/// tests. With the `reqwest` feature (on by default), a [`reqwest`] client
/// is already a `Backend` — see `Reqwest`.
pub trait Backend: Send + Sync {
    /// Execute a [`Request`] and return its [`Response`].
    fn execute(&self, req: Request) -> impl Future<Output = Result<Response, Error>> + Send;
}

/// The HTTP client of an [`Ingester`](crate::Ingester). The type parameter
/// is its [`Backend`].
///
/// With the `reqwest` feature (on by default), `HttpClient<Reqwest>`
/// executes requests through [`reqwest`](https://docs.rs/reqwest).
///
/// ```
/// use ingester::{Error, HttpClient, Request, Response};
///
/// // Each `ingester::client::Backend` can be the backend of an `HttpClient`:
/// struct Stub;
///
/// impl ingester::client::Backend for Stub {
///     async fn execute(&self, _req: Request) -> Result<Response, Error> {
///         unimplemented!()
///     }
/// }
///
/// let client = HttpClient::new(Stub);
/// ```
#[derive(Debug, Clone, Default)]
pub struct HttpClient<B> {
    backend: B,
}

impl<B: Backend> HttpClient<B> {
    /// Make an `HttpClient` that contains `backend`.
    pub fn new(backend: B) -> Self {
        Self { backend }
    }

    /// Execute a [`Request`] and return its [`Response`].
    pub async fn execute(&self, req: Request) -> Result<Response, Error> {
        self.backend.execute(req).await
    }
}

/// Makes an `HttpClient` from each [`Backend`]. Thus, the
/// [`Ingester`](crate::Ingester) constructors accept backends directly.
impl<B: Backend> From<B> for HttpClient<B> {
    fn from(backend: B) -> Self {
        Self::new(backend)
    }
}

/// The [`reqwest`](https://docs.rs/reqwest) client, re-exported for use as an
/// [`HttpClient`] backend: `HttpClient<Reqwest>`.
///
/// To share one client with other parts of your application, clone it. The
/// clones share the connection pool.
///
/// ```
/// use ingester::{HttpClient, Reqwest};
///
/// // Default settings:
/// let client = HttpClient::new(Reqwest::new());
///
/// // Or with full configuration through the reqwest builder:
/// let custom = HttpClient::new(
///     Reqwest::builder()
///         .user_agent("my-scraper")
///         .build()
///         .unwrap(),
/// );
/// ```
#[cfg(feature = "reqwest")]
pub use reqwest::Client as Reqwest;

#[cfg(feature = "reqwest")]
impl Backend for Reqwest {
    async fn execute(&self, req: Request) -> Result<Response, Error> {
        let mut builder = self.request(req.method, req.url).headers(req.headers);
        if let Some(body) = req.body {
            builder = builder.body(body);
        }
        let resp = builder.send().await?;
        let status = resp.status();
        let headers = resp.headers().clone();
        let url = resp.url().clone();
        let body = resp.bytes().await?;
        Ok(Response::new(status, headers, url, body))
    }
}
