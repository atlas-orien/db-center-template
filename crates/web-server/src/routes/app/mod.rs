use axum::{
    Router,
    middleware::from_fn,
    routing::{get, post},
};
use toolcraft_axum_kit::middleware::auth_mw::auth;
use toolcraft_jwt::VerifyJwt;

use crate::handlers::app::{current_user_permissions, register_app_user};

pub fn app_routes() -> Router {
    Router::new().merge(
        Router::new()
            .route("/register", post(register_app_user))
            .route("/me/permissions", get(current_user_permissions))
            .route_layer(from_fn(auth::<VerifyJwt>)),
    )
}
