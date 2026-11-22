use crate::browser_image::{BrowserImage, ImageBytes, MAX_BYTES};
use gtk::{gdk, gio, glib, prelude::*};

pub fn provider(files: &[gio::File]) -> gdk::ContentProvider {
    gdk::ContentProvider::for_value(&gdk::FileList::from_array(files).to_value())
}

pub fn clipboard_provider(files: &[gio::File]) -> gdk::ContentProvider {
    // Thunar enables Paste only when the clipboard identifies the file operation.
    let uris: Vec<_> = files.iter().map(|file| file.uri()).collect();
    let paths = uris
        .iter()
        .map(|uri| uri.as_str())
        .collect::<Vec<_>>()
        .join("\n");
    let bytes = glib::Bytes::from_owned(format!("copy\n{paths}"));
    gdk::ContentProvider::new_union(&[
        provider(files),
        gdk::ContentProvider::for_bytes("x-special/gnome-copied-files", &bytes),
    ])
}

pub async fn open(file: &gio::File, context: &gio::AppLaunchContext) -> Result<(), glib::Error> {
    gio::AppInfo::launch_default_for_uri_future(&file.uri(), Some(context)).await
}

pub async fn reveal(file: &gio::File, context: &gio::AppLaunchContext) -> Result<(), glib::Error> {
    if let Ok(bus) = gio::bus_get_future(gio::BusType::Session).await {
        let interface = "org.freedesktop.FileManager1";
        let args = (vec![file.uri().to_string()], "").to_variant();
        let shown = bus
            .call_future(
                Some(interface),
                "/org/freedesktop/FileManager1",
                interface,
                "ShowItems",
                Some(&args),
                None,
                gio::DBusCallFlags::NONE,
                2500,
            )
            .await;
        if shown.is_ok() {
            return Ok(());
        }
    }
    open(&file.parent().unwrap_or_else(|| file.clone()), context).await
}

const URI_LIST: &str = "text/uri-list";
const FIREFOX_FILENAME: &str = "application/x-moz-file-promise-dest-filename";
const BROWSER_URLS: &[&str] = &[
    "application/x-moz-file-promise-url",
    "text/x-moz-url-data",
    "text/x-moz-url",
];

pub enum DropData {
    Files(Vec<gio::File>),
    Image(BrowserImage),
}

pub fn can_receive(formats: &gdk::ContentFormats) -> bool {
    formats.contains_type(gdk::FileList::static_type())
        || formats.contain_mime_type(URI_LIST)
        || BROWSER_URLS
            .iter()
            .any(|mime| formats.contain_mime_type(mime))
        || formats.mime_types().iter().any(|mime| image_offer(mime))
}

fn image_offer(mime: &str) -> bool {
    mime.starts_with("image/") || mime.starts_with("application/octet-stream;name=")
}

pub async fn receive(drop: &gdk::Drop) -> Result<DropData, String> {
    let formats = drop.formats();
    let image_format = formats
        .mime_types()
        .into_iter()
        .find(|mime| image_offer(mime));
    let mut last_error = None;
    if let Some(mime) = image_format {
        let name = if let Some(name) = mime.strip_prefix("application/octet-stream;name=") {
            Some(name.trim_matches('"').to_owned())
        } else if formats.contain_mime_type(FIREFOX_FILENAME) {
            read_bytes(drop, FIREFOX_FILENAME, 4096)
                .await
                .ok()
                .and_then(|bytes| browser_text(&bytes))
        } else {
            None
        };
        match read_bytes(drop, &mime, MAX_BYTES)
            .await
            .and_then(|bytes| ImageBytes::new(bytes, name.as_deref()))
        {
            Ok(image) => return Ok(DropData::Image(BrowserImage::Embedded(image))),
            Err(error) => last_error = Some(error),
        }
    }

    for mime in BROWSER_URLS
        .iter()
        .filter(|mime| formats.contain_mime_type(mime))
    {
        if let Ok(bytes) = read_bytes(drop, mime, 8 * 1024 * 1024).await
            && let Some(url) = browser_text(&bytes)
            && let Some(image) = BrowserImage::from_uri(url)
        {
            return Ok(DropData::Image(image));
        }
    }

    if let Ok(value) = drop
        .read_value_future(gdk::FileList::static_type(), glib::Priority::DEFAULT)
        .await
        && let Ok(list) = value.get::<gdk::FileList>()
        && !list.files().is_empty()
    {
        return Ok(classify(list.files()));
    }

    if formats.contain_mime_type(URI_LIST) {
        let bytes = read_bytes(drop, URI_LIST, 8 * 1024 * 1024).await?;
        let list = std::str::from_utf8(&bytes).map_err(|_| "The file list is invalid.")?;
        if list.contains('\0') {
            return Err("The file list is invalid.".into());
        }
        let files = glib::Uri::list_extract_uris(list)
            .iter()
            .map(|uri| gio::File::for_uri(uri))
            .collect();
        return Ok(classify(files));
    }

    Err(last_error.unwrap_or_else(|| "The drop contains no files or browser images.".into()))
}

fn classify(files: Vec<gio::File>) -> DropData {
    if let [file] = files.as_slice()
        && let Some(image) = BrowserImage::from_uri(file.uri().to_string())
    {
        return DropData::Image(image);
    }
    DropData::Files(files)
}

fn browser_text(bytes: &[u8]) -> Option<String> {
    let text = if bytes.contains(&0)
        || bytes.starts_with(&[0xff, 0xfe])
        || bytes.starts_with(&[0xfe, 0xff])
    {
        let (units, remainder) = bytes.as_chunks::<2>();
        if !remainder.is_empty() {
            return None;
        }
        let decode = if bytes.starts_with(&[0xfe, 0xff]) {
            u16::from_be_bytes
        } else {
            u16::from_le_bytes
        };
        let utf16: Vec<_> = units.iter().copied().map(decode).collect();
        String::from_utf16(&utf16).ok()?
    } else {
        std::str::from_utf8(bytes).ok()?.to_owned()
    };
    let first_line = text
        .trim_start_matches('\u{feff}')
        .lines()
        .next()?
        .trim()
        .trim_end_matches('\0');
    (!first_line.is_empty() && !first_line.contains('\0')).then(|| first_line.to_owned())
}

async fn read_bytes(drop: &gdk::Drop, mime: &str, limit: usize) -> Result<Vec<u8>, String> {
    let priority = glib::Priority::DEFAULT;
    let (stream, _) = drop
        .read_future(&[mime], priority)
        .await
        .map_err(|_| "Could not read the drop.")?;
    let mut bytes = Vec::new();
    loop {
        let part = stream
            .read_bytes_future(8192, priority)
            .await
            .map_err(|_| "Could not read the drop.")?;
        if part.is_empty() {
            return Ok(bytes);
        }
        if bytes.len() + part.len() > limit {
            return Err("The drop was too large.".into());
        }
        bytes.extend_from_slice(&part);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn reads_browser_url_without_the_title_in_utf8_or_utf16() {
        let url = "https://example.test/picture.png";
        let offer = format!("{url}\r\nImage title");
        assert_eq!(browser_text(offer.as_bytes()).as_deref(), Some(url));
        let utf16: Vec<_> = offer.encode_utf16().flat_map(u16::to_le_bytes).collect();
        assert_eq!(browser_text(&utf16).as_deref(), Some(url));
        assert!(image_offer("application/octet-stream;name=\"image.png\""));
        assert!(!image_offer("text/html"));
    }
}
