use std::{fs, path::PathBuf};

use anyhow::{Context, Result};
use directories::ProjectDirs;
use serde::Deserialize;

#[derive(Clone, Debug, Default, Deserialize)]
pub struct Config {
    #[serde(default)]
    pub database: Option<DatabaseSource>,
    #[serde(default)]
    pub rss: RssSection,
    #[serde(default)]
    pub ical: IcalSection,
    #[serde(default)]
    pub import: ImportSection,
    // legacy flat keys
    pub rss_output: Option<PathBuf>,
    pub ical_output: Option<PathBuf>,
    pub import_source: Option<PathBuf>,
}

#[derive(Clone, Debug, Default, Deserialize)]
pub struct RssSection {
    pub output: Option<PathBuf>,
}

#[derive(Clone, Debug, Default, Deserialize)]
pub struct IcalSection {
    pub output: Option<PathBuf>,
}

#[derive(Clone, Debug, Default, Deserialize)]
pub struct ImportSection {
    pub source: Option<PathBuf>,
}

#[derive(Clone, Debug, Deserialize)]
#[serde(untagged)]
pub enum DatabaseSource {
    Path(PathBuf),
    Section(DatabaseSection),
}

#[derive(Clone, Debug, Default, Deserialize)]
pub struct DatabaseSection {
    pub path: Option<PathBuf>,
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

impl Config {
    pub fn database_path(&self) -> Option<PathBuf> {
        self.database.as_ref().and_then(|source| match source {
            DatabaseSource::Path(path) => Some(path.clone()),
            DatabaseSource::Section(section) => section.path.clone(),
        })
    }

    pub fn rss_output_path(&self) -> Option<PathBuf> {
        self.rss.output.clone().or_else(|| self.rss_output.clone())
    }

    pub fn ical_output_path(&self) -> Option<PathBuf> {
        self.ical
            .output
            .clone()
            .or_else(|| self.ical_output.clone())
    }

    pub fn import_source_path(&self) -> Option<PathBuf> {
        self.import
            .source
            .clone()
            .or_else(|| self.import_source.clone())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::{env, fs, sync::Mutex};
    use tempfile::tempdir;

    static ENV_MUTEX: Mutex<()> = Mutex::new(());

    struct EnvOverride {
        key: &'static str,
        original: Option<std::ffi::OsString>,
    }

    impl EnvOverride {
        fn set_path(key: &'static str, value: &std::path::Path) -> Self {
            let original = env::var_os(key);
            // SAFETY: test serially overrides env vars and restores them before drop.
            unsafe { env::set_var(key, value) };
            Self { key, original }
        }
    }

    impl Drop for EnvOverride {
        fn drop(&mut self) {
            if let Some(val) = &self.original {
                // SAFETY: restoring captured env var for test isolation.
                unsafe { env::set_var(self.key, val) };
            } else {
                // SAFETY: removing env var to restore prior unset state.
                unsafe { env::remove_var(self.key) };
            }
        }
    }

    #[test]
    fn load_config_reads_structured_sections() {
        let _lock = ENV_MUTEX.lock().unwrap();
        let home = tempdir().unwrap();
        let _guard = EnvOverride::set_path("XDG_CONFIG_HOME", home.path());
        let dirs = ProjectDirs::from("dev", "toki-note", "toki-note").expect("project dirs");
        let path = dirs.config_dir().join("config.toml");
        fs::create_dir_all(path.parent().unwrap()).unwrap();
        fs::write(
            &path,
            r#"
            database = "/tmp/custom.db"
            rss_output = "/tmp/legacy.xml"

            [rss]
            output = "/tmp/rss.xml"

            [ical]
            output = "/tmp/ical.ics"

            [import]
            source = "/tmp/import.ics"
            "#,
        )
        .unwrap();

        let loaded: Config = toml::from_str(&fs::read_to_string(&path).unwrap()).unwrap();
        assert!(loaded.database.is_some());

        let cfg = load_config().unwrap();
        assert_eq!(
            cfg.database_path().as_deref(),
            Some(std::path::Path::new("/tmp/custom.db"))
        );
        assert_eq!(
            cfg.rss_output_path().as_deref(),
            Some(std::path::Path::new("/tmp/rss.xml"))
        );
        assert_eq!(
            cfg.ical_output_path().as_deref(),
            Some(std::path::Path::new("/tmp/ical.ics"))
        );
        assert_eq!(
            cfg.import_source_path().as_deref(),
            Some(std::path::Path::new("/tmp/import.ics"))
        );
    }

    #[test]
    fn load_config_defaults_when_missing() {
        let _lock = ENV_MUTEX.lock().unwrap();
        let home = tempdir().unwrap();
        let _guard = EnvOverride::set_path("XDG_CONFIG_HOME", home.path());
        let cfg = load_config().unwrap();
        assert!(cfg.database_path().is_none());
        assert!(cfg.rss_output_path().is_none());
        assert!(cfg.ical_output_path().is_none());
        assert!(cfg.import_source_path().is_none());
    }

    #[test]
    fn resolve_database_uses_provided_path() {
        let custom = PathBuf::from("/tmp/db.sqlite");
        let path = resolve_database_path(Some(custom.clone())).unwrap();
        assert_eq!(path, custom);
    }

    #[test]
    fn resolve_database_respects_xdg_data_home() {
        let _lock = ENV_MUTEX.lock().unwrap();
        let data = tempdir().unwrap();
        let _guard = EnvOverride::set_path("XDG_DATA_HOME", data.path());
        let expected = ProjectDirs::from("dev", "toki-note", "toki-note")
            .expect("dirs")
            .data_dir()
            .join("toki-note.db");
        let resolved = resolve_database_path(None).unwrap();
        assert_eq!(resolved, expected);
    }

    #[test]
    fn database_section_path_is_used_when_present() {
        let _lock = ENV_MUTEX.lock().unwrap();
        let home = tempdir().unwrap();
        let _guard = EnvOverride::set_path("XDG_CONFIG_HOME", home.path());
        let dirs = ProjectDirs::from("dev", "toki-note", "toki-note").expect("project dirs");
        let path = dirs.config_dir().join("config.toml");
        fs::create_dir_all(path.parent().unwrap()).unwrap();
        fs::write(
            &path,
            r#"
            [database]
            path = "/tmp/from-section.db"
            "#,
        )
        .unwrap();

        let cfg = load_config().unwrap();
        assert_eq!(
            cfg.database_path().as_deref(),
            Some(std::path::Path::new("/tmp/from-section.db"))
        );
    }
}
