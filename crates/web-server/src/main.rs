mod clients;
mod dto;
mod error;
mod handlers;
mod logging;
mod notifications;
mod routes;
mod settings;
mod statics;

use std::{process::ExitCode, sync::Arc};

use clients::auth_client::fetch_verify_jwt;
use settings::Settings;
use toolcraft_axum_kit::http_server;
use tracing::error;

use crate::{
    logging::init_tracing_to_file,
    statics::{
        db_manager::init_db, llm_client::init_llm_clients, request_client::init_request_client,
    },
};

#[tokio::main]
async fn main() -> ExitCode {
    init_tracing_to_file();
    let settings = Settings::load("config/services.toml").unwrap();
    let internal_base_url = settings
        .jwt_verify
        .url
        .trim_end_matches("/jwt_verify_config")
        .to_owned();
    init_request_client(
        internal_base_url,
        settings.jwt_verify.header.clone(),
        settings.jwt_verify.token.clone(),
    )
    .expect("Request client initialization failed");
    init_db(settings.db.clone())
        .await
        .expect("DatabaseManager initialization failed");
    init_llm_clients(settings.llm.clone()).expect("LLM client initialization failed");
    let jwt = Arc::new(
        fetch_verify_jwt()
            .await
            .expect("VerifyJwt initialization failed"),
    );

    let router = routes::create_routes(jwt);
    let port = settings.http.port;

    match http_server::start(port, router).await {
        Ok(()) => ExitCode::SUCCESS,
        Err(err) => {
            error!(port, error = %err, "HTTP server failed");
            eprintln!("HTTP server failed on 0.0.0.0:{port}: {err}");
            ExitCode::FAILURE
        }
    }
}
