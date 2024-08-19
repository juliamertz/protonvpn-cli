use anyhow::Result;
use std::{
    fmt::Display,
    path::PathBuf,
    str::FromStr,
    time::{Duration, SystemTime},
};

use crate::config;

pub fn get_path() -> PathBuf {
    let cache_dir = PathBuf::from_str("/etc").unwrap();
    cache_dir.join("protonvpn-rs")
}

pub trait CachedObject {
    fn filename() -> &'static str;
}

pub fn write<T>(value: &T) -> Result<()>
where
    T: Display + CachedObject,
{
    let path = get_path().join(T::filename());
    std::fs::create_dir_all(get_path())?;
    std::fs::write(path, value.to_string())?;
    Ok(())
}

/// returns `None` if the file doesn't exist and when the files last modified date is older than `config.max_cache_age`
pub fn read<T>() -> Option<T>
where
    T: Sized + CachedObject + TryFrom<String>,
    <T as std::convert::TryFrom<std::string::String>>::Error: std::fmt::Debug,
{
    let path = get_path().join(T::filename());
    let config = config::read().expect("config to be initialized");

    if let Ok(metadata) = std::fs::metadata(&path) {
        let modified_at = metadata.modified().unwrap();
        let sys_time = SystemTime::now();
        let difference = sys_time
            .duration_since(modified_at)
            .expect("a time difference");

        if difference > Duration::from_secs(config.max_cache_age * 60 * 60 * 24) {
            return None;
        }
    }

    if let Ok(content) = std::fs::read(path) {
        let string = String::from_utf8(content).unwrap();
        return Some(string.try_into().unwrap());
    }

    None
}

pub fn delete<T>() -> Result<()>
where
    T: Display + CachedObject,
{
    let path = get_path().join(T::filename());
    std::fs::remove_file(path)?;
    Ok(())
}

pub fn file_path<T>() -> PathBuf
where
    T: Display + CachedObject,
{
    get_path().join(T::filename())
}
