# zino

`zino` is a full-featured web application framework for Rust with a focus on
productivity and performance.

## Highlights

- 🚀 Out-of-the-box features for rapid application development.
- ✨ Minimal design, modular architecture and high-level abstractions.
- ⚡ Embrace practical conventions to get the best performance.
- 🐘 Highly optimized ORM for PostgreSQL built on top of [`sqlx`].
- 🕗 Lightweight scheduler for sync and async cron jobs.
- 💠 Unified access to storage services, data sources and chatbots.
- 📊 Support for [`tracing`], [`metrics`] and logging.

## Getting started

You can start with the example [`axum-app`].

## Feature flags

Currently, we only provide the `axum` feature to enable an integration with [`axum`].

[`sqlx`]: https://crates.io/crates/sqlx
[`tracing`]: https://crates.io/crates/tracing
[`metrics`]: https://crates.io/crates/metrics
[`axum`]: https://crates.io/crates/axum
[`axum-app`]: https://github.com/photino/zino/tree/main/examples/axum-app
