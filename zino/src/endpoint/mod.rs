cfg_if::cfg_if! {
    if #[cfg(feature = "axum")] {
        mod axum_sse;
        mod axum_websocket;

        pub(crate) use self::axum_sse::sse_handler;
        pub(crate) use self::axum_websocket::websocket_handler;
    }
}
