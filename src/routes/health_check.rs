use actix_web::{HttpResponse, Responder};

pub const HEALTH_CHECK_PATH: &str = "/management/health";

#[tracing::instrument(name = "health check")]
pub async fn health_check() -> impl Responder {
    HttpResponse::Ok().finish()
}
