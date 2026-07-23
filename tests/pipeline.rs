//! Offline integration tests: a mock HTTP client serves canned responses, a
//! counting rate limiter records how often it was awaited.

use std::collections::VecDeque;
use std::sync::atomic::{AtomicUsize, Ordering};
use std::sync::{Arc, Mutex};

use bytes::Bytes;
use futures_util::StreamExt;
use http::{HeaderMap, StatusCode};
use ingester::{Endpoint, Error, Ingester, NoRateLimit, Request, Response, client, rate_limit};
use url::Url;


#[derive(Clone, Default)]
struct MockClient {
    responses: Arc<Mutex<VecDeque<Response>>>,
    requested: Arc<Mutex<Vec<Url>>>,
}

impl MockClient {
    fn new(responses: Vec<Response>) -> Self {
        Self {
            responses: Arc::new(Mutex::new(responses.into())),
            requested: Arc::new(Mutex::new(Vec::new())),
        }
    }

    fn requested_urls(&self) -> Vec<Url> {
        self.requested.lock().unwrap().clone()
    }
}

impl client::Backend for MockClient {
    async fn execute(&self, req: Request) -> Result<Response, Error> {
        self.requested.lock().unwrap().push(req.url.clone());
        self.responses
            .lock()
            .unwrap()
            .pop_front()
            .ok_or_else(|| Error::RequestBuild("mock client ran out of canned responses".into()))
    }
}

#[derive(Clone, Default)]
struct CountingRateLimit(Arc<AtomicUsize>);

impl CountingRateLimit {
    fn count(&self) -> usize {
        self.0.load(Ordering::SeqCst)
    }
}

impl rate_limit::Backend for CountingRateLimit {
    async fn wait(&self) {
        self.0.fetch_add(1, Ordering::SeqCst);
    }
}


/// Serves pages of `{"items": [...], "next": "<cursor>" | null}`; the next
/// request's URL embeds the cursor from the previous response.
struct CursorEndpoint {
    base: Url,
}

impl CursorEndpoint {
    fn new() -> Self {
        Self {
            base: "http://example.com/items".parse().unwrap(),
        }
    }
}

impl Endpoint for CursorEndpoint {
    type Item = u64;

    fn next_request(&mut self, last: Option<&Response>) -> Option<Request> {
        let mut url = self.base.clone();
        match last {
            None => {}
            Some(resp) => {
                let body: serde_json::Value = resp.json().ok()?;
                let next = body["next"].as_str()?;
                url.query_pairs_mut().append_pair("cursor", next);
            }
        }
        Some(Request::get(url))
    }

    fn parse(&self, response: &Response) -> Result<Vec<u64>, Error> {
        let body: serde_json::Value = response.json()?;
        Ok(body["items"]
            .as_array()
            .into_iter()
            .flatten()
            .filter_map(|v| v.as_u64())
            .collect())
    }
}


#[tokio::test]
async fn cursor_pagination_flows_through_responses() {
    let client = MockClient::new(vec![
        Response::new(
            StatusCode::OK,
            HeaderMap::new(),
            "http://example.com/items".parse().unwrap(),
            Bytes::from(r#"{"items": [1, 2], "next": "tok-a"}"#),
        ),
        Response::new(
            StatusCode::OK,
            HeaderMap::new(),
            "http://example.com/items?cursor=tok-a".parse().unwrap(),
            Bytes::from(r#"{"items": [3], "next": null}"#),
        ),
    ]);
    let rate_limit = CountingRateLimit::default();

    let items = Ingester::new(CursorEndpoint::new(), client.clone(), rate_limit.clone())
        .collect()
        .await
        .unwrap();

    assert_eq!(items, vec![1, 2, 3]);
    assert_eq!(
        client.requested_urls(),
        vec![
            Url::parse("http://example.com/items").unwrap(),
            Url::parse("http://example.com/items?cursor=tok-a").unwrap(),
        ],
        "second request carries the cursor from the first response"
    );
    assert_eq!(rate_limit.count(), 2, "one wait per request");
}

#[tokio::test]
async fn endpoint_returning_none_immediately_yields_empty_stream() {
    struct Empty;

    impl Endpoint for Empty {
        type Item = u64;
        fn next_request(&mut self, _last: Option<&Response>) -> Option<Request> {
            None
        }
        fn parse(&self, _response: &Response) -> Result<Vec<u64>, Error> {
            unreachable!()
        }
    }

    let client = MockClient::new(vec![]);
    let items = Ingester::new(Empty, client.clone(), NoRateLimit)
        .collect()
        .await
        .unwrap();

    assert!(items.is_empty());
    assert!(client.requested_urls().is_empty(), "no requests were made");
}

#[tokio::test]
async fn parse_error_yields_one_err_then_ends_stream() {
    struct AlwaysFails;

    impl Endpoint for AlwaysFails {
        type Item = u64;
        fn next_request(&mut self, last: Option<&Response>) -> Option<Request> {
            // Keep making requests; parse will fail on the first response.
            if last.is_none() {
                Some(Request::get("http://example.com/items".parse().unwrap()))
            } else {
                None
            }
        }
        fn parse(&self, _response: &Response) -> Result<Vec<u64>, Error> {
            Err(Error::parse(std::io::Error::other("boom")))
        }
    }

    let client = MockClient::new(vec![Response::new(
        StatusCode::OK,
        HeaderMap::new(),
        "http://example.com/items".parse().unwrap(),
        Bytes::from(r#"{"items": [1]}"#),
    )]);

    let results: Vec<_> = Ingester::new(AlwaysFails, client, NoRateLimit)
        .into_stream()
        .collect()
        .await;

    assert_eq!(results.len(), 1, "one Err item, then the stream ends");
    assert!(results[0].is_err());
}

#[tokio::test]
async fn rate_limit_is_awaited_before_every_request() {
    let client = MockClient::new(vec![
        Response::new(
            StatusCode::OK,
            HeaderMap::new(),
            "http://example.com/items".parse().unwrap(),
            Bytes::from(r#"{"items": [1], "next": "a"}"#),
        ),
        Response::new(
            StatusCode::OK,
            HeaderMap::new(),
            "http://example.com/items?cursor=a".parse().unwrap(),
            Bytes::from(r#"{"items": [2], "next": "b"}"#),
        ),
        Response::new(
            StatusCode::OK,
            HeaderMap::new(),
            "http://example.com/items?cursor=b".parse().unwrap(),
            Bytes::from(r#"{"items": [3], "next": null}"#),
        ),
    ]);
    let rate_limit = CountingRateLimit::default();

    let items = Ingester::new(CursorEndpoint::new(), client.clone(), rate_limit.clone())
        .collect()
        .await
        .unwrap();

    assert_eq!(items, vec![1, 2, 3]);
    assert_eq!(rate_limit.count(), 3);
}

#[test]
fn response_text_and_json_helpers() {
    let resp = Response::new(
        StatusCode::OK,
        HeaderMap::new(),
        "http://example.com".parse().unwrap(),
        Bytes::from(r#"{"a": 1}"#),
    );
    assert_eq!(resp.text().unwrap(), r#"{"a": 1}"#);
    let value: serde_json::Value = resp.json().unwrap();
    assert_eq!(value["a"], 1);
}
