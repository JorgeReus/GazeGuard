use std::fs;
use std::path::{Path, PathBuf};

pub const CONFIG_FILE_NAME: &str = "config.yaml";

pub fn ensure_config_file(path: &Path, default_yaml: &str) -> Result<PathBuf, String> {
    if let Some(parent) = path.parent() {
        fs::create_dir_all(parent).map_err(|error| error.to_string())?;
    }

    if !path.exists() {
        fs::write(path, default_yaml).map_err(|error| error.to_string())?;
    }

    Ok(path.to_path_buf())
}

#[cfg(desktop)]
pub fn desktop_config_path() -> Result<PathBuf, String> {
    #[cfg(target_os = "windows")]
    {
        let app_data = std::env::var_os("APPDATA").ok_or("APPDATA is not set")?;
        return Ok(PathBuf::from(app_data)
            .join("GazeGuard")
            .join(CONFIG_FILE_NAME));
    }

    #[cfg(any(target_os = "macos", target_os = "linux"))]
    {
        let home = std::env::var_os("HOME").ok_or("HOME is not set")?;
        return Ok(desktop_config_path_from_home(Path::new(&home)));
    }

    #[allow(unreachable_code)]
    Err("unsupported desktop platform".to_string())
}

#[cfg(any(target_os = "macos", target_os = "linux"))]
pub fn desktop_config_path_from_home(home: &Path) -> PathBuf {
    home.join(".config")
        .join("GazeGuard")
        .join(CONFIG_FILE_NAME)
}

#[cfg(test)]
mod tests {
    use super::{desktop_config_path_from_home, ensure_config_file};
    use std::fs;
    use std::path::{Path, PathBuf};
    use std::time::{SystemTime, UNIX_EPOCH};

    struct TestDir {
        path: PathBuf,
    }

    impl TestDir {
        fn path(&self) -> &Path {
            self.path.as_path()
        }
    }

    impl Drop for TestDir {
        fn drop(&mut self) {
            let _ = fs::remove_dir_all(&self.path);
        }
    }

    fn unique_test_dir(name: &str) -> TestDir {
        let suffix = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap()
            .as_nanos();
        let path = std::env::temp_dir().join(format!("gazeguard-config-file-{name}-{suffix}"));
        fs::create_dir_all(&path).unwrap();
        TestDir { path }
    }

    #[test]
    fn ensure_config_file_writes_defaults_when_missing() {
        let temp = unique_test_dir("writes-defaults");
        let config_path = temp.path().join(".config/GazeGuard/config.yaml");

        let resolved = ensure_config_file(&config_path, "short_break_interval: 1\n").unwrap();

        assert_eq!(resolved, config_path);
        assert_eq!(
            fs::read_to_string(&config_path).unwrap(),
            "short_break_interval: 1\n"
        );
    }

    #[test]
    fn ensure_config_file_keeps_existing_contents() {
        let temp = unique_test_dir("keeps-existing");
        let config_path = temp.path().join(".config/GazeGuard/config.yaml");
        fs::create_dir_all(config_path.parent().unwrap()).unwrap();
        fs::write(&config_path, "short_break_interval: 99\n").unwrap();

        ensure_config_file(&config_path, "short_break_interval: 1\n").unwrap();

        assert_eq!(
            fs::read_to_string(&config_path).unwrap(),
            "short_break_interval: 99\n"
        );
    }

    #[cfg(any(target_os = "macos", target_os = "linux"))]
    #[test]
    fn desktop_config_path_uses_dot_config_directory() {
        let path = desktop_config_path_from_home(Path::new("/tmp/test-home"));

        assert_eq!(
            path,
            PathBuf::from("/tmp/test-home/.config/GazeGuard/config.yaml")
        );
    }
}
