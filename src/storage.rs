use crate::shelf::{Entry, local_file};
use gtk::{gio, glib, prelude::*};
use serde::{Deserialize, Serialize, de::DeserializeOwned};
use std::{
    fs, io,
    path::{Path, PathBuf},
};

#[derive(Clone, Copy, Debug, Default, Deserialize, Eq, PartialEq, Serialize)]
pub enum Theme {
    System,
    #[default]
    Dark,
    Light,
}

#[derive(Clone, Copy, Debug, Default, Deserialize, Eq, PartialEq, Serialize)]
pub enum Mode {
    #[default]
    Compact,
    Expanded,
}

#[derive(Clone, Copy, Debug, Deserialize, Serialize)]
#[serde(default)]
pub struct Preferences {
    pub theme: Theme,
    pub mode: Mode,
    pub grid: bool,
    pub width: i32,
    pub height: i32,
    pub max_items: usize,
    pub opacity: i32,
    pub clear_on_exit: bool,
    pub sort_by_name: bool,
}

impl Default for Preferences {
    fn default() -> Self {
        Self {
            theme: Theme::Dark,
            mode: Mode::Compact,
            grid: false,
            width: 412,
            height: 322,
            max_items: 24,
            opacity: 100,
            clear_on_exit: true,
            sort_by_name: false,
        }
    }
}

pub struct Storage {
    settings: PathBuf,
    entries: PathBuf,
}

impl Storage {
    pub fn new() -> Self {
        Self {
            settings: glib::user_config_dir().join("shelfdrop/preferences.json"),
            entries: std::env::var_os("XDG_STATE_HOME")
                .map(PathBuf::from)
                .filter(|path| path.is_absolute())
                .unwrap_or_else(|| glib::home_dir().join(".local/state"))
                .join("shelfdrop/entries.json"),
        }
    }

    pub fn load(&self) -> (Preferences, Vec<Entry>, String) {
        let mut errors = Vec::new();
        let mut settings: Preferences = read(&self.settings, &mut errors).unwrap_or_default();
        settings.width = settings.width.clamp(300, 1800);
        settings.height = settings.height.clamp(220, 1200);
        settings.max_items = settings.max_items.clamp(1, 999);
        settings.opacity = settings.opacity.clamp(85, 100);
        let mut seen = std::collections::HashSet::new();
        let entries = if settings.clear_on_exit {
            if let Err(error) = remove_state(&self.entries) {
                errors.push(format!("Could not clear saved shelf ({error})"));
            }
            Vec::new()
        } else {
            let restored: Vec<Entry> = read(&self.entries, &mut errors).unwrap_or_default();
            restored
                .into_iter()
                .filter(|entry| {
                    !entry.uri.contains('\0')
                        && !entry.name.contains('\0')
                        && !entry.content_type.contains('\0')
                        && seen.insert(entry.uri.clone())
                        && local_file(&entry.file())
                })
                .collect()
        };
        (settings, entries, errors.join(" · "))
    }

    pub fn save_settings(&self, settings: &Preferences) -> Result<(), String> {
        write(&self.settings, settings)
    }

    pub fn save_entries(&self, clear_on_exit: bool, entries: &[Entry]) -> Result<(), String> {
        if clear_on_exit {
            remove_state(&self.entries)
        } else {
            write(&self.entries, entries)
        }
    }
}

fn read<T: DeserializeOwned>(path: &Path, errors: &mut Vec<String>) -> Option<T> {
    match fs::read(path) {
        Ok(bytes) => match serde_json::from_slice(&bytes) {
            Ok(value) => Some(value),
            Err(error) => {
                errors.push(format!("Could not read {} ({error})", path.display()));
                None
            }
        },
        Err(error) if error.kind() == io::ErrorKind::NotFound => None,
        Err(error) => {
            errors.push(format!("Could not read {} ({error})", path.display()));
            None
        }
    }
}

fn write(path: &Path, value: &(impl Serialize + ?Sized)) -> Result<(), String> {
    if let Some(dir) = path.parent() {
        fs::create_dir_all(dir).map_err(|error| error.to_string())?;
    }
    let bytes = serde_json::to_vec(value).map_err(|error| error.to_string())?;
    gio::File::for_path(path)
        .replace_contents(
            &bytes,
            None,
            false,
            gio::FileCreateFlags::PRIVATE | gio::FileCreateFlags::REPLACE_DESTINATION,
            gio::Cancellable::NONE,
        )
        .map(|_| ())
        .map_err(|error| error.to_string())
}

fn remove_state(path: &Path) -> Result<(), String> {
    match fs::remove_file(path) {
        Ok(()) => Ok(()),
        Err(error) if error.kind() == io::ErrorKind::NotFound => Ok(()),
        Err(error) => Err(error.to_string()),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn restores_entries_and_discards_state_on_clear() {
        let dir =
            std::env::temp_dir().join(format!("shelfdrop-test-{}", glib::uuid_string_random()));
        fs::create_dir(&dir).unwrap();
        let storage = Storage {
            settings: dir.join("prefs.json"),
            entries: dir.join("shelf.json"),
        };
        let settings = Preferences {
            clear_on_exit: false,
            ..Preferences::default()
        };
        storage.save_settings(&settings).unwrap();
        storage
            .save_entries(
                false,
                &[Entry {
                    uri: "file:///tmp/missing.pdf".into(),
                    name: "missing.pdf".into(),
                    content_type: "application/pdf".into(),
                    size: Some(12),
                    missing: true,
                }],
            )
            .unwrap();
        let (loaded, entries, status) = storage.load();
        assert!(status.is_empty() && !loaded.clear_on_exit);
        assert_eq!(entries[0].name, "missing.pdf");
        fs::write(&storage.entries, "{broken").unwrap();
        let (_, entries, status) = storage.load();
        assert!(entries.is_empty() && !status.is_empty());
        storage.save_entries(true, &[]).unwrap();
        assert!(!storage.entries.exists() && storage.settings.exists());
        fs::remove_dir_all(dir).unwrap();
    }
}
