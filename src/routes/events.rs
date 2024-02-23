use actix_web::{HttpResponse, Responder};

// @TODO: Turborepo post specific events for metrics
// collections to this endpoint. For now, we just ignore it,
// but would be interesting to gather these metrics
#[tracing::instrument(name = "Record Turbo event")]
pub async fn post_events() -> impl Responder {
    HttpResponse::Created().finish()
}
