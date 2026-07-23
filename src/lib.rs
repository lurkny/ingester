//! Web scraping and data ingestion.
//!
//! Implement [`Endpoint`] for a type that describes your data source. The
//! endpoint makes the next request and parses each response into items. The
//! endpoint sees the previous response, thus cursor pagination is possible.
//! Give the endpoint to an [`Ingester`] with an [`HttpClient`] and a
//! [`RateLimit`]. Then read the items from the stream that the ingester
//! makes.
//!
//! ```no_run
//! use ingester::{Endpoint, Error, Request, Response, ingest};
//!
//! struct StoryIds {
//!     page: u32,
//!     max_pages: Option<u64>,
//! }
//!
//! impl Endpoint for StoryIds {
//!     type Item = u64;
//!
//!     fn next_request(&mut self, last: Option<&Response>) -> Option<Request> {
//!         if let Some(resp) = last {
//!             self.max_pages = resp.json::<serde_json::Value>().ok()?["nbPages"].as_u64();
//!         }
//!         if self.max_pages.is_some_and(|max| u64::from(self.page) >= max) {
//!             return None;
//!         }
//!         let url = format!(
//!             "https://hn.algolia.com/api/v1/search?tags=front_page&page={}",
//!             self.page
//!         );
//!         self.page += 1;
//!         Some(Request::get(url.parse().unwrap()))
//!     }
//!
//!     fn parse(&self, response: &Response) -> Result<Vec<u64>, Error> {
//!         let body: serde_json::Value = response.json()?;
//!         Ok(body["hits"]
//!             .as_array()
//!             .into_iter()
//!             .flatten()
//!             .filter_map(|hit| hit["objectID"].as_str()?.parse().ok())
//!             .collect())
//!     }
//! }
//!
//! # async fn example() -> Result<(), ingester::Error> {
//! let stories = ingest(StoryIds { page: 0, max_pages: None })
//!     .collect()
//!     .await?;
//! # Ok(())
//! # }
//! ```

pub mod client;
mod endpoint;
mod error;
mod ingester;
pub mod rate_limit;
mod request;
mod response;

pub use client::HttpClient;
pub use endpoint::Endpoint;
pub use error::Error;
pub use ingester::Ingester;
pub use rate_limit::{NoRateLimit, RateLimit};
pub use request::Request;
pub use response::Response;

#[cfg(feature = "reqwest")]
pub use client::Reqwest;

#[cfg(feature = "governor")]
pub use rate_limit::Governor;

/// Start ingestion of an endpoint with the default configuration: an
/// [`HttpClient`] that uses [`Reqwest`], and no rate limit. Use
/// [`Ingester::with_client`] and [`Ingester::with_rate_limit`] to change the
/// configuration.
#[cfg(feature = "reqwest")]
pub fn ingest<E: Endpoint>(endpoint: E) -> Ingester<E, Reqwest, NoRateLimit> {
    Ingester::new(endpoint, Reqwest::new(), NoRateLimit)
}
