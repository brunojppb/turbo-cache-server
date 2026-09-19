use aws_credential_types::provider::{ProvideCredentials, future};
use aws_sdk_s3::config::{
    BehaviorVersion, Credentials, Region, RequestChecksumCalculation, ResponseChecksumValidation,
};
use secrecy::ExposeSecret;
use tokio::sync::OnceCell;

use crate::app_settings::{AppSettings, S3ChecksumMode};

/// Builds the default credential chain on first use, so client construction
/// stays synchronous and the server starts even when instance metadata is
/// briefly unreachable.
#[derive(Debug)]
struct LazyDefaultCredentials {
    region: Region,
    chain: OnceCell<aws_config::default_provider::credentials::DefaultCredentialsChain>,
}

impl ProvideCredentials for LazyDefaultCredentials {
    fn provide_credentials<'a>(&'a self) -> future::ProvideCredentials<'a>
    where
        Self: 'a,
    {
        future::ProvideCredentials::new(async move {
            self.chain
                .get_or_init(|| async {
                    aws_config::default_provider::credentials::DefaultCredentialsChain::builder()
                        .region(self.region.clone())
                        .build()
                        .await
                })
                .await
                .provide_credentials()
                .await
        })
    }
}

/// Builds the shared S3 configuration.
pub(crate) fn build_config(settings: &AppSettings) -> aws_sdk_s3::config::Builder {
    let region = Region::new(settings.s3_region.clone());

    let (request_checksums, response_checksums) = match settings.s3_checksum_mode {
        S3ChecksumMode::WhenRequired => (
            RequestChecksumCalculation::WhenRequired,
            ResponseChecksumValidation::WhenRequired,
        ),
        S3ChecksumMode::WhenSupported => (
            RequestChecksumCalculation::WhenSupported,
            ResponseChecksumValidation::WhenSupported,
        ),
    };

    let mut config = aws_sdk_s3::Config::builder()
        .behavior_version(BehaviorVersion::latest())
        .retry_config(aws_sdk_s3::config::retry::RetryConfig::standard().with_max_attempts(3))
        .timeout_config(
            aws_sdk_s3::config::timeout::TimeoutConfig::builder()
                // Three attempts of 60 s, plus backoff, fit inside the operation
                // timeout, so a slow store never loses an attempt.
                .operation_timeout(std::time::Duration::from_secs(240))
                .operation_attempt_timeout(std::time::Duration::from_secs(60))
                .build(),
        )
        .region(region.clone())
        .force_path_style(settings.s3_use_path_style)
        .request_checksum_calculation(request_checksums)
        .response_checksum_validation(response_checksums);

    if let Some(endpoint) = &settings.s3_endpoint {
        config = config.endpoint_url(endpoint);
    }

    match (&settings.s3_access_key, &settings.s3_secret_key) {
        (Some(access_key), Some(secret_key)) => config.credentials_provider(Credentials::new(
            access_key.expose_secret(),
            secret_key.expose_secret(),
            None,
            None,
            "turbo-cache-server",
        )),
        // Credentials handled by IAM: instance metadata, IRSA, or a profile.
        _ => config.credentials_provider(LazyDefaultCredentials {
            region,
            chain: OnceCell::new(),
        }),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::app_settings::{AppSettings, S3ChecksumMode};

    fn settings() -> AppSettings {
        AppSettings {
            host: "127.0.0.1".to_owned(),
            port: 8000,
            s3_access_key: Some("access".into()),
            s3_secret_key: Some("secret".into()),
            s3_endpoint: Some("http://127.0.0.1:1".to_owned()),
            s3_use_path_style: true,
            s3_region: "eu-central-1".to_owned(),
            s3_bucket_name: "turbo".to_owned(),
            s3_server_side_encryption: None,
            s3_checksum_mode: S3ChecksumMode::WhenRequired,
            turbo_token: None,
        }
    }

    #[test]
    fn when_required_is_the_default_checksum_behavior() {
        let config = build_config(&settings()).build();

        assert_eq!(
            config.request_checksum_calculation(),
            Some(&RequestChecksumCalculation::WhenRequired)
        );
        assert_eq!(
            config.response_checksum_validation(),
            Some(&ResponseChecksumValidation::WhenRequired)
        );
    }

    #[test]
    fn when_supported_opts_into_checksums() {
        let mut settings = settings();
        settings.s3_checksum_mode = S3ChecksumMode::WhenSupported;

        let config = build_config(&settings).build();

        assert_eq!(
            config.request_checksum_calculation(),
            Some(&RequestChecksumCalculation::WhenSupported)
        );
    }

    /// Every retry must fit inside the operation timeout, or the last attempt
    /// dies before the store can answer.
    #[test]
    fn retries_fit_inside_the_operation_timeout() {
        let config = build_config(&settings()).build();

        let timeouts = config.timeout_config().expect("no timeout config");
        let retries = config.retry_config().expect("no retry config");
        let attempt = timeouts
            .operation_attempt_timeout()
            .expect("no attempt timeout");
        let operation = timeouts.operation_timeout().expect("no operation timeout");

        assert_eq!(attempt, std::time::Duration::from_secs(60));
        assert_eq!(operation, std::time::Duration::from_secs(240));
        assert!(
            operation >= attempt * retries.max_attempts(),
            "{} attempts of {attempt:?} do not fit inside {operation:?}",
            retries.max_attempts()
        );
    }

    /// A hand-built config must resolve an HTTP client and a sleep impl from the
    /// enabled features, or the SDK panics on the first request instead of
    /// returning an error.
    #[tokio::test]
    async fn a_hand_built_config_can_send_a_request() {
        let client = aws_sdk_s3::Client::from_conf(build_config(&settings()).build());

        // Port 1 refuses connections, so this exercises the request path.
        let result = client
            .head_object()
            .bucket("turbo")
            .key("missing")
            .send()
            .await;

        assert!(result.is_err(), "expected a dispatch error, not a panic");
    }

    #[test]
    fn missing_credentials_fall_back_to_the_default_chain() {
        let mut settings = settings();
        settings.s3_access_key = None;
        settings.s3_secret_key = None;

        // Building must not panic and must not block on credential lookup.
        let _ = build_config(&settings).build();
    }
}
