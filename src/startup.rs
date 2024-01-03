use actix_web::{dev::Server, web, App, HttpServer};
use dotenv;
use std::net::TcpListener;

use crate::{
    routes::{health_check, hello},
    storage::Storage,
};

pub fn run(listener: TcpListener, storage: Storage) -> Result<Server, std::io::Error> {
    dotenv::dotenv().ok();
    let storage = web::Data::new(storage);
    let server = HttpServer::new(move || {
        App::new()
            .route("/", web::get().to(hello))
            .route("/management/health", web::get().to(health_check))
            .route("/{name}", web::get().to(hello))
            .app_data(storage.clone())
    })
    .listen(listener)?
    .run();

    Ok(server)
}
