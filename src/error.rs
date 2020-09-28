use std::error::Error;
use std::fmt;

use actix_http::error::PayloadError;
use actix_http::http::StatusCode;
use actix_http::ResponseError;
use actix_web::HttpResponse;
use futures_util::core_reexport::fmt::{Display, Formatter};

#[derive(Debug)]
pub struct CborError(serde_cbor::Error);

#[derive(Debug)]
pub enum CborPayloadError {
    /// Payload size is bigger than allowed. (default: 32kB)
    Overflow,
    /// Content type error
    ContentType,
    /// Deserialize error
    Deserialize(CborError),
    /// Payload error
    Payload(PayloadError),
}

impl From<CborError> for CborPayloadError {
    fn from(e: CborError) -> Self {
        Self::Deserialize(e)
    }
}

impl From<serde_cbor::Error> for CborPayloadError {
    fn from(e: serde_cbor::Error) -> Self {
        Self::Deserialize(e.into())
    }
}

impl From<PayloadError> for CborPayloadError {
    fn from(e: PayloadError) -> Self {
        Self::Payload(e)
    }
}

impl Display for CborPayloadError {
    fn fmt(&self, f: &mut Formatter<'_>) -> fmt::Result {
        match self {
            CborPayloadError::Overflow => writeln!(f, "Cbor payload size is bigger than allowed"),
            CborPayloadError::ContentType => writeln!(f, "Content type error"),
            CborPayloadError::Deserialize(inner) => {
                writeln!(f, "CBOR deserialize error: {}", inner)
            }
            CborPayloadError::Payload(inner) => {
                writeln!(f, "Error that occur during reading payload: {:?}", inner)
            }
        }
    }
}

impl Error for CborPayloadError {}

/// Return `BadRequest` for `CborPayloadError`
impl ResponseError for CborPayloadError {
    fn error_response(&self) -> HttpResponse {
        match *self {
            CborPayloadError::Overflow => {
                HttpResponse::new(StatusCode::PAYLOAD_TOO_LARGE)
            }
            _ => HttpResponse::new(StatusCode::BAD_REQUEST),
        }
    }
}

impl fmt::Display for CborError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        self.0.fmt(f)
    }
}

impl Error for CborError {}

impl ResponseError for CborError {
    fn status_code(&self) -> StatusCode {
        StatusCode::INTERNAL_SERVER_ERROR
    }
}

impl From<serde_cbor::Error> for CborError {
    fn from(e: serde_cbor::Error) -> Self {
        Self(e)
    }
}