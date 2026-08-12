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

/// Builds the S3 configuration shared by the plain client and the transfer
/// manager.
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
