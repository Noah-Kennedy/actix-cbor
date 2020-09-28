use super::*;
use actix_web::error::InternalError;
use actix_web::http::header::{self, HeaderValue, CONTENT_LENGTH, CONTENT_TYPE};
use actix_web::test::{load_stream, TestRequest};
use actix_web::{HttpResponse, web};
use actix_http::body::Body;

#[derive(Serialize, Deserialize, PartialEq, Debug, Clone)]
struct MyObject {
    name: String,
    number: i32,
}

impl Default for MyObject {
    fn default() -> Self {
        Self {
            name: "test".to_owned(),
            number: 7
        }
    }
}

fn get_test_bytes() -> Vec<u8> {
    serde_cbor::to_vec(&MyObject::default()).unwrap()
}

fn cbor_eq(err: CborPayloadError, other: CborPayloadError) -> bool {
    match err {
        CborPayloadError::Overflow => matches!(other, CborPayloadError::Overflow),
        CborPayloadError::ContentType => {
            matches!(other, CborPayloadError::ContentType)
        }
        _ => false,
    }
}

#[actix_rt::test]
async fn test_responder() {
    let req = TestRequest::default().to_http_request();
    
    let obj = MyObject::default();
    
    let encoded = get_test_bytes();

    let j = Cbor(obj.clone());
    let resp = j.respond_to(&req).await.unwrap();
    assert_eq!(resp.status(), StatusCode::OK);
    assert_eq!(
        resp.headers().get(header::CONTENT_TYPE).unwrap(),
        header::HeaderValue::from_static("application/cbor")
    );

    let body = resp.body();
    let payload= body.as_ref().unwrap();

    if let Body::Bytes(b) = payload {
        assert_eq!(&encoded, b);

        let decoded: MyObject = serde_cbor::from_slice(&b).unwrap();
        assert_eq!(obj, decoded);
    }
}

#[actix_rt::test]
async fn test_custom_error_responder() {
    let (req, mut pl) = TestRequest::default()
        .header(
            header::CONTENT_TYPE,
            header::HeaderValue::from_static("application/cbor"),
        )
        .header(
            header::CONTENT_LENGTH,
            header::HeaderValue::from_static("16"),
        )
        .set_payload(get_test_bytes())
        .app_data(CborConfig::default().limit(10).error_handler(|err, _| {
            let msg = MyObject::default();
            let resp = HttpResponse::BadRequest()
                .body(serde_cbor::to_vec(&msg).unwrap());
            InternalError::from_response(err, resp).into()
        }))
        .to_http_parts();

    let s = Cbor::<MyObject>::from_request(&req, &mut pl).await;
    let mut resp = Response::from_error(s.err().unwrap());
    assert_eq!(resp.status(), StatusCode::BAD_REQUEST);

    let body = load_stream(resp.take_body()).await.unwrap();
    let msg: MyObject = serde_cbor::from_slice(&body).unwrap();
    assert_eq!(msg.name, "test");
}

#[actix_rt::test]
async fn test_extract() {
    let (req, mut pl) = TestRequest::default()
        .header(
            header::CONTENT_TYPE,
            header::HeaderValue::from_static("application/cbor"),
        )
        .header(
            header::CONTENT_LENGTH,
            header::HeaderValue::from_static("16"),
        )
        .set_payload(get_test_bytes())
        .to_http_parts();

    let s = Cbor::<MyObject>::from_request(&req, &mut pl).await.unwrap();
    assert_eq!(s.name, "test");
    assert_eq!(
        s.into_inner(),
        MyObject::default()
    );

    let (req, mut pl) = TestRequest::default()
        .header(
            header::CONTENT_TYPE,
            header::HeaderValue::from_static("application/cbor"),
        )
        .header(
            header::CONTENT_LENGTH,
            header::HeaderValue::from_static("16"),
        )
        .set_payload(get_test_bytes())
        .app_data(CborConfig::default().limit(10))
        .to_http_parts();

    let s = Cbor::<MyObject>::from_request(&req, &mut pl).await;
    assert!(format!("{}", s.err().unwrap())
        .contains("Cbor payload size is bigger than allowed"));

    let (req, mut pl) = TestRequest::default()
        .header(
            header::CONTENT_TYPE,
            header::HeaderValue::from_static("application/cbor"),
        )
        .header(
            header::CONTENT_LENGTH,
            header::HeaderValue::from_static("16"),
        )
        .set_payload(get_test_bytes())
        .app_data(
            CborConfig::default()
                .limit(10)
                .error_handler(|_, _| CborPayloadError::ContentType.into()),
        )
        .to_http_parts();
    let s = Cbor::<MyObject>::from_request(&req, &mut pl).await;
    assert!(format!("{}", s.err().unwrap()).contains("Content type error"));
}

#[actix_rt::test]
async fn test_cbor_body() {
    let (req, mut pl) = TestRequest::default().to_http_parts();
    let cbor = CborBody::<MyObject>::new(&req, &mut pl, None).await;
    assert!(cbor_eq(cbor.err().unwrap(), CborPayloadError::ContentType));

    let (req, mut pl) = TestRequest::default()
        .header(
            header::CONTENT_TYPE,
            header::HeaderValue::from_static("application/text"),
        )
        .to_http_parts();
    let cbor = CborBody::<MyObject>::new(&req, &mut pl, None).await;
    assert!(cbor_eq(cbor.err().unwrap(), CborPayloadError::ContentType));

    let (req, mut pl) = TestRequest::default()
        .header(
            header::CONTENT_TYPE,
            header::HeaderValue::from_static("application/cbor"),
        )
        .header(
            header::CONTENT_LENGTH,
            header::HeaderValue::from_static("10000"),
        )
        .to_http_parts();

    let cbor = CborBody::<MyObject>::new(&req, &mut pl, None)
        .limit(100)
        .await;
    assert!(cbor_eq(cbor.err().unwrap(), CborPayloadError::Overflow));

    let (req, mut pl) = TestRequest::default()
        .header(
            header::CONTENT_TYPE,
            header::HeaderValue::from_static("application/cbor"),
        )
        .header(
            header::CONTENT_LENGTH,
            header::HeaderValue::from_static("16"),
        )
        .set_payload(get_test_bytes())
        .to_http_parts();

    let cbor = CborBody::<MyObject>::new(&req, &mut pl, None).await;
    assert_eq!(
        cbor.ok().unwrap(),
        MyObject::default()
    );
}

#[actix_rt::test]
async fn test_with_cbor_and_bad_content_type() {
    let (req, mut pl) = TestRequest::with_header(
        header::CONTENT_TYPE,
        header::HeaderValue::from_static("text/plain"),
    )
        .header(
            header::CONTENT_LENGTH,
            header::HeaderValue::from_static("16"),
        )
        .set_payload(get_test_bytes())
        .app_data(CborConfig::default().limit(4096))
        .to_http_parts();

    let s = Cbor::<MyObject>::from_request(&req, &mut pl).await;
    assert!(s.is_err())
}

#[actix_rt::test]
async fn test_with_cbor_and_good_custom_content_type() {
    let (req, mut pl) = TestRequest::with_header(
        header::CONTENT_TYPE,
        header::HeaderValue::from_static("text/plain"),
    )
        .header(
            header::CONTENT_LENGTH,
            header::HeaderValue::from_static("16"),
        )
        .set_payload(get_test_bytes())
        .app_data(CborConfig::default().content_type_raw(|mime: &str| {
            mime == "text/plain"
        }))
        .to_http_parts();

    let s = Cbor::<MyObject>::from_request(&req, &mut pl).await;
    assert!(s.is_ok())
}

#[actix_rt::test]
async fn test_with_cbor_and_bad_custom_content_type() {
    let (req, mut pl) = TestRequest::with_header(
        header::CONTENT_TYPE,
        header::HeaderValue::from_static("text/html"),
    )
        .header(
            header::CONTENT_LENGTH,
            header::HeaderValue::from_static("16"),
        )
        .set_payload(get_test_bytes())
        .app_data(CborConfig::default().content_type_raw(|mime: &str| {
            mime == "text/plain"
        }))
        .to_http_parts();

    let s = Cbor::<MyObject>::from_request(&req, &mut pl).await;
    assert!(s.is_err())
}

#[actix_rt::test]
async fn test_with_config_in_data_wrapper() {
    let (req, mut pl) = TestRequest::default()
        .header(CONTENT_TYPE, HeaderValue::from_static("application/cbor"))
        .header(CONTENT_LENGTH, HeaderValue::from_static("16"))
        .set_payload(get_test_bytes())
        .app_data(web::Data::new(CborConfig::default().limit(10)))
        .to_http_parts();

    let s = Cbor::<MyObject>::from_request(&req, &mut pl).await;
    assert!(s.is_err());

    let err_str = s.err().unwrap().to_string();
    assert!(err_str.contains("Cbor payload size is bigger than allowed"));
}
