use actix_web::{dev::Server, middleware::Logger, web, App, HttpServer};
use std::net::TcpListener;

use crate::{
    app_settings::AppSettings,
    routes::{get_file, health_check, post_events, put_file},
    storage::Storage,
};

pub fn run(listener: TcpListener, app_settings: AppSettings) -> Result<Server, std::io::Error> {
    let storage = Storage::new(&app_settings);
    let storage = web::Data::new(storage);
    let server = HttpServer::new(move || {
        App::new()
            .wrap(Logger::default())
            .route("/management/health", web::get().to(health_check))
            .route("/v8/artifacts/events", web::post().to(post_events))
            .route("/v8/artifacts/{hash}", web::put().to(put_file))
            .route("/v8/artifacts/{hash}", web::get().to(get_file))
            .app_data(storage.clone())
            .app_data(actix_web::web::PayloadConfig::new(
                app_settings.max_payload_size_in_bytes,
            ))
    })
    .listen(listener)?
    .run();

    println!("Decay server started");

    Ok(server)
}
