use actix_web::{HttpResponse, ResponseError, http::StatusCode};

use crate::domain::CacheError;

/// The only place where `CacheError` meets HTTP.
impl ResponseError for CacheError {
    fn status_code(&self) -> StatusCode {
        match self {
            CacheError::NotFound => StatusCode::NOT_FOUND,
            CacheError::StoreUnavailable(_) => StatusCode::INTERNAL_SERVER_ERROR,
        }
    }

    fn error_response(&self) -> HttpResponse {
        if let CacheError::StoreUnavailable(error) = self {
            tracing::error!(error = %error, "Storage request failed");
        }

        // Status code with an empty body, matching the pre-refactor handlers.
        HttpResponse::new(self.status_code())
    }
}
