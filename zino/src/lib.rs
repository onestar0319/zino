//! [![github]](https://github.com/photino/zino)
//! [![crates-io]](https://crates.io/crates/zino)
//! [![docs-rs]](https://docs.rs/zino)
//!
//! [github]: https://img.shields.io/badge/github-8da0cb?labelColor=555555&logo=github
//! [crates-io]: https://img.shields.io/badge/crates.io-fc8d62?labelColor=555555&logo=rust
//! [docs-rs]: https://img.shields.io/badge/docs.rs-66c2a5?labelColor=555555&logo=docs.rs
//!
//! [`zino`] is a full-featured web application framework for Rust
//! with a focus on productivity and performance.
//!
//! ## Highlights
//!
//! - 🚀 Out-of-the-box features for rapid application development.
//! - ✨ Minimal design, modular architecture and high-level abstractions.
//! - ⚡ Embrace practical conventions to get the best performance.
//! - 🐘 Highly optimized ORM for PostgreSQL built on top of [`sqlx`].
//! - 🕗 Lightweight scheduler for sync and async cron jobs.
//! - 💠 Unified access to storage services, data sources and chatbots.
//! - 📊 Support for [`tracing`], [`metrics`] and logging.
//!
//! ## Getting started
//!
//! You can start with the example [`axum-app`].
//!
//! ## Feature flags
//!
//! Currently, we only provide the `axum` feature to enable an integration with [`axum`].
//!
//! [`zino`]: https://github.com/photino/zino
//! [`sqlx`]: https://crates.io/crates/sqlx
//! [`tracing`]: https://crates.io/crates/tracing
//! [`metrics`]: https://crates.io/crates/metrics
//! [`axum`]: https://crates.io/crates/axum
//! [`axum-app`]: https://github.com/photino/zino/tree/main/examples/axum-app

#![feature(async_fn_in_trait)]
#![feature(doc_auto_cfg)]
#![feature(lazy_cell)]
#![feature(result_option_inspect)]
#![feature(string_leak)]
#![forbid(unsafe_code)]

mod channel;
mod cluster;
mod endpoint;
mod middleware;
mod request;

#[doc(no_inline)]
pub use zino_core::{
    application::Application,
    database::Schema,
    datetime::DateTime,
    error::Error,
    extension::JsonObjectExt,
    model::{Model, Mutation, Query},
    request::{RequestContext, Validation},
    response::{ExtractRejection, Rejection},
    schedule::{AsyncCronJob, CronJob},
    BoxFuture, Map, Record, Uuid,
};

cfg_if::cfg_if! {
    if #[cfg(feature = "axum")] {
        use axum::{
            body::{Body, Bytes, Full},
            http,
        };

        pub use cluster::axum_cluster::AxumCluster;
        pub use request::axum_request::AxumExtractor;

        /// A specialized request extractor for `axum`.
        pub type Request<B = Body> = AxumExtractor<http::Request<B>>;

        /// A specialized response for `axum`.
        pub type Response = zino_core::response::Response<http::StatusCode>;

        /// A specialized `Result` type for `axum`.
        pub type Result<T = http::Response<Full<Bytes>>> =
            std::result::Result<T, http::Response<Full<Bytes>>>;
    }
}
