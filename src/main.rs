use actix_web::{web, App, HttpRequest, HttpServer, Responder};

async fn say_hi(req: HttpRequest) -> impl Responder {
    let hi_to = req.match_info().get("name").unwrap_or("there");
    format!("Hi {}!", hi_to)
}

#[tokio::main]
async fn main() -> Result<(), std::io::Error> {
    println!("Running Decay server...");
    HttpServer::new(|| {
        App::new()
            .route("/", web::get().to(say_hi))
            .route("/{name}", web::get().to(say_hi))
    })
    .bind("127.0.0.1:8000")?
    .run()
    .await
}
