use actix_web::{HttpResponse, Responder};

pub async fn post_events() -> impl Responder {
    HttpResponse::Created().finish()
}
