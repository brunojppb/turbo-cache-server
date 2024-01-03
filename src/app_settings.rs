use serde_aux::field_attributes::deserialize_number_from_string;
#[derive(Clone, serde::Deserialize)]
pub struct AppSettings {
    #[serde(deserialize_with = "deserialize_number_from_string")]
    pub port: u16,
}

pub fn get_settings() -> Result<AppSettings, config::ConfigError> {
    let settings = config::Config::builder()
        .add_source(config::File::new("config.yaml", config::FileFormat::Yaml))
        .add_source(
            // This allows us to overwrite any of our config values
            // using environment variables starting with APP_
            // For instance, to overwrite the port number, we can pas
            // in the following variable:
            // APP__PORT=3000
            config::Environment::with_prefix("APP")
                .prefix_separator("_")
                .separator("__"),
        )
        .build()?;

    settings.try_deserialize::<AppSettings>()
}
