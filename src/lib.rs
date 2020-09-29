//! # Example
//! ```
//! use actix_cbor::Cbor;
//!
//! struct User {
//!     name: String,
//! }
//! struct Greeting {
//!     inner: String,
//! }
//!
//! #[get("/users/hello")]
//! pub async fn greet_user(user: Cbor<User>) -> Cbor<Greeting> {
//!     let name: &str = &user.name;
//!     let inner: String = format!("Hello {}!", name);
//!     Cbor(Greeting { inner })
//! }
//! ```

#[cfg(test)]
#[macro_use]
extern crate serde;

use std::fmt;
use std::ops::{Deref, DerefMut};

use actix_http::{Payload, PayloadStream, Response};
use actix_http::http::StatusCode;
use actix_web::{FromRequest, HttpRequest, Responder};
#[cfg(feature = "compress")]
use actix_web::dev::Decompress;
use futures_util::core_reexport::fmt::Formatter;
use futures_util::future::{err, LocalBoxFuture, ok, Ready};
use futures_util::FutureExt;
use serde::de::DeserializeOwned;
use serde::Serialize;

pub use body::*;
pub use config::*;
pub use error::*;

mod error;
mod config;
mod body;

#[cfg(test)]
mod tests;

/// Extractor/Responder for CBOR encoded data.
///
/// This will encode data with the content-type `application/cbor`.
///
/// By default, it expects to receive data with that content-type as well.
///
/// # Example
/// ```
/// use actix_cbor::Cbor;
///
/// struct User {
///     name: String,
/// }
/// struct Greeting {
///     inner: String,
/// }
///
/// #[get("/users/hello")]
/// pub async fn greet_user(user: Cbor<User>) -> Cbor<Greeting> {
///     let name: &str = &user.name;
///     let inner: String = format!("Hello {}!", name);
///     Cbor(Greeting { inner })
/// }
/// ```
pub struct Cbor<T>(pub T);

impl<T> Cbor<T> {
    /// Deconstruct to an inner value
    pub fn into_inner(self) -> T {
        self.0
    }
}

impl<T> Deref for Cbor<T> {
    type Target = T;

    fn deref(&self) -> &T {
        &self.0
    }
}

impl<T> DerefMut for Cbor<T> {
    fn deref_mut(&mut self) -> &mut T {
        &mut self.0
    }
}

impl<T> fmt::Debug for Cbor<T> where T: fmt::Debug {
    fn fmt(&self, f: &mut Formatter<'_>) -> fmt::Result {
        write!(f, "Cbor: {:?}", self.0)
    }
}

impl<T> Responder for Cbor<T> where T: Serialize {
    type Error = CborError;
    type Future = Ready<Result<Response, Self::Error>>;

    fn respond_to(self, _: &HttpRequest) -> Self::Future {
        let body = match serde_cbor::to_vec(&self.0) {
            Ok(body) => body,
            Err(e) => return err(e.into())
        };

        ok(Response::build(StatusCode::OK)
            .content_type("application/cbor")
            .body(body))
    }
}

impl<T> FromRequest for Cbor<T> where T: DeserializeOwned + 'static {
    type Error = actix_web::Error;
    type Future = LocalBoxFuture<'static, Result<Self, Self::Error>>;
    type Config = CborConfig;

    fn from_request(req: &HttpRequest, payload: &mut Payload<PayloadStream>) -> Self::Future {
        let req2 = req.clone();
        let config = CborConfig::from_req(req);

        let limit = config.limit;
        let ctype = config.content_type.clone();
        let err_handler = config.err_handler.clone();

        CborBody::new(req, payload, ctype)
            .limit(limit)
            .map(move |res| match res {
                Err(e) => {
                    log::debug!(
                        "Failed to deserialize CBOR from payload. \
                         Request path: {}",
                        req2.path()
                    );

                    if let Some(err) = err_handler {
                        Err((*err)(e, &req2))
                    } else {
                        Err(e.into())
                    }
                }
                Ok(data) => Ok(Cbor(data)),
            })
            .boxed_local()
    }
}