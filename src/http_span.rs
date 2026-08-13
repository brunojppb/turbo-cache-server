use actix_web::{Error, body::MessageBody, dev::ServiceRequest, dev::ServiceResponse};
use tracing::Span;
use tracing_actix_web::{DefaultRootSpanBuilder, RootSpanBuilder, root_span};

use crate::routes::HEALTH_CHECK_PATH;

/// Root span builder for the HTTP layer: every request gets a standard
/// "HTTP request" root span (method, route, status code), so handler and
/// storage spans nest under it instead of the auth middleware span acting
/// as the trace root.
///
/// Health checks are exempt: orchestrators probe them every few seconds
/// and their spans would drown out real traffic in the tracing backend.
/// `Span::none()` skips the root span entirely. The handler's own span in
/// `routes::health_check` is unaffected and can be silenced independently
/// via the env filter (e.g. `RUST_LOG=info,decay::routes::health_check=off`).
pub struct DecayRootSpanBuilder;

impl RootSpanBuilder for DecayRootSpanBuilder {
    fn on_request_start(request: &ServiceRequest) -> Span {
        if request.path() == HEALTH_CHECK_PATH {
            Span::none()
        } else {
            root_span!(request)
        }
    }

    fn on_request_end<B: MessageBody>(span: Span, outcome: &Result<ServiceResponse<B>, Error>) {
        DefaultRootSpanBuilder::on_request_end(span, outcome);
    }
}
