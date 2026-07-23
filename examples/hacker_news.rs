//! Get the Hacker News front page from the Algolia API.
//!
//! This example shows cursor pagination (the endpoint gets `nbPages` from
//! the first response) and a rate limit with `governor`.
//!
//! Run with: cargo run --example hacker_news --features governor
//! (network access is necessary)

use std::num::NonZeroU32;

use governor::Quota;
use ingester::{Endpoint, Error, RateLimit, Request, Response, ingest};
use serde::Deserialize;

#[derive(Debug, Deserialize)]
struct Story {
    #[serde(rename = "objectID")]
    id: String,
    title: Option<String>,
    url: Option<String>,
}

/// Paginate `https://hn.algolia.com/api/v1/search?tags=front_page&page=N`.
struct FrontPage {
    page: u32,
    /// Learned from the first response; `None` until then.
    nb_pages: Option<u64>,
}

impl FrontPage {
    fn new() -> Self {
        Self {
            page: 0,
            nb_pages: None,
        }
    }
}

impl Endpoint for FrontPage {
    type Item = Story;

    fn next_request(&mut self, last: Option<&Response>) -> Option<Request> {
        if let Some(resp) = last {
            self.nb_pages = resp.json::<serde_json::Value>().ok()?["nbPages"].as_u64();
        }
        if self.nb_pages.is_some_and(|max| u64::from(self.page) >= max) {
            return None;
        }
        let url = format!(
            "https://hn.algolia.com/api/v1/search?tags=front_page&page={}",
            self.page
        );
        self.page += 1;
        Some(Request::get(url.parse().expect("valid URL")))
    }

    fn parse(&self, response: &Response) -> Result<Vec<Story>, Error> {
        let body: serde_json::Value = response.json()?;
        Ok(serde_json::from_value(body["hits"].clone())?)
    }
}

#[tokio::main]
async fn main() -> Result<(), Error> {
    // A maximum of 2 requests each second.
    let rate_limit = RateLimit::from_quota(Quota::per_second(
        NonZeroU32::new(2).expect("dev can't read and constructed a NonZeroU32 with 0"),
    ));

    let stories = ingest(FrontPage::new())
        .with_rate_limit(rate_limit)
        .collect()
        .await?;

    println!("fetched {} stories", stories.len());
    for story in stories.iter().take(10) {
        println!(
            "- [{}] {} ({})",
            story.id,
            story.title.as_deref().unwrap_or("<no title>"),
            story.url.as_deref().unwrap_or("<no url>")
        );
    }
    Ok(())
}
