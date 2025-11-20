use std::{fs, path::PathBuf};

use anyhow::{Context, Result};
use directories::ProjectDirs;
use serde::Deserialize;

#[derive(Clone, Debug, Default, Deserialize)]
pub struct Config {
    pub database: Option<PathBuf>,
    pub rss_output: Option<PathBuf>,
    pub ical_output: Option<PathBuf>,
    pub import_source: Option<PathBuf>,
}

pub fn load_config() -> Result<Config> {
    if let Some(project_dirs) = ProjectDirs::from("dev", "toki-note", "toki-note") {
        let path = project_dirs.config_dir().join("config.toml");
        if path.exists() {
            let contents = fs::read_to_string(&path)
                .with_context(|| format!("failed to read config {}", path.display()))?;
            let cfg: Config = toml::from_str(&contents)
                .with_context(|| format!("failed to parse {}", path.display()))?;
            return Ok(cfg);
        }
    }
    Ok(Config::default())
}

pub fn resolve_database_path(input: Option<PathBuf>) -> Result<PathBuf> {
    if let Some(path) = input {
        return Ok(path);
    }

    if let Some(project_dirs) = ProjectDirs::from("dev", "toki-note", "toki-note") {
        let mut path = project_dirs.data_dir().to_path_buf();
        path.push("toki-note.db");
        Ok(path)
    } else {
        Ok(PathBuf::from("toki-note.db"))
    }
}
