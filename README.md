# ingester

[![crates.io](https://img.shields.io/crates/v/ingester.svg)](https://crates.io/crates/ingester)
[![docs.rs](https://img.shields.io/docsrs/ingester)](https://docs.rs/ingester)
[![license](https://img.shields.io/crates/l/ingester.svg)](#license)

The `ingester` crate reads paginated APIs and makes an async stream of parsed
items.

You tell the crate two things:

- The request that it must send next.
- The procedure that parses each response into items.

The crate then does the work. It sends each request through an HTTP client
that you can replace. It obeys a rate limit that you can configure. It gives
the parsed items to you as a [`futures::Stream`].

Your endpoint sees the last response before it makes the next request. Thus
pagination with cursors, with tokens, or with a page count in the response
body is possible.

[`futures::Stream`]: https://docs.rs/futures/latest/futures/stream/trait.Stream.html

## Example

This example gets all the front-page story IDs from the Hacker News Algolia
API. The endpoint gets the page count from the first response.

```rust
use ingester::{Endpoint, Error, Request, Response, ingest};

struct StoryIds {
    page: u32,
    max_pages: Option<u64>,
}

impl Endpoint for StoryIds {
    type Item = u64;

    fn next_request(&mut self, last: Option<&Response>) -> Option<Request> {
        if let Some(resp) = last {
            self.max_pages = resp.json::<serde_json::Value>().ok()?["nbPages"].as_u64();
        }
        if self.max_pages.is_some_and(|max| u64::from(self.page) >= max) {
            return None;
        }
        let url = format!(
            "https://hn.algolia.com/api/v1/search?tags=front_page&page={}",
            self.page
        );
        self.page += 1;
        Some(Request::get(url.parse().unwrap()))
    }

    fn parse(&self, response: &Response) -> Result<Vec<u64>, Error> {
        let body: serde_json::Value = response.json()?;
        Ok(body["hits"]
            .as_array()
            .into_iter()
            .flatten()
            .filter_map(|hit| hit["objectID"].as_str()?.parse().ok())
            .collect())
    }
}

async fn example() -> Result<(), Error> {
    let stories = ingest(StoryIds { page: 0, max_pages: None })
        .collect()
        .await?;
    Ok(())
}
```

The `collect()` method collects all the items into a `Vec`. As an
alternative, use `into_stream()` to get each item when it arrives. If an
error occurs, the stream gives one `Err` item and then stops.

## Operation

Implement the [`Endpoint`] trait. The trait has two methods:

- `next_request(&mut self, last: Option<&Response>) -> Option<Request>` —
  make the next request. The `last` parameter contains the previous
  response. On the first call, `last` is `None`. For cursor pagination, read
  the cursor from the last response body. For a page counter, keep the
  counter in `self`. Return `None` to stop.
- `parse(&self, response: &Response) -> Result<Vec<Item>, Error>` — parse a
  response into zero or more items.

The `ingest(endpoint)` function makes an `Ingester` with the default
configuration. The default configuration is a [`reqwest`] HTTP client and no
rate limit. Use `with_client` and `with_rate_limit` to change the
configuration:

```rust,ignore
let ingester = ingest(endpoint)
    .with_rate_limit(RateLimit::from_quota(quota)) // governor feature
    .with_client(Reqwest::builder().user_agent("my-scraper").build()?);
```

[`Endpoint`]: https://docs.rs/ingester/latest/ingester/trait.Endpoint.html
[`reqwest`]: https://docs.rs/reqwest

## Rate limits

Enable the `governor` feature to use a [`governor`] rate limiter. This
example permits a maximum of 2 requests each second:

```rust,ignore
use std::num::NonZeroU32;
use governor::Quota;
use ingester::RateLimit;

let stories = ingest(StoryIds { page: 0, max_pages: None })
    .with_rate_limit(RateLimit::from_quota(Quota::per_second(
        NonZeroU32::new(2).unwrap(),
    )))
    .collect()
    .await?;
```

[`governor`]: https://docs.rs/governor

## Custom backends

The HTTP client and the rate limiter are traits. You can supply your own
implementations. Some examples are:

- A different HTTP library.
- A rate limiter that your full application shares.
- A mock client for tests.

Implement `ingester::client::Backend` for an HTTP client. Implement
`ingester::rate_limit::Backend` for a rate limiter. Give the backend
directly to `Ingester::new`, `with_client`, or `with_rate_limit`. The crate
wraps the backend for you.

```rust,ignore
struct MockClient { /* prepared responses */ }

impl ingester::client::Backend for MockClient {
    async fn execute(&self, req: Request) -> Result<Response, Error> {
        // return the next prepared response
    }
}

let items = Ingester::new(endpoint, MockClient::new(responses), NoRateLimit)
    .collect()
    .await?;
```

## Feature flags

| Feature    | Default | Effect                                                           |
| ---------- | ------- | ---------------------------------------------------------------- |
| `reqwest`  | yes     | The default HTTP client backend and the `ingest()` function      |
| `governor` | no      | [`governor`] rate limiters as `RateLimit` backends, `from_quota` |

If you set `default-features = false`, the crate contains no HTTP client.
You must then supply your own `client::Backend`.

## License

- MIT license ([LICENSE](LICENSE-MIT))
