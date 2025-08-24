use actix_web::{
    App, HttpServer,
    dev::Server,
    middleware::{Logger, from_fn},
    web,
};
use std::net::TcpListener;

use crate::{
    app_settings::AppSettings,
    auth::turbo_token::validate_turbo_token,
    routes::{
        artifacts_status, get_file, head_check_file, health_check, post_events, post_list_team_artifacts, put_file,
    },
    storage::Storage,
};

pub fn run(listener: TcpListener, app_settings: AppSettings) -> Result<Server, std::io::Error> {
    let storage = Storage::new(&app_settings);
    let storage = web::Data::new(storage);
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
            .route("/management/health", web::get().to(health_check))
            .service(artifacts_scope)
            .app_data(app_settings.clone())
            .app_data(storage.clone())
            .app_data(actix_web::web::PayloadConfig::new(
                app_settings.max_payload_size_in_bytes,
            ))
    })
    .listen(listener)?
    .run();

    tracing::info!(port = port, "Decay server started");

    Ok(server)
}
