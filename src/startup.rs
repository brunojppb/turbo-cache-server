use actix_web::{
    App, HttpServer,
    dev::Server,
    middleware::{Logger, from_fn},
    web,
};
use std::{net::TcpListener, sync::Arc};

use tracing_actix_web::TracingLogger;

use crate::{
    app_settings::AppSettings,
    artifact_cache::ArtifactCache,
    auth::turbo_token::{TokenAuth, validate_turbo_token},
    http_span::DecayRootSpanBuilder,
    routes::{
        HEALTH_CHECK_PATH, artifacts_status, get_file, head_check_file, health_check, post_events,
        post_list_team_artifacts, put_file,
    },
    storage::Storage,
};

/// Must be called from within a Tokio runtime: with `TURBO_TOKENS_DIR` set,
/// it starts the task that re-reads the tokens directory.
pub fn run(listener: TcpListener, app_settings: AppSettings) -> Result<Server, std::io::Error> {
    let token_auth = TokenAuth::from_settings(&app_settings)?;
    if let TokenAuth::PerTeam(tokens) = &token_auth {
        Arc::clone(tokens).watch(app_settings.turbo_tokens_reload_interval);
    }
    let token_auth = web::Data::new(token_auth);
    let cache = web::Data::new(ArtifactCache::new(Storage::new(&app_settings)));
    let port = listener
        .local_addr()
        .expect("TCPListener should be valid")
        .port();
    let app_settings = web::Data::new(app_settings);
    let server = HttpServer::new(move || {
        let artifacts_scope = web::scope("/v8/artifacts")
            .route("", web::post().to(post_list_team_artifacts))
            .route("/status", web::get().to(artifacts_status))
            .route("/events", web::post().to(post_events))
            .route("/{hash}", web::put().to(put_file))
            .route("/{hash}", web::get().to(get_file))
            .route("/{hash}", web::head().to(head_check_file))
            .wrap(from_fn(validate_turbo_token));

        App::new()
            .wrap(Logger::default())
            // Registered after Logger so it runs outermost: the "HTTP request"
            // root span wraps the whole request and all handler/storage spans
            // nest under it.
            .wrap(TracingLogger::<DecayRootSpanBuilder>::new())
            .route(HEALTH_CHECK_PATH, web::get().to(health_check))
            .service(artifacts_scope)
            .app_data(app_settings.clone())
            .app_data(token_auth.clone())
            .app_data(cache.clone())
    })
    .listen(listener)?
    .run();

    tracing::info!(port = port, "Decay server started");

    Ok(server)
}
