use std::net::TcpListener;

use actix_web::{dev::Server, web, App, HttpRequest, HttpResponse, HttpServer, Responder};

async fn say_hi(req: HttpRequest) -> impl Responder {
    let hi_to = req.match_info().get("name").unwrap_or("there");
    println!("{} - {}", req.method(), req.path());
    format!("Hi {}!", hi_to)
}

async fn health_check() -> impl Responder {
    HttpResponse::Ok().finish()
}

pub fn run(listener: TcpListener) -> Result<Server, std::io::Error> {
    let server = HttpServer::new(|| {
        App::new()
            .route("/", web::get().to(say_hi))
            .route("/management/health", web::get().to(health_check))
            .route("/{name}", web::get().to(say_hi))
    })
    .listen(listener)?
    .run();

    Ok(server)
}
