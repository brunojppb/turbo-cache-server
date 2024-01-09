use actix_web::{dev::Server, web, App, HttpServer};
use std::net::TcpListener;

use crate::{
    routes::{get_file, health_check, post_events, put_file},
    storage::Storage,
};

const ONE_HUNDRED_MB_IN_BYTES: usize = 100 * 1024 * 1024;

pub fn run(listener: TcpListener, storage: Storage) -> Result<Server, std::io::Error> {
    let storage = web::Data::new(storage);
    let server = HttpServer::new(move || {
        App::new()
            .route("/management/health", web::get().to(health_check))
            .route("/v8/artifacts/events", web::post().to(post_events))
            .route("/v8/artifacts/{hash}", web::put().to(put_file))
            .route("/v8/artifacts/{hash}", web::get().to(get_file))
            .app_data(storage.clone())
            .app_data(actix_web::web::PayloadConfig::new(ONE_HUNDRED_MB_IN_BYTES))
    })
    .listen(listener)?
    .run();

    Ok(server)
}
