use gtk::{gio, glib, prelude::*};
use serde::{Deserialize, Serialize};
use std::collections::HashSet;

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct Entry {
    pub uri: String,
    pub name: String,
    pub content_type: String,
    pub size: Option<u64>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub text: Option<String>,
    #[serde(skip)]
    pub missing: bool,
}

pub const MAX_TEXT_BYTES: usize = 1024 * 1024;

impl Entry {
    pub fn from_text(text: String) -> Result<Self, String> {
        if text.len() > MAX_TEXT_BYTES {
            return Err("Dropped text must be 1 MiB or smaller.".into());
        }
        if text.trim().is_empty() || text.contains('\0') {
            return Err("The drop contains no usable text.".into());
        }
        let name: String = text
            .trim()
            .lines()
            .next()
            .unwrap_or_default()
            .chars()
            .filter(|ch| !ch.is_control())
            .take(100)
            .collect();
        let hash = glib::compute_checksum_for_data(glib::ChecksumType::Sha256, text.as_bytes())
            .expect("SHA-256 is supported");
        Ok(Self {
            uri: format!("shelfdrop-text:{hash}"),
            name: if name.is_empty() {
                "Plain text".into()
            } else {
                name
            },
            content_type: "text/plain".into(),
            size: Some(text.len() as u64),
            text: Some(text),
            missing: false,
        })
    }

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
        text: None,
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
            parts.push("1 item added".into());
        } else if count > 1 {
            parts.push(format!("{count} items added"));
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
    fn text_entries_preserve_content_and_deduplicate() {
        let text = "  Halo dunia ü\nbaris kedua\n";
        let first = Entry::from_text(text.into()).unwrap();
        assert_eq!(first.name, "Halo dunia ü");
        assert_eq!(first.text.as_deref(), Some(text));
        let mut shelf = Shelf::default();
        let added = shelf.add([Ok(first), Ok(Entry::from_text(text.into()).unwrap())], 2);
        assert_eq!(added.duplicates, 1);
        assert_eq!(shelf.entries.len(), 1);
        assert!(Entry::from_text(" \n".into()).is_err());
        assert!(Entry::from_text("a\0b".into()).is_err());
        assert!(Entry::from_text("a".repeat(MAX_TEXT_BYTES + 1)).is_err());
    }

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
