use actix_web::{HttpRequest, Responder};

pub async fn hello(req: HttpRequest) -> impl Responder {
    let hi_to = req.match_info().get("name").unwrap_or("there");
    println!("{} - {}", req.method(), req.path());
    format!("Hi {}!", hi_to)
}
