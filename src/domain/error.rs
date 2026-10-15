use std::fmt;

#[derive(Debug)]
pub enum CacheError {
    /// The store answered, but holds no artifact under that path.
    NotFound,
    /// The store could not be reached, or rejected the request.
    StoreUnavailable(Box<dyn std::error::Error + Send + Sync>),
}

impl fmt::Display for CacheError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::NotFound => write!(f, "no such artifact in the store"),
            Self::StoreUnavailable(error) => write!(f, "store request failed: {error}"),
        }
    }
}

impl std::error::Error for CacheError {
    fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
        match self {
            Self::NotFound => None,
            Self::StoreUnavailable(error) => Some(&**error),
        }
    }
}
