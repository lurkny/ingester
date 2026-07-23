use bytes::Bytes;
use http::{HeaderMap, StatusCode};
use url::Url;

use crate::Error;

/// The response to one [`Request`](crate::Request).
#[derive(Debug, Clone)]
pub struct Response {
    pub status: StatusCode,
    pub headers: HeaderMap,
    /// The URL that supplied the response.
    pub url: Url,
    body: Bytes,
}

impl Response {
    pub fn new(status: StatusCode, headers: HeaderMap, url: Url, body: Bytes) -> Self {
        Self {
            status,
            headers,
            url,
            body,
        }
    }

    pub fn bytes(&self) -> &Bytes {
        &self.body
    }

    /// Get the body as UTF-8 text.
    pub fn text(&self) -> Result<String, Error> {
        Ok(String::from_utf8(self.body.to_vec())?)
    }

    /// Parse the body as JSON.
    pub fn json<T: serde::de::DeserializeOwned>(&self) -> Result<T, Error> {
        Ok(serde_json::from_slice(&self.body)?)
    }
}
