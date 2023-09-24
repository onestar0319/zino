use actix_web::{
    body::BoxBody,
    http::{
        header::{self, HeaderName, HeaderValue},
        StatusCode,
    },
    HttpRequest, HttpResponse, Responder, ResponseError,
};
use std::fmt;
use zino_core::{
    response::{Rejection, Response, ResponseCode},
    trace::TimingMetric,
};

/// An HTTP response for `actix-web`.
pub struct ActixResponse<S>(Response<S>);

impl<S: ResponseCode> From<Response<S>> for ActixResponse<S> {
    #[inline]
    fn from(response: Response<S>) -> Self {
        Self(response)
    }
}

impl Responder for ActixResponse<StatusCode> {
    type Body = BoxBody;

    fn respond_to(self, req: &HttpRequest) -> HttpResponse<Self::Body> {
        let mut response = self.0;
        if !response.has_context() {
            let req = crate::Request::from(req.clone());
            response = response.context(&req);
        }

        let mut res = build_http_response(&mut response);
        for (key, value) in response.finalize() {
            if let Ok(header_name) = HeaderName::try_from(key.as_ref()) &&
                let Ok(header_value) = HeaderValue::try_from(value)
            {
                res.headers_mut().insert(header_name, header_value);
            }
        }

        res
    }
}

/// An HTTP rejection response for `actix-web`.
pub struct ActixRejection(Response<StatusCode>);

impl fmt::Debug for ActixRejection {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        write!(f, "{}", self.0.message().unwrap_or("OK"))
    }
}

impl fmt::Display for ActixRejection {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        write!(f, "{}", self.0.status_code())
    }
}

impl From<Rejection> for ActixRejection {
    #[inline]
    fn from(rejection: Rejection) -> Self {
        Self(Response::from(rejection))
    }
}

impl ResponseError for ActixRejection {
    #[inline]
    fn status_code(&self) -> StatusCode {
        let response = &self.0;
        response
            .status_code()
            .try_into()
            .unwrap_or(StatusCode::INTERNAL_SERVER_ERROR)
    }

    fn error_response(&self) -> HttpResponse<BoxBody> {
        let mut response = self.0.clone();
        let mut res = build_http_response(&mut response);
        let request_id = response.request_id();
        if !request_id.is_nil() {
            if let Ok(header_value) = HeaderValue::try_from(request_id.to_string()) {
                let header_name = HeaderName::from_static("x-request-id");
                res.headers_mut().insert(header_name, header_value);
            }
        }

        let (traceparent, tracestate) = response.trace_context();
        if let Ok(header_value) = HeaderValue::try_from(traceparent) {
            let header_name = HeaderName::from_static("traceparent");
            res.headers_mut().insert(header_name, header_value);
        }
        if let Ok(header_value) = HeaderValue::try_from(tracestate) {
            let header_name = HeaderName::from_static("tracestate");
            res.headers_mut().insert(header_name, header_value);
        }

        let response_time = response.response_time();
        let timing = TimingMetric::new("total".into(), None, response_time.into());
        if let Ok(header_value) = HeaderValue::try_from(timing.to_string()) {
            let header_name = HeaderName::from_static("server-timing");
            res.headers_mut().insert(header_name, header_value);
        }

        for (key, value) in response.headers() {
            if let Ok(header_name) = HeaderName::try_from(key.as_ref()) &&
                let Ok(header_value) = HeaderValue::try_from(value)
            {
                res.headers_mut().insert(header_name, header_value);
            }
        }

        res
    }
}

/// Build http response from `zino_core::response::Response`.
fn build_http_response(response: &mut Response<StatusCode>) -> HttpResponse<BoxBody> {
    match response.read_bytes() {
        Ok(data) => {
            let status_code = response
                .status_code()
                .try_into()
                .unwrap_or(StatusCode::INTERNAL_SERVER_ERROR);
            let body = BoxBody::new(data);
            let mut res = HttpResponse::with_body(status_code, body);
            if let Ok(header_value) = HeaderValue::try_from(response.content_type()) {
                res.headers_mut().insert(header::CONTENT_TYPE, header_value);
            }
            res
        }
        Err(err) => {
            let status_code = StatusCode::INTERNAL_SERVER_ERROR;
            let body = BoxBody::new(err.to_string());
            let mut res = HttpResponse::with_body(status_code, body);
            res.headers_mut().insert(
                header::CONTENT_TYPE,
                HeaderValue::from_static("text/plain; charset=utf-8"),
            );
            res
        }
    }
}
