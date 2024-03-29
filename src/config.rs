use serde::Deserialize;
use url::Url;

#[derive(Debug, Clone, Deserialize)]
pub struct Config {
    pub gitlab: GitlabConfig,
    #[serde(alias = "repository")]
    pub repositories: Vec<Repository>,
}

#[derive(Debug, Clone, Deserialize)]
pub struct GitlabConfig {
    pub url: Url,
    pub token: String,
}

#[derive(Debug, Clone, Deserialize)]
pub struct Repository {
    pub name: String,
    #[serde(default)]
    pub labels: Vec<String>,
}

impl Config {
    pub async fn read() -> color_eyre::Result<Self> {
        let config = tokio::fs::read_to_string("config.toml").await?;
        let config: Config = toml::from_str(&config)?;

        Ok(config)
    }
}
