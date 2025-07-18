use actix_web::{
    Error, HttpResponse,
    body::MessageBody,
    dev::{ServiceRequest, ServiceResponse},
    middleware::Next,
    web::Data,
};

use crate::app_settings::AppSettings;

#[tracing::instrument(name = "Request Auth", skip(req, next))]
pub async fn validate_turbo_token(
    req: ServiceRequest,
    next: Next<impl MessageBody + 'static>,
) -> Result<ServiceResponse<impl MessageBody>, Error> {
    let app_settings = req.app_data::<Data<AppSettings>>().unwrap();
    let maybe_req_token = req.headers().get("Authorization");

    match (&app_settings.turbo_token, maybe_req_token) {
        (Some(token), Some(header_token)) => {
            let token = format!("Bearer {token}");
            if token == *header_token {
                return next.call(req).await.map(|v| v.map_into_boxed_body());
            } else {
                return Ok(unauthrorized(req));
            }
        }
        (Some(_token), None) => return Ok(unauthrorized(req)),
        _ => {
            tracing::info!("No token provided. skipping...");
            return next.call(req).await.map(|v| v.map_into_boxed_body());
        }
    }
}

fn unauthrorized(req: ServiceRequest) -> ServiceResponse {
    req.into_response(HttpResponse::Unauthorized().body("Unauthorized. Invalid TURBO_TOKEN"))
}
