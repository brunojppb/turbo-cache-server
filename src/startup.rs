use actix_web::{dev::Server, web, App, HttpServer};
use std::net::TcpListener;

use crate::{
    routes::{get_file, health_check, hello, put_file},
    storage::Storage,
};

pub fn run(listener: TcpListener, storage: Storage) -> Result<Server, std::io::Error> {
    let storage = web::Data::new(storage);
    let server = HttpServer::new(move || {
        App::new()
            .route("/", web::get().to(hello))
            .route("/management/health", web::get().to(health_check))
            .route("/{name}", web::get().to(hello))
            .route("/v8/artifacts/{hash}", web::put().to(put_file))
            .route("/v8/artifacts/{hash}", web::get().to(get_file))
            .app_data(storage.clone())
            .app_data(actix_web::web::PayloadConfig::new(999999))
    })
    .listen(listener)?
    .run();

    Ok(server)
}
