use std::env;
use std::fmt;
use std::str::FromStr;

/// Server-side encryption algorithms supported by AWS S3.
/// See: https://docs.aws.amazon.com/AmazonS3/latest/API/API_PutObject.html
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum S3ServerSideEncryption {
    Aes256,
    AwsFsx,
    AwsKms,
    AwsKmsDsse,
}

impl S3ServerSideEncryption {
    /// Returns the string value expected by the S3 API
    pub fn as_str(&self) -> &'static str {
        match self {
            Self::Aes256 => "AES256",
            Self::AwsFsx => "aws:fsx",
            Self::AwsKms => "aws:kms",
            Self::AwsKmsDsse => "aws:kms:dsse",
        }
    }
}

impl AsRef<str> for S3ServerSideEncryption {
    fn as_ref(&self) -> &str {
        self.as_str()
    }
}

impl fmt::Display for S3ServerSideEncryption {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}", self.as_str())
    }
}

impl FromStr for S3ServerSideEncryption {
    type Err = String;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        match s {
            "AES256" => Ok(Self::Aes256),
            "aws:fsx" => Ok(Self::AwsFsx),
            "aws:kms" => Ok(Self::AwsKms),
            "aws:kms:dsse" => Ok(Self::AwsKmsDsse),
            _ => Err(format!(
                "Invalid S3 server-side encryption value: '{}'. \
                 Valid values are: AES256, aws:fsx, aws:kms, aws:kms:dsse",
                s
            )),
        }
    }
}

#[derive(Clone)]
pub struct AppSettings {
    /// Host where to bind the server to
    /// Defaults to the loopback address
    pub host: String,
    pub port: u16,
    /// The maximum size allowed for payloads
    /// uploaded by Turborepo. Defaults to 100MB.
    pub max_payload_size_in_bytes: usize,
    pub s3_access_key: Option<String>,
    pub s3_secret_key: Option<String>,
    pub s3_endpoint: Option<String>,
    /// if your S3-compatible store does not support requests
    /// like https://bucket.hostname.domain/. Setting `s3_use_path_style`
    /// to true configures the S3 client to make requests like
    /// https://hostname.domain/bucket instead.
    pub s3_use_path_style: bool,
    pub s3_region: String,
    pub s3_bucket_name: String,
    /// The server-side encryption algorithm to use for the S3 bucket.
    pub s3_server_side_encryption: Option<S3ServerSideEncryption>,
    pub turbo_token: Option<String>,
}

pub fn get_settings() -> AppSettings {
    let port = env::var("PORT")
        .unwrap_or("8000".to_string())
        .parse::<u16>()
        .expect("Could not read PORT from env");

    let host = env::var("HOST").unwrap_or("127.0.0.1".to_owned());

    let s3_access_key = env::var("S3_ACCESS_KEY").ok();
    let s3_secret_key = env::var("S3_SECRET_KEY").ok();
    let s3_region = env::var("S3_REGION").unwrap_or("eu-central-1".to_owned());
    let s3_endpoint = env::var("S3_ENDPOINT").ok();
    let s3_use_path_style = env::var("S3_USE_PATH_STYLE")
        .map(|v| v == "true" || v == "1")
        .unwrap_or(false);
    let s3_server_side_encryption = env::var("S3_SERVER_SIDE_ENCRYPTION").ok().map(|v| {
        v.parse::<S3ServerSideEncryption>()
            .expect("Invalid S3_SERVER_SIDE_ENCRYPTION value")
    });

    // by default,we scope Turborepo artifacts using the "TURBO_TEAM" name sent by turborepo
    // which creates a folder within the S3 bucket and uploads everything under that.
    let s3_bucket_name = env::var("S3_BUCKET_NAME").unwrap_or("turbo".to_owned());

    let payload_in_mb = env::var("MAX_PAYLOAD_SIZE_IN_MB").unwrap_or("100".to_string());

    let max_payload_size_in_bytes = payload_in_mb
        .parse::<usize>()
        .map(|size_in_mb| size_in_mb * 1024 * 1024)
        .unwrap_or_else(|_| {
            panic!("Invalid value given for MAX_PAYLOAD_SIZE_IN_MB: \"{payload_in_mb}\"",)
        });

    let turbo_token = env::var("TURBO_TOKEN").ok();

    AppSettings {
        host,
        port,
        max_payload_size_in_bytes,
        s3_access_key,
        s3_secret_key,
        s3_region,
        s3_endpoint,
        s3_bucket_name,
        s3_use_path_style,
        s3_server_side_encryption,
        turbo_token,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parse_aes256_encryption() {
        let result = "AES256".parse::<S3ServerSideEncryption>();
        assert_eq!(result, Ok(S3ServerSideEncryption::Aes256));
    }

    #[test]
    fn parse_aws_fsx_encryption() {
        let result = "aws:fsx".parse::<S3ServerSideEncryption>();
        assert_eq!(result, Ok(S3ServerSideEncryption::AwsFsx));
    }

    #[test]
    fn parse_aws_kms_encryption() {
        let result = "aws:kms".parse::<S3ServerSideEncryption>();
        assert_eq!(result, Ok(S3ServerSideEncryption::AwsKms));
    }

    #[test]
    fn parse_aws_kms_dsse_encryption() {
        let result = "aws:kms:dsse".parse::<S3ServerSideEncryption>();
        assert_eq!(result, Ok(S3ServerSideEncryption::AwsKmsDsse));
    }

    #[test]
    fn parse_invalid_encryption_returns_error() {
        let result = "invalid".parse::<S3ServerSideEncryption>();
        assert!(result.is_err());
        assert!(
            result
                .unwrap_err()
                .contains("Invalid S3 server-side encryption value")
        );
    }

    #[test]
    fn encryption_to_string_aes256() {
        assert_eq!(S3ServerSideEncryption::Aes256.to_string(), "AES256");
    }

    #[test]
    fn encryption_to_string_aws_fsx() {
        assert_eq!(S3ServerSideEncryption::AwsFsx.to_string(), "aws:fsx");
    }

    #[test]
    fn encryption_to_string_aws_kms() {
        assert_eq!(S3ServerSideEncryption::AwsKms.to_string(), "aws:kms");
    }

    #[test]
    fn encryption_to_string_aws_kms_dsse() {
        assert_eq!(
            S3ServerSideEncryption::AwsKmsDsse.to_string(),
            "aws:kms:dsse"
        );
    }

    #[test]
    fn encryption_as_ref_str() {
        let encryption = S3ServerSideEncryption::Aes256;
        let s: &str = encryption.as_ref();
        assert_eq!(s, "AES256");
    }
}
