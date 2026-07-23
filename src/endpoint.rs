use crate::{Error, Request, Response};

/// Your description of the procedure that ingests one endpoint.
///
/// An `Endpoint` supplies **the next request** and **the parse procedure for
/// each response**. The [`Ingester`](crate::Ingester) calls it in a loop:
///
/// 1. `next_request(last)` — return the next [`Request`], or `None` to stop.
///    The `last` parameter contains the response to the previous request. On
///    the first call, `last` is `None`. For cursor pagination, read the
///    cursor from the previous body and put it in the next URL. For a page
///    counter, keep the counter in `self` and ignore `last`.
/// 2. `parse(&response)` — parse the response into zero or more items.
///
/// ```no_run
/// use ingester::{Endpoint, Error, Request, Response};
/// use url::Url;
///
/// struct Pages {
///     base: Url,
///     page: u32,
///     last_page: Option<u32>,
/// }
///
/// impl Endpoint for Pages {
///     type Item = serde_json::Value;
///
///     fn next_request(&mut self, last: Option<&Response>) -> Option<Request> {
///         if let Some(resp) = last {
///             // Get the total page count from the first response.
///             let total = resp.json::<serde_json::Value>().ok()?["pages"].as_u64()? as u32;
///             self.last_page = Some(total);
///         }
///         if self.last_page.is_some_and(|last| self.page > last) {
///             return None;
///         }
///         let mut url = self.base.clone();
///         url.query_pairs_mut().append_pair("page", &self.page.to_string());
///         self.page += 1;
///         Some(Request::get(url))
///     }
///
///     fn parse(&self, response: &Response) -> Result<Vec<Self::Item>, Error> {
///         Ok(response.json::<serde_json::Value>()?["items"]
///             .as_array()
///             .cloned()
///             .unwrap_or_default())
///     }
/// }
/// ```
pub trait Endpoint: Send {
    /// The type of the items that this endpoint makes.
    type Item: Send;

    /// Return the next request, or `None` to stop the ingestion.
    ///
    /// The `last` parameter contains the response to the previous request.
    /// On the first call, `last` is `None`. The response is a reference: the
    /// ingester subsequently gives it to [`parse`](Self::parse) and then
    /// releases it.
    fn next_request(&mut self, last: Option<&Response>) -> Option<Request>;

    /// Parse a response into zero or more items.
    fn parse(&self, response: &Response) -> Result<Vec<Self::Item>, Error>;
}
