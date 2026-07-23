use bytes::Bytes;
use http::{HeaderMap, Method, header::InvalidHeaderValue};
use url::Url;

/// One HTTP request that an [`Endpoint`](crate::Endpoint) makes.
#[derive(Debug, Clone)]
pub struct Request {
    pub method: Method,
    pub url: Url,
    pub headers: HeaderMap,
    pub body: Option<Bytes>,
}

#[cfg(feature = "reqwest")]
impl From<reqwest::Request> for Request {
    fn from(r: reqwest::Request) -> Self {
        Self {
            method: r.method().clone(),
            url: r.url().clone(),
            headers: r.headers().clone(),
            body: r
                .body()
                .and_then(|b| b.as_bytes().map(Bytes::copy_from_slice)),
        }
    }
}

impl Request {
    /// Make a `GET` request to `url`.
    pub fn get(url: Url) -> Self {
        Self::new(Method::GET, url)
    }

    /// Make a `POST` request to `url`.
    pub fn post(url: Url) -> Self {
        Self::new(Method::POST, url)
    }

    pub fn new(method: Method, url: Url) -> Self {
        Self {
            method,
            url,
            headers: HeaderMap::new(),
            body: None,
        }
    }

    /// Set a header. This function panics if `value` is not a valid header
    /// value.
    pub fn header(mut self, name: impl http::header::IntoHeaderName, value: &str) -> Self {
        let value = value
            .parse()
            .expect("header value contains invalid characters");
        self.headers.insert(name, value);
        self
    }

    /// Set a header. If `value` is not valid, this function returns an
    /// error and does not panic.
    pub fn try_header(
        mut self,
        name: impl http::header::IntoHeaderName,
        value: &str,
    ) -> Result<Self, InvalidHeaderValue> {
        let value = value.parse()?;
        self.headers.insert(name, value);
        Ok(self)
    }

    pub fn body(mut self, body: impl Into<Bytes>) -> Self {
        self.body = Some(body.into());
        self
    }
}
