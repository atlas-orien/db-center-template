use axum::http::StatusCode;
use db_core::error::{BIZ_INTERNAL_ERROR, BizError};
use error_code::{admin as admin_error, app as app_error};
use thiserror::Error;
use toolcraft_axum_kit::{ApiError, CommonError};

#[derive(Error, Debug)]
#[allow(dead_code)]
pub enum Error {
    #[error("config error: {0}")]
    #[allow(clippy::enum_variant_names)]
    Config(#[from] toolcraft_config::error::Error),

    #[error("validation error: {0}")]
    #[allow(clippy::enum_variant_names)]
    Validation(#[from] validator::ValidationErrors),

    #[error(transparent)]
    Jwt(#[from] toolcraft_jwt::error::Error), // catch-all for other errors

    #[error("custom error: {0}")]
    Custom(String),
}

pub type Result<T, E = Error> = core::result::Result<T, E>;

impl From<Error> for ApiError {
    fn from(err: Error) -> Self {
        (
            StatusCode::INTERNAL_SERVER_ERROR,
            CommonError {
                code: BIZ_INTERNAL_ERROR,
                message: err.to_string(),
            }
            .to_json(),
        )
    }
}

pub fn from_biz_error(err: BizError) -> ApiError {
    let status = match err.code() {
        app_error::APP_USER_NOT_FOUND => StatusCode::NOT_FOUND,
        app_error::APP_USER_DISABLED => StatusCode::FORBIDDEN,
        admin_error::ADMIN_USER_NOT_FOUND
        | admin_error::ADMIN_USER_DISABLED
        | admin_error::ADMIN_PERMISSION_DENIED
        | admin_error::ADMIN_ROLE_RESERVED => StatusCode::FORBIDDEN,
        admin_error::ADMIN_AUTH_USER_NOT_FOUND => StatusCode::NOT_FOUND,
        _ => StatusCode::BAD_REQUEST,
    };

    (
        status,
        CommonError {
            code: err.code(),
            message: err.message().to_string(),
        }
        .to_json(),
    )
}
