use crate::{
    controller::{stats, task, user},
    middleware,
};
use axum::{
    middleware::from_fn,
    routing::{get, post},
    Router,
};

pub(crate) fn routes() -> Vec<Router> {
    let mut routes = Vec::new();

    // User controller.
    let router = Router::new()
        .route("/user/new", post(user::new))
        .route("/user/:id/update", post(user::update))
        .route("/user/list", get(user::list))
        .route("/user/:id/view", get(user::view));
    routes.push(router);

    // Task controller.
    let router = Router::new().route("/task/execute", post(task::execute));
    routes.push(router);

    // Stats controller.
    let router = Router::new()
        .route("/stats", get(stats::index))
        .layer(from_fn(middleware::check_client_ip));
    routes.push(router);

    routes
}
