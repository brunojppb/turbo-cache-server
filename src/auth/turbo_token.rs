use actix_web::{
    Error, HttpResponse,
    body::MessageBody,
    dev::{ServiceRequest, ServiceResponse},
    http::header,
    middleware::Next,
    web::Data,
};

use secrecy::ExposeSecret;
use serde::Serialize;
use tracing::instrument;

use crate::app_settings::AppSettings;

#[derive(Serialize)]
struct AuthError {
    error: &'static str,
}

/// What the middleware decided about the incoming request.
enum Outcome {
    /// No `TURBO_TOKEN` is configured, so the server accepts every request.
    NoTokenConfigured,
    Valid,
    MissingHeader,
    InvalidToken,
}

/// Rejects requests that do not carry the configured `TURBO_TOKEN` as a Bearer token.
/// Lets every request through when no token is configured.
#[instrument(name = "validate_turbo_token", skip_all)]
pub async fn validate_turbo_token(
    req: ServiceRequest,
    next: Next<impl MessageBody + 'static>,
) -> Result<ServiceResponse<impl MessageBody>, Error> {
    // Scoped so no borrow of `req` outlives the decision and blocks the move below.
    let outcome = {
        let app_settings = req
            .app_data::<Data<AppSettings>>()
            .expect("AppSettings must be registered as app data");

        match &app_settings.turbo_token {
            None => Outcome::NoTokenConfigured,
            Some(expected) => {
                let provided = req
                    .headers()
                    .get(header::AUTHORIZATION)
                    .and_then(|value| value.to_str().ok())
                    .and_then(parse_bearer_token);

                match provided {
                    None => Outcome::MissingHeader,
                    Some(token) if token == expected.expose_secret() => Outcome::Valid,
                    Some(_) => Outcome::InvalidToken,
                }
            }
        }
    };

    match outcome {
        Outcome::NoTokenConfigured => {
            tracing::debug!("No TURBO_TOKEN configured. Skipping token validation");
            next.call(req).await.map(|v| v.map_into_boxed_body())
        }
        Outcome::Valid => next.call(req).await.map(|v| v.map_into_boxed_body()),
        Outcome::MissingHeader => {
            tracing::warn!("Rejected request with a missing or malformed Authorization header");
            Ok(unauthorized(
                req,
                "Missing or malformed Authorization header",
            ))
        }
        Outcome::InvalidToken => {
            tracing::warn!("Rejected request with an invalid TURBO_TOKEN");
            Ok(unauthorized(req, "Invalid TURBO_TOKEN"))
        }
    }
}

/// Extracts the token from an `Authorization` header value.
/// Returns `None` when the scheme is not Bearer or the token is empty.
fn parse_bearer_token(header_value: &str) -> Option<&str> {
    let (scheme, token) = header_value.split_once(' ')?;

    // RFC 7235 defines the auth scheme as case-insensitive.
    if !scheme.eq_ignore_ascii_case("Bearer") {
        return None;
    }

    let token = token.trim_start();
    if token.is_empty() { None } else { Some(token) }
}

fn unauthorized(req: ServiceRequest, error: &'static str) -> ServiceResponse {
    req.into_response(
        HttpResponse::Unauthorized()
            .insert_header((header::WWW_AUTHENTICATE, "Bearer"))
            .json(AuthError { error }),
    )
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_a_bearer_token() {
        assert_eq!(parse_bearer_token("Bearer secret"), Some("secret"));
    }

    #[test]
    fn parses_a_bearer_scheme_in_any_case() {
        assert_eq!(parse_bearer_token("bearer secret"), Some("secret"));
        assert_eq!(parse_bearer_token("BEARER secret"), Some("secret"));
        assert_eq!(parse_bearer_token("BeArEr secret"), Some("secret"));
    }

    #[test]
    fn parses_a_token_padded_with_extra_spaces() {
        assert_eq!(parse_bearer_token("Bearer   secret"), Some("secret"));
    }

    #[test]
    fn rejects_another_auth_scheme() {
        assert_eq!(parse_bearer_token("Basic secret"), None);
    }

    #[test]
    fn rejects_a_header_without_a_scheme() {
        assert_eq!(parse_bearer_token("secret"), None);
    }

    #[test]
    fn rejects_an_empty_token() {
        assert_eq!(parse_bearer_token("Bearer "), None);
        assert_eq!(parse_bearer_token("Bearer    "), None);
    }

    #[test]
    fn rejects_an_empty_header() {
        assert_eq!(parse_bearer_token(""), None);
    }

    #[test]
    fn keeps_the_token_exact() {
        // Tokens are compared byte for byte, so no case folding on the token itself.
        assert_eq!(parse_bearer_token("Bearer SeCrEt"), Some("SeCrEt"));
    }
}
