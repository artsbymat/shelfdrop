use gtk::{gio, prelude::*};
use serde::{Deserialize, Serialize};
use std::collections::HashSet;

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct Entry {
    pub uri: String,
    pub name: String,
    pub content_type: String,
    pub size: Option<u64>,
    #[serde(skip)]
    pub missing: bool,
}

impl Entry {
    pub fn file(&self) -> gio::File {
        gio::File::for_uri(&self.uri)
    }
}

#[derive(Clone, Copy, Debug)]
pub enum Rejection {
    Unsupported,
    Unavailable,
}

pub fn local_file(file: &gio::File) -> bool {
    file.is_native() && file.uri_scheme().as_deref() == Some("file")
}

pub async fn inspect(file: &gio::File) -> Result<Entry, Rejection> {
    if !local_file(file) {
        return Err(Rejection::Unsupported);
    }
    let info = file
        .query_info_future(
            "standard::type,standard::display-name,standard::content-type,standard::size",
            gio::FileQueryInfoFlags::NONE,
            gtk::glib::Priority::DEFAULT,
        )
        .await
        .map_err(|_| Rejection::Unavailable)?;
    if info.file_type() != gio::FileType::Regular {
        return Err(Rejection::Unsupported);
    }
    Ok(Entry {
        uri: file.uri().to_string(),
        name: info.display_name().to_string(),
        content_type: info.content_type().map_or_else(
            || "application/octet-stream".into(),
            |mime| mime.to_string(),
        ),
        size: info
            .has_attribute("standard::size")
            .then(|| u64::try_from(info.size()).ok())
            .flatten(),
        missing: false,
    })
}

#[derive(Default, Debug)]
pub struct Added {
    pub uris: Vec<String>,
    pub duplicates: usize,
    pub unavailable: usize,
    pub unsupported: usize,
    pub full: usize,
}

impl Added {
    pub fn message(&self) -> String {
        let mut parts = Vec::new();
        let count = self.uris.len();
        if count == 1 {
            parts.push("1 file added".into());
        } else if count > 1 {
            parts.push(format!("{count} files added"));
        }
        for (count, reason) in [
            (self.duplicates, "already on shelf"),
            (self.unavailable, "unavailable"),
            (self.unsupported, "unsupported"),
            (self.full, "over the item limit"),
        ] {
            if count > 0 {
                parts.push(format!("{count} {reason}"));
            }
        }
        parts.join(" · ")
    }
}

#[derive(Default)]
pub struct Shelf {
    pub entries: Vec<Entry>,
}

impl Shelf {
    pub fn add(
        &mut self,
        entries: impl IntoIterator<Item = Result<Entry, Rejection>>,
        max: usize,
    ) -> Added {
        let mut result = Added::default();
        let mut seen: HashSet<_> = self.entries.iter().map(|entry| entry.uri.clone()).collect();
        let mut added = Vec::new();
        for entry in entries {
            match entry {
                Ok(entry) if !seen.insert(entry.uri.clone()) => result.duplicates += 1,
                Ok(_) if self.entries.len() + added.len() >= max => result.full += 1,
                Ok(entry) => {
                    result.uris.push(entry.uri.clone());
                    added.push(entry);
                }
                Err(Rejection::Unavailable) => result.unavailable += 1,
                Err(Rejection::Unsupported) => result.unsupported += 1,
            }
        }
        self.entries.splice(..0, added);
        result
    }

    pub fn remove(&mut self, uris: &[String]) {
        self.entries.retain(|entry| !uris.contains(&entry.uri));
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::fs;

    #[test]
    fn mixed_batch_preserves_files_and_handles_limits_and_duplicates() {
        let dir = std::env::temp_dir().join(format!(
            "shelfdrop-test-{}",
            gtk::glib::uuid_string_random()
        ));
        fs::create_dir(&dir).unwrap();
        let first = dir.join("catatan ü.txt");
        let second = dir.join("README");
        fs::write(&first, "hello").unwrap();
        fs::write(&second, "world").unwrap();
        let context = gtk::glib::MainContext::new();
        let (first_entry, second_entry) = context
            .with_thread_default(|| {
                context.block_on(async {
                    let first = inspect(&gio::File::for_path(&first)).await.unwrap();
                    let second = inspect(&gio::File::for_path(&second)).await.unwrap();
                    assert!(matches!(
                        inspect(&gio::File::for_path(dir.join("missing"))).await,
                        Err(Rejection::Unavailable)
                    ));
                    assert!(matches!(
                        inspect(&gio::File::for_path(&dir)).await,
                        Err(Rejection::Unsupported)
                    ));
                    assert!(matches!(
                        inspect(&gio::File::for_uri("sftp://example.org/file")).await,
                        Err(Rejection::Unsupported)
                    ));
                    (first, second)
                })
            })
            .unwrap();
        assert_eq!(first_entry.name, "catatan ü.txt");
        assert_eq!(first_entry.size, Some(5));
        assert_eq!(second_entry.name, "README");
        let mut shelf = Shelf::default();
        let batch = shelf.add(
            [
                Ok(first_entry.clone()),
                Err(Rejection::Unavailable),
                Ok(first_entry.clone()),
                Ok(second_entry.clone()),
                Err(Rejection::Unsupported),
            ],
            1,
        );
        assert_eq!(
            (
                batch.duplicates,
                batch.unavailable,
                batch.unsupported,
                batch.full
            ),
            (1, 1, 1, 1)
        );
        assert_eq!(shelf.add([Ok(second_entry.clone())], 0).full, 1);
        shelf.remove(&[first_entry.uri]);
        assert_eq!(shelf.add([Ok(second_entry)], 1).uris.len(), 1);
        shelf.entries.clear();
        assert_eq!(fs::read_to_string(&first).unwrap(), "hello");
        assert_eq!(fs::read_to_string(&second).unwrap(), "world");
        fs::remove_dir_all(dir).unwrap();
    }
}
