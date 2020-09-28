use std::future::Future;
use std::pin::Pin;
use std::sync::Arc;
use std::task::{Context, Poll};

use actix_http::{HttpMessage, Payload};
use actix_http::http::header::CONTENT_LENGTH;
#[cfg(feature = "compress")]
use actix_web::dev::Decompress;
use actix_web::HttpRequest;
use actix_web::web::BytesMut;
use futures_util::future::{FutureExt, LocalBoxFuture};
use futures_util::StreamExt;
use serde::de::DeserializeOwned;

use crate::CborPayloadError;

/// Request's payload json parser, it resolves to a deserialized `T` value.
/// This future could be used with `ServiceRequest` and `ServiceFromRequest`.
///
/// Returns error:
///
/// * content type is not `application/json`
///   (unless specified in [`JsonConfig`](struct.JsonConfig.html))
/// * content length is greater than 256k
pub struct CborBody<U> {
    pub(crate) limit: usize,
    pub(crate) length: Option<usize>,
    #[cfg(feature = "compress")]
    pub(crate) stream: Option<Decompress<Payload>>,
    #[cfg(not(feature = "compress"))]
    pub(crate) stream: Option<Payload>,
    pub(crate) err: Option<CborPayloadError>,
    pub(crate) fut: Option<LocalBoxFuture<'static, Result<U, CborPayloadError>>>,
}

impl<U> CborBody<U>
    where
        U: DeserializeOwned + 'static,
{
    /// Create `JsonBody` for request.
    #[allow(clippy::borrow_interior_mutable_const)]
    pub fn new(
        req: &HttpRequest,
        payload: &mut Payload,
        ctype: Option<Arc<dyn Fn(&str) -> bool + Send + Sync>>,
    ) -> Self {
        // check content-type
        let mime = req.content_type();
        let is_good_mime =
            mime == "application/cbor"
                || mime == "cbor"
                || ctype.as_ref().map_or(false, |predicate| predicate(mime));

        if !is_good_mime {
            return CborBody {
                limit: 262_144,
                length: None,
                stream: None,
                fut: None,
                err: Some(CborPayloadError::ContentType),
            };
        }

        let len = req
            .headers()
            .get(&CONTENT_LENGTH)
            .and_then(|l| l.to_str().ok())
            .and_then(|s| s.parse::<usize>().ok());

        #[cfg(feature = "compress")]
            let payload = Decompress::from_headers(payload.take(), req.headers());
        #[cfg(not(feature = "compress"))]
            let payload = payload.take();

        CborBody {
            limit: 262_144,
            length: len,
            stream: Some(payload),
            fut: None,
            err: None,
        }
    }

    /// Change max size of payload. By default max size is 256Kb
    pub fn limit(mut self, limit: usize) -> Self {
        self.limit = limit;
        self
    }
}

impl<U> Future for CborBody<U>
    where
        U: DeserializeOwned + 'static,
{
    type Output = Result<U, CborPayloadError>;

    fn poll(mut self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<Self::Output> {
        if let Some(ref mut fut) = self.fut {
            return Pin::new(fut).poll(cx);
        }

        if let Some(err) = self.err.take() {
            return Poll::Ready(Err(err));
        }

        let limit = self.limit;
        if let Some(len) = self.length.take() {
            if len > limit {
                return Poll::Ready(Err(CborPayloadError::Overflow));
            }
        }
        let mut stream = self.stream.take().unwrap();

        self.fut = Some(
            async move {
                let mut body = BytesMut::with_capacity(8192);

                while let Some(item) = stream.next().await {
                    let chunk = item?;
                    if (body.len() + chunk.len()) > limit {
                        return Err(CborPayloadError::Overflow);
                    } else {
                        body.extend_from_slice(&chunk);
                    }
                }
                Ok(serde_cbor::from_slice::<U>(&body)?)
            }
                .boxed_local(),
        );

        self.poll(cx)
    }
}