use std::fmt;

/// The errors that can occur during the ingestion of an
/// [`Endpoint`](crate::Endpoint).
#[derive(Debug)]
#[non_exhaustive]
pub enum Error {
    /// The [`HttpClient`](crate::HttpClient) could not execute a request.
    Http(Box<dyn std::error::Error + Send + Sync>),
    /// The construction of a request failed (for example, a header value
    /// that is not valid).
    RequestBuild(String),
    /// [`Endpoint::parse`](crate::Endpoint::parse) failed.
    Parse(Box<dyn std::error::Error + Send + Sync>),
    /// A response body is not valid UTF-8.
    Utf8(std::string::FromUtf8Error),
    /// A response body is not valid JSON.
    Json(serde_json::Error),
}

impl Error {
    /// Make a `Parse` error that contains `err`. This function is for
    /// [`Endpoint::parse`](crate::Endpoint::parse) implementations.
    pub fn parse<E: std::error::Error + Send + Sync + 'static>(err: E) -> Self {
        Self::Parse(Box::new(err))
    }
}

impl fmt::Display for Error {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Http(e) => write!(f, "http error: {e}"),
            Self::RequestBuild(msg) => write!(f, "failed to build request: {msg}"),
            Self::Parse(e) => write!(f, "failed to parse response: {e}"),
            Self::Utf8(e) => write!(f, "response body is not valid UTF-8: {e}"),
            Self::Json(e) => write!(f, "response body is not valid JSON: {e}"),
        }
    }
}

impl std::error::Error for Error {
    fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
        match self {
            Self::Http(e) | Self::Parse(e) => Some(&**e),
            Self::Utf8(e) => Some(e),
            Self::Json(e) => Some(e),
            Self::RequestBuild(_) => None,
        }
    }
}

impl From<std::string::FromUtf8Error> for Error {
    fn from(err: std::string::FromUtf8Error) -> Self {
        Self::Utf8(err)
    }
}

impl From<serde_json::Error> for Error {
    fn from(err: serde_json::Error) -> Self {
        Self::Json(err)
    }
}

#[cfg(feature = "reqwest")]
impl From<reqwest::Error> for Error {
    fn from(err: reqwest::Error) -> Self {
        Self::Http(Box::new(err))
    }
}
