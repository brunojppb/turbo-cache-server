use std::net::TcpListener;

use actix_web::{dev::Server, web, App, HttpServer};

use crate::routes::{health_check, hello};

pub fn run(listener: TcpListener) -> Result<Server, std::io::Error> {
    let server = HttpServer::new(|| {
        App::new()
            .route("/", web::get().to(hello))
            .route("/management/health", web::get().to(health_check))
            .route("/{name}", web::get().to(hello))
    })
    .listen(listener)?
    .run();

    Ok(server)
}
