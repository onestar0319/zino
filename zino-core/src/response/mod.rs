//! Constructing responses and rejections.

use crate::{
    request::{RequestContext, Validation},
    trace::TraceContext,
    SharedString, Uuid,
};
use bytes::Bytes;
use http::header::{self, HeaderValue};
use http_body::Full;
use http_types::trace::{Metric, ServerTiming};
use serde::Serialize;
use serde_json::Value;
use std::{
    borrow::Cow,
    marker::PhantomData,
    time::{Duration, Instant},
};

mod rejection;

pub use rejection::Rejection;

/// Response code.
/// See [Problem Details for HTTP APIs](https://tools.ietf.org/html/rfc7807).
pub trait ResponseCode {
    /// 200 Ok.
    const OK: Self;

    /// Status code.
    fn status_code(&self) -> u16;

    /// Error code.
    fn error_code(&self) -> Option<SharedString>;

    /// Returns `true` if the response is successful.
    fn is_success(&self) -> bool;

    /// A URI reference that identifies the problem type.
    /// For successful response, it should be `None`.
    fn type_uri(&self) -> Option<SharedString>;

    /// A short, human-readable summary of the problem type.
    /// For successful response, it should be `None`.
    fn title(&self) -> Option<SharedString>;

    /// A context-specific descriptive message. If the response is not successful,
    /// it should be a human-readable explanation specific to this occurrence of the problem.
    fn message(&self) -> Option<SharedString>;
}

/// An HTTP response.
#[derive(Debug, Serialize)]
#[serde(rename_all = "snake_case")]
pub struct Response<S> {
    /// A URI reference that identifies the problem type.
    #[serde(rename = "type")]
    #[serde(skip_serializing_if = "Option::is_none")]
    type_uri: Option<SharedString>,
    /// A short, human-readable summary of the problem type.
    #[serde(skip_serializing_if = "Option::is_none")]
    title: Option<SharedString>,
    /// Status code.
    #[serde(rename = "status")]
    status_code: u16,
    /// Error code.
    #[serde(rename = "error")]
    #[serde(skip_serializing_if = "Option::is_none")]
    error_code: Option<SharedString>,
    /// A human-readable explanation specific to this occurrence of the problem.
    #[serde(skip_serializing_if = "Option::is_none")]
    detail: Option<SharedString>,
    /// A URI reference that identifies the specific occurrence of the problem.
    #[serde(skip_serializing_if = "Option::is_none")]
    instance: Option<SharedString>,
    /// Indicates the response is successful or not.
    success: bool,
    /// A context-specific descriptive message for successful response.
    #[serde(skip_serializing_if = "Option::is_none")]
    message: Option<SharedString>,
    /// Start time.
    #[serde(skip)]
    start_time: Instant,
    /// Request ID.
    #[serde(skip_serializing_if = "Uuid::is_nil")]
    request_id: Uuid,
    /// Response data.
    #[serde(skip_serializing_if = "Value::is_null")]
    data: Value,
    /// Content type.
    #[serde(skip)]
    content_type: Option<SharedString>,
    /// Trace context.
    #[serde(skip)]
    trace_context: Option<TraceContext>,
    /// Server timing.
    #[serde(skip)]
    server_timing: ServerTiming,
    /// Phantom type of response code.
    #[serde(skip)]
    phantom: PhantomData<S>,
}

impl<S: ResponseCode> Response<S> {
    /// Creates a new instance.
    pub fn new(code: S) -> Self {
        let success = code.is_success();
        let message = code.message();
        let mut res = Self {
            type_uri: code.type_uri(),
            title: code.title(),
            status_code: code.status_code(),
            error_code: code.error_code(),
            detail: None,
            instance: None,
            success,
            message: None,
            start_time: Instant::now(),
            request_id: Uuid::nil(),
            data: Value::Null,
            content_type: None,
            trace_context: None,
            server_timing: ServerTiming::new(),
            phantom: PhantomData,
        };
        if success {
            res.message = message;
        } else {
            res.detail = message;
        }
        res
    }

    /// Creates a new instance with the request context.
    pub fn with_context<T: RequestContext>(code: S, ctx: &T) -> Self {
        let success = code.is_success();
        let message = code.message();
        let mut res = Self {
            type_uri: code.type_uri(),
            title: code.title(),
            status_code: code.status_code(),
            error_code: code.error_code(),
            detail: None,
            instance: (!success).then(|| ctx.request_path().to_string().into()),
            success,
            message: None,
            start_time: ctx.start_time(),
            request_id: ctx.request_id(),
            data: Value::Null,
            content_type: None,
            trace_context: None,
            server_timing: ServerTiming::new(),
            phantom: PhantomData,
        };
        if success {
            res.message = message;
        } else {
            res.detail = message;
        }
        res.trace_context = Some(ctx.new_trace_context().child());
        res
    }

    /// Provides the request context for the response.
    pub fn provide_context<T: RequestContext>(mut self, ctx: &T) -> Self {
        self.instance = (!self.is_success()).then(|| ctx.request_path().to_string().into());
        self.start_time = ctx.start_time();
        self.request_id = ctx.request_id();
        self.trace_context = Some(ctx.new_trace_context().child());
        self
    }

    /// Sets the code.
    pub fn set_code(&mut self, code: S) {
        let success = code.is_success();
        let message = code.message();
        self.type_uri = code.type_uri();
        self.title = code.title();
        self.status_code = code.status_code();
        self.error_code = code.error_code();
        self.success = success;
        if success {
            self.detail = None;
            self.message = message;
        } else {
            self.detail = message;
            self.message = None;
        }
    }

    /// Sets a URI reference that identifies the specific occurrence of the problem.
    pub fn set_instance(&mut self, instance: impl Into<Option<SharedString>>) {
        self.instance = instance.into();
    }

    /// Sets the message. If the response is not successful,
    /// it should be a human-readable explanation specific to this occurrence of the problem.
    pub fn set_message(&mut self, message: impl Into<SharedString>) {
        if self.is_success() {
            self.detail = None;
            self.message = Some(message.into());
        } else {
            self.detail = Some(message.into());
            self.message = None;
        }
    }

    /// Sets the response data.
    #[inline]
    pub fn set_data(&mut self, data: impl Into<Value>) {
        self.data = data.into();
    }

    /// Sets the content type.
    #[inline]
    pub fn set_content_type(&mut self, content_type: impl Into<SharedString>) {
        self.content_type = Some(content_type.into());
    }

    /// Records a server timing entry.
    pub fn record_server_timing(
        &mut self,
        name: impl Into<String>,
        dur: impl Into<Option<Duration>>,
        desc: impl Into<Option<String>>,
    ) {
        match Metric::new(name.into(), dur.into(), desc.into()) {
            Ok(entry) => self.server_timing.push(entry),
            Err(err) => tracing::error!("{err}"),
        }
    }

    /// Returns `true` if the response is successful or `false` otherwise.
    #[inline]
    pub fn is_success(&self) -> bool {
        self.success
    }

    /// Returns the request ID.
    #[inline]
    pub fn request_id(&self) -> Uuid {
        self.request_id
    }

    /// Returns the trace ID.
    pub fn trace_id(&self) -> Uuid {
        match self.trace_context {
            Some(ref trace_context) => Uuid::from_u128(trace_context.trace_id()),
            None => Uuid::nil(),
        }
    }
}

impl ResponseCode for http::StatusCode {
    const OK: Self = http::StatusCode::OK;

    #[inline]
    fn status_code(&self) -> u16 {
        self.as_u16()
    }

    #[inline]
    fn error_code(&self) -> Option<SharedString> {
        None
    }

    #[inline]
    fn is_success(&self) -> bool {
        self.is_success()
    }

    #[inline]
    fn type_uri(&self) -> Option<SharedString> {
        None
    }

    #[inline]
    fn title(&self) -> Option<SharedString> {
        if self.is_success() {
            None
        } else {
            self.canonical_reason().map(Cow::Borrowed)
        }
    }

    #[inline]
    fn message(&self) -> Option<SharedString> {
        if self.is_success() {
            self.canonical_reason().map(Cow::Borrowed)
        } else {
            None
        }
    }
}

impl Default for Response<http::StatusCode> {
    #[inline]
    fn default() -> Self {
        Self::new(http::StatusCode::OK)
    }
}

impl From<Validation> for Response<http::StatusCode> {
    fn from(validation: Validation) -> Self {
        if validation.is_success() {
            Self::new(http::StatusCode::OK)
        } else {
            let mut res = Self::new(http::StatusCode::BAD_REQUEST);
            res.set_data(validation.into_map());
            res
        }
    }
}

impl From<Response<http::StatusCode>> for http::Response<Full<Bytes>> {
    fn from(mut response: Response<http::StatusCode>) -> Self {
        let status_code = response.status_code;
        let mut res = match response.content_type {
            Some(ref content_type) => match serde_json::to_vec(&response.data) {
                Ok(bytes) => http::Response::builder()
                    .status(status_code)
                    .header(header::CONTENT_TYPE, content_type.as_ref())
                    .body(Full::from(bytes))
                    .unwrap_or_default(),
                Err(err) => http::Response::builder()
                    .status(http::StatusCode::INTERNAL_SERVER_ERROR)
                    .header(header::CONTENT_TYPE, "text/plain")
                    .body(Full::from(err.to_string()))
                    .unwrap_or_default(),
            },
            None => match serde_json::to_vec(&response) {
                Ok(bytes) => {
                    let content_type = if response.is_success() {
                        "application/json"
                    } else {
                        "application/problem+json"
                    };
                    http::Response::builder()
                        .status(status_code)
                        .header(header::CONTENT_TYPE, content_type)
                        .body(Full::from(bytes))
                        .unwrap_or_default()
                }
                Err(err) => http::Response::builder()
                    .status(http::StatusCode::INTERNAL_SERVER_ERROR)
                    .header(header::CONTENT_TYPE, "text/plain")
                    .body(Full::from(err.to_string()))
                    .unwrap_or_default(),
            },
        };
        let trace_context = match response.trace_context {
            Some(ref trace_context) => trace_context.to_string(),
            None => TraceContext::new().to_string(),
        };
        if let Ok(header_value) = HeaderValue::try_from(trace_context) {
            res.headers_mut().insert("traceparent", header_value);
        }

        let duration = response.start_time.elapsed();
        response.record_server_timing("total", duration, None);
        if let Ok(header_value) = HeaderValue::try_from(response.server_timing.value().as_str()) {
            res.headers_mut().insert("server-timing", header_value);
        }

        let request_id = response.request_id;
        if !request_id.is_nil() {
            if let Ok(header_value) = HeaderValue::try_from(request_id.to_string()) {
                res.headers_mut().insert("x-request-id", header_value);
            }
        }

        // Emit metrics.
        let labels = [("status_code", status_code.to_string())];
        metrics::decrement_gauge!("zino_http_requests_pending", 1.0);
        metrics::increment_counter!("zino_http_responses_total", &labels);
        metrics::histogram!(
            "zino_http_requests_duration_seconds",
            duration.as_secs_f64(),
            &labels,
        );

        res
    }
}

impl ResponseCode for http_types::StatusCode {
    const OK: Self = http_types::StatusCode::Ok;

    #[inline]
    fn status_code(&self) -> u16 {
        *self as u16
    }

    #[inline]
    fn error_code(&self) -> Option<SharedString> {
        None
    }

    #[inline]
    fn is_success(&self) -> bool {
        self.is_success()
    }

    #[inline]
    fn type_uri(&self) -> Option<SharedString> {
        None
    }

    #[inline]
    fn title(&self) -> Option<SharedString> {
        (!self.is_success()).then(|| self.canonical_reason().into())
    }

    #[inline]
    fn message(&self) -> Option<SharedString> {
        self.is_success().then(|| self.canonical_reason().into())
    }
}

impl Default for Response<http_types::StatusCode> {
    #[inline]
    fn default() -> Self {
        Self::new(http_types::StatusCode::Ok)
    }
}

impl From<Validation> for Response<http_types::StatusCode> {
    fn from(validation: Validation) -> Self {
        if validation.is_success() {
            Self::new(http_types::StatusCode::Ok)
        } else {
            let mut res = Self::new(http_types::StatusCode::BadRequest);
            res.set_data(validation.into_map());
            res
        }
    }
}
