# zino

`zino` is a full featured web application framework which focuses on productivity and performance.

[![Crates.io](https://img.shields.io/crates/v/zino)][zino]
[![Documentation](https://docs.rs/zino/badge.svg)][zino-docs]
[![Downloads](https://img.shields.io/crates/d/zino)][zino]


## Highlights

- 🚀 Out-of-the-box features for rapid application development.
- ✨ Minimal design, modular architecture and high-level abstractions.
- ⚡ Embrace practical conventions to get the best performance.
- 🐘 Highly optimized ORM for PostgreSQL built with [`sqlx`][sqlx].
- ⏲ Lightweight scheduler for sync and async cron jobs.
- 📊 Support for `tracing`, `logging` and `metrics`.

## Getting started

You can start with the example [`axum-app`].

## Crates

| Name            | Description            | Crates.io    | Documentation |
|-----------------|------------------------|--------------|---------------|
| [`zino-core`]   | Core types and traits. | [![Crates.io](https://img.shields.io/crates/v/zino-core)][zino-core] | [![Documentation](https://docs.rs/zino-core/badge.svg)][zino-core-docs] |
| [`zino-derive`] | Derived traits.        | [![Crates.io](https://img.shields.io/crates/v/zino-derive)][zino-derive] | [![Documentation](https://docs.rs/zino-derive/badge.svg)][zino-derive-docs] |
| [`zino-model`]  | Model types.           | [![Crates.io](https://img.shields.io/crates/v/zino-model)][zino-model] | [![Documentation](https://docs.rs/zino-model/badge.svg)][zino-model-docs] |

## License

This project is licensed under the [MIT license][license].

[`zino`]: https://github.com/photino/zino/tree/main/zino
[`zino-core`]: https://github.com/photino/zino/tree/main/zino-core
[`zino-derive`]: https://github.com/photino/zino/tree/main/zino-derive
[`zino-model`]: https://github.com/photino/zino/tree/main/zino-model
[zino]: https://crates.io/crates/zino
[zino-docs]: https://docs.rs/zino
[zino-core]: https://crates.io/crates/zino-core
[zino-core-docs]: https://docs.rs/zino-core
[zino-derive]: https://crates.io/crates/zino-derive
[zino-derive-docs]: https://docs.rs/zino-derive
[zino-model]: https://crates.io/crates/zino-model
[zino-model-docs]: https://docs.rs/zino-model
[sqlx]: https://crates.io/crates/sqlx
[`axum-app`]: https://github.com/photino/zino/tree/main/examples/axum-app
[license]: https://github.com/photino/zino/blob/main/LICENSE