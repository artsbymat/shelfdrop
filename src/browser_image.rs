use gtk::{gio, glib, prelude::*};
use std::{
    fs,
    io::Read,
    path::{Path, PathBuf},
    time::Duration,
};

pub const MAX_BYTES: usize = 16 * 1024 * 1024;
const TOO_LARGE: &str = "Browser images must be 16 MiB or smaller.";
const UNSUPPORTED: &str = "The browser did not provide a supported image.";

pub enum BrowserImage {
    Embedded(ImageBytes),
    Url(String),
}

pub struct ImageBytes {
    bytes: Vec<u8>,
    filename: String,
}

impl BrowserImage {
    pub fn from_uri(uri: String) -> Option<Self> {
        let scheme = glib::Uri::peek_scheme(&uri)?;
        matches!(scheme.as_str(), "http" | "https" | "data").then_some(Self::Url(uri))
    }

    pub async fn save(self) -> Result<gio::File, String> {
        let cache = glib::user_cache_dir().join("shelfdrop/browser-images");
        gio::spawn_blocking(move || {
            let image = match self {
                Self::Embedded(image) => image,
                Self::Url(url) => fetch(&url)?,
            };
            image.save_in(&cache)
        })
        .await
        .map_err(|_| "Could not import the browser image.".to_string())?
        .map(gio::File::for_path)
    }
}

impl ImageBytes {
    pub fn new(bytes: Vec<u8>, name: Option<&str>) -> Result<Self, String> {
        if bytes.len() > MAX_BYTES {
            return Err(TOO_LARGE.into());
        }
        let (kind, _) = gio::content_type_guess(None::<&Path>, bytes.as_slice());
        let extension = match kind.as_str() {
            "image/png" => "png",
            "image/jpeg" => "jpg",
            "image/webp" => "webp",
            "image/gif" => "gif",
            "image/avif" => "avif",
            "image/bmp" | "image/x-ms-bmp" => "bmp",
            "image/tiff" => "tiff",
            "image/x-icon" | "image/vnd.microsoft.icon" => "ico",
            _ => return Err(UNSUPPORTED.into()),
        };
        let stem: String = name
            .and_then(|name| name.rsplit(['/', '\\']).next())
            .and_then(|name| Path::new(name).file_stem())
            .and_then(|stem| stem.to_str())
            .unwrap_or_default()
            .chars()
            .filter(|ch| !ch.is_control())
            .take(48)
            .collect();
        let stem = stem.trim_matches(['.', ' ', '"']);
        let stem = if stem.is_empty() {
            "Browser image"
        } else {
            stem
        };
        Ok(Self {
            bytes,
            filename: format!("{stem}.{extension}"),
        })
    }

    fn save_in(self, cache: &Path) -> Result<PathBuf, String> {
        let digest = glib::compute_checksum_for_data(glib::ChecksumType::Sha256, &self.bytes)
            .ok_or_else(|| "Could not identify the browser image.".to_string())?;
        let dir = cache.join(digest.as_str());
        fs::create_dir_all(&dir).map_err(|_| "Could not create the image cache.")?;
        let path = dir.join(self.filename);
        gio::File::for_path(&path)
            .replace_contents(
                &self.bytes,
                None,
                false,
                gio::FileCreateFlags::PRIVATE | gio::FileCreateFlags::REPLACE_DESTINATION,
                gio::Cancellable::NONE,
            )
            .map_err(|_| "Could not save the image to the cache.")?;
        Ok(path)
    }
}

fn fetch(url: &str) -> Result<ImageBytes, String> {
    if let Some(data) = url.strip_prefix("data:") {
        let (header, payload) = data.split_once(',').ok_or(UNSUPPORTED)?;
        if !header.starts_with("image/") || !header.ends_with(";base64") || payload.contains('\0') {
            return Err(UNSUPPORTED.into());
        }
        return ImageBytes::new(glib::base64_decode(payload), None);
    }

    let uri = glib::Uri::parse(url, glib::UriFlags::NONE).map_err(|_| UNSUPPORTED)?;
    if !matches!(uri.scheme().as_str(), "http" | "https")
        || uri.host().is_none_or(|host| host.is_empty())
        || uri.userinfo().is_some()
    {
        return Err(UNSUPPORTED.into());
    }

    let agent = ureq::AgentBuilder::new()
        .redirects(4)
        .timeout(Duration::from_secs(15))
        .build();
    let response = agent
        .get(url)
        .set("Accept", "image/*")
        .call()
        .map_err(|_| "Could not download the browser image.")?;
    if !(200..300).contains(&response.status()) {
        return Err("Could not download the browser image.".into());
    }
    if response
        .header("Content-Length")
        .and_then(|size| size.parse::<usize>().ok())
        .is_some_and(|size| size > MAX_BYTES)
    {
        return Err(TOO_LARGE.into());
    }
    let mut bytes = Vec::new();
    response
        .into_reader()
        .take(MAX_BYTES as u64 + 1)
        .read_to_end(&mut bytes)
        .map_err(|_| "Could not read the browser image.")?;
    ImageBytes::new(bytes, Some(uri.path().as_str()))
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::os::unix::fs::PermissionsExt;

    #[test]
    fn imports_raster_bytes_into_a_stable_private_cache_path() {
        let dir =
            std::env::temp_dir().join(format!("shelfdrop-browser-{}", glib::uuid_string_random()));
        let png = glib::base64_decode(
            "iVBORw0KGgoAAAANSUhEUgAAAAEAAAABCAQAAAC1HAwCAAAAC0lEQVR42mP8/x8AAwMCAO+/X2kAAAAASUVORK5CYII=",
        );
        let a = ImageBytes::new(png.clone(), Some("../../gambar.svg"))
            .unwrap()
            .save_in(&dir)
            .unwrap();
        let b = ImageBytes::new(png.clone(), Some("gambar.png"))
            .unwrap()
            .save_in(&dir)
            .unwrap();
        assert_eq!(a, b);
        assert_eq!(a.file_name().unwrap(), "gambar.png");
        assert_eq!(fs::read(&a).unwrap(), png);
        assert_eq!(fs::metadata(&a).unwrap().permissions().mode() & 0o077, 0);
        let as_url = format!("data:image/png;base64,{}", glib::base64_encode(&png));
        assert_eq!(fetch(&as_url).unwrap().bytes, png);
        assert!(ImageBytes::new(b"<html>not an image</html>".to_vec(), Some("fake.png")).is_err());
        assert!(ImageBytes::new(vec![0; MAX_BYTES + 1], None).is_err());
        assert!(fetch("https://user:secret@example.test/private.png").is_err());
        assert!(BrowserImage::from_uri("ftp://example.test/example.png".into()).is_none());
        fs::remove_dir_all(dir).unwrap();
    }
}
