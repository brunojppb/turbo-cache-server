use actix_web::{HttpResponse, Responder};

#[tracing::instrument(name = "health check")]
pub async fn health_check() -> impl Responder {
    HttpResponse::Ok().finish()
}
