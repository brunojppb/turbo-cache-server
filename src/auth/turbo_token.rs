use std::{io, sync::Arc};

use actix_web::{
    Error, HttpResponse,
    body::MessageBody,
    dev::{ServiceRequest, ServiceResponse},
    http::header,
    middleware::Next,
    web::Data,
};

use secrecy::{ExposeSecret, SecretString};
use serde::Serialize;
use subtle::ConstantTimeEq;

use crate::{
    app_settings::AppSettings,
    auth::team_tokens::{TeamCheck, TeamTokens},
    routes::team_from_query,
};

#[derive(Serialize)]
struct AuthError {
    error: &'static str,
}

/// How the server authenticates artifact requests.
pub enum TokenAuth {
    /// Neither `TURBO_TOKEN` nor `TURBO_TOKENS_DIR` is set: every request is accepted.
    Open,
    /// `TURBO_TOKEN`: one token shared by every team.
    Shared(SecretString),
    /// `TURBO_TOKENS_DIR`: each team has its own token, valid for that team only.
    PerTeam(Arc<TeamTokens>),
}

impl TokenAuth {
    /// Picks the mode from the settings and reads `TURBO_TOKENS_DIR` once.
    /// Fails when both variables are set, since either reading of that is a
    /// guess, and when the tokens directory cannot be read.
    pub fn from_settings(settings: &AppSettings) -> io::Result<Self> {
        match (&settings.turbo_token, &settings.turbo_tokens_dir) {
            (Some(_), Some(_)) => Err(io::Error::new(
                io::ErrorKind::InvalidInput,
                "TURBO_TOKEN and TURBO_TOKENS_DIR are both set. Set only one",
            )),
            (Some(token), None) => Ok(Self::Shared(token.clone())),
            (None, Some(dir)) => TeamTokens::load(dir)
                .map(|tokens| Self::PerTeam(Arc::new(tokens)))
                .map_err(|error| {
                    io::Error::new(
                        error.kind(),
                        format!("Could not read TURBO_TOKENS_DIR {}: {error}", dir.display()),
                    )
                }),
            (None, None) => Ok(Self::Open),
        }
    }
}

/// What the middleware decided about the incoming request.
enum Outcome {
    /// No token is configured, so the server accepts every request.
    NoTokenConfigured,
    Valid,
    MissingHeader,
    InvalidToken,
    /// A per-team token that is valid, but not for the team the request names.
    WrongTeam,
}

/// Rejects requests that do not carry a valid token as a Bearer token: the
/// shared `TURBO_TOKEN`, or the token of the team the request names when
/// `TURBO_TOKENS_DIR` is set. Lets every request through when neither is configured.
pub async fn validate_turbo_token(
    req: ServiceRequest,
    next: Next<impl MessageBody + 'static>,
) -> Result<ServiceResponse<impl MessageBody>, Error> {
    // Scoped so no borrow of `req` outlives the decision and blocks the move below.
    let outcome = {
        let token_auth = req
            .app_data::<Data<TokenAuth>>()
            .expect("TokenAuth must be registered as app data");

        let provided = || {
            req.headers()
                .get(header::AUTHORIZATION)
                .and_then(|value| value.to_str().ok())
                .and_then(parse_bearer_token)
        };

        match token_auth.as_ref() {
            TokenAuth::Open => Outcome::NoTokenConfigured,
            TokenAuth::Shared(expected) => match provided() {
                None => Outcome::MissingHeader,
                Some(token) if tokens_match(token, expected.expose_secret()) => Outcome::Valid,
                Some(_) => Outcome::InvalidToken,
            },
            TokenAuth::PerTeam(tokens) => match provided() {
                None => Outcome::MissingHeader,
                Some(token) => {
                    let team = team_from_query(req.query_string());
                    match tokens.check(token, team.as_deref()) {
                        TeamCheck::Valid => Outcome::Valid,
                        TeamCheck::OtherTeam => Outcome::WrongTeam,
                        TeamCheck::UnknownToken => Outcome::InvalidToken,
                    }
                }
            },
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
        Outcome::WrongTeam => {
            tracing::warn!("Rejected request with a TURBO_TOKEN of another team");
            Ok(req.into_response(HttpResponse::Forbidden().json(AuthError {
                error: "TURBO_TOKEN is not for this team",
            })))
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

/// Compares a token against the configured one in constant time.
/// A plain `==` stops at the first byte that differs, so its running time
/// tells an attacker how much of a guess was correct.
///
/// The token length is not treated as a secret: `ct_eq` returns early when
/// the lengths differ, which is the usual trade for this kind of check.
fn tokens_match(provided: &str, expected: &str) -> bool {
    provided.as_bytes().ct_eq(expected.as_bytes()).into()
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

    #[test]
    fn matches_an_identical_token() {
        assert!(tokens_match("secret", "secret"));
    }

    #[test]
    fn rejects_a_token_that_differs() {
        assert!(!tokens_match("secret", "other!"));
    }

    #[test]
    fn rejects_a_token_that_only_shares_a_prefix() {
        assert!(!tokens_match("secret", "secret-and-more"));
        assert!(!tokens_match("secret-and-more", "secret"));
    }

    #[test]
    fn rejects_a_token_that_differs_in_case() {
        assert!(!tokens_match("secret", "SECRET"));
    }

    #[test]
    fn rejects_an_empty_token_against_a_real_one() {
        assert!(!tokens_match("", "secret"));
    }

    #[test]
    fn matches_a_token_with_multibyte_characters() {
        assert!(tokens_match("sécret-🔑", "sécret-🔑"));
        assert!(!tokens_match("sécret-🔑", "sécret-🔒"));
    }
}
