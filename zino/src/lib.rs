//! [![github]](https://github.com/photino/zino)
//! [![crates-io]](https://crates.io/crates/zino)
//! [![docs-rs]](https://docs.rs/zino)
//!
//! [github]: https://img.shields.io/badge/github-8da0cb?labelColor=555555&logo=github
//! [crates-io]: https://img.shields.io/badge/crates.io-fc8d62?labelColor=555555&logo=rust
//! [docs-rs]: https://img.shields.io/badge/docs.rs-66c2a5?labelColor=555555&logo=docs.rs
//!
//! [`zino`] is a **next-generation** framework for **composable** applications in Rust
//! which emphasizes **simplicity**, **extensibility** and **productivity**.
//!
//! ## Highlights
//!
//! - 🚀 Out-of-the-box features for rapid application development.
//! - ✨ Minimal design, composable architecture and high-level abstractions.
//! - 🌐 Adopt an API-first approch to development with open standards.
//! - ⚡ Embrace practical conventions to get the best performance.
//! - 💎 Highly optimized ORM for MySQL and PostgreSQL based on [`sqlx`].
//! - 📅 Lightweight scheduler for sync and async cron jobs.
//! - 💠 Unified access to storage services, data sources and chatbots.
//! - 📊 Built-in support for [`tracing`], [`metrics`] and logging.
//! - 🎨 Full integrations with [`actix-web`] and [`axum`].
//!
//! ## Getting started
//!
//! You can start with the example [`actix-app`] or [`axum-app`].
//!
//! ## Feature flags
//!
//! The following optional features are available:
//!
//! | Name         | Description                                          | Default? |
//! |--------------|------------------------------------------------------|----------|
//! | `actix`      | Enables the integration with [`actix-web`].          | No       |
//! | `axum`       | Enables the integration with [`axum`].               | No       |
//! | `orm`        | Enables the ORM for MySQL or **PostgreSQL**.         | Yes      |
//! | `view`       | Enables the HTML template rendering.                 | Yes      |
//!
//! [`zino`]: https://github.com/photino/zino
//! [`sqlx`]: https://crates.io/crates/sqlx
//! [`tracing`]: https://crates.io/crates/tracing
//! [`metrics`]: https://crates.io/crates/metrics
//! [`actix-web`]: https://crates.io/crates/actix-web
//! [`axum`]: https://crates.io/crates/axum
//! [`actix-app`]: https://github.com/photino/zino/tree/main/examples/actix-app
//! [`axum-app`]: https://github.com/photino/zino/tree/main/examples/axum-app

#![feature(async_fn_in_trait)]
#![feature(doc_auto_cfg)]
#![feature(lazy_cell)]
#![feature(result_option_inspect)]
#![forbid(unsafe_code)]

mod channel;
mod cluster;
mod controller;
mod endpoint;
mod middleware;
mod request;
mod response;

pub mod prelude;

pub use controller::DefaultController;

cfg_if::cfg_if! {
    if #[cfg(feature = "actix")] {
        use actix_web::{http::StatusCode, web::ServiceConfig, HttpRequest};

        use cluster::actix_cluster::ActixCluster;
        use request::actix_request::ActixExtractor;
        use response::actix_response::{ActixRejection, ActixResponse};

        /// Cluster for `actix-web`.
        pub type Cluster = ActixCluster;

        /// Router configure for `actix-web`.
        pub type RouterConfigure = fn(cfg: &mut ServiceConfig);

        /// A specialized request extractor for `actix-web`.
        pub type Request = ActixExtractor<HttpRequest>;

        /// A specialized response for `actix-web`.
        pub type Response = zino_core::response::Response<StatusCode>;

        /// A specialized `Result` type for `actix-web`.
        pub type Result<T = ActixResponse<StatusCode>> = std::result::Result<T, ActixRejection>;
    } else if #[cfg(feature = "axum")] {
        use axum::{body::Body, http::{self, StatusCode}};

        use cluster::axum_cluster::AxumCluster;
        use request::axum_request::AxumExtractor;
        use response::axum_response::{AxumRejection, AxumResponse};

        pub use channel::axum_channel::MessageChannel;

        /// Cluster for `axum`.
        pub type Cluster = AxumCluster;

        /// A specialized request extractor for `axum`.
        pub type Request = AxumExtractor<http::Request<Body>>;

        /// A specialized response for `axum`.
        pub type Response = zino_core::response::Response<StatusCode>;

        /// A specialized `Result` type for `axum`.
        pub type Result<T = AxumResponse<StatusCode>> = std::result::Result<T, AxumRejection>;
    }
}
