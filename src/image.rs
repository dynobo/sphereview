use gtk4::gio;
use log::error;
use std::fs;
use std::path::PathBuf;

pub const SUPPORTED_FILE_TYPES: &[(&str, &str)] = &[
    ("jpg", "image/jpeg"),
    ("jpeg", "image/jpeg"),
    ("png", "image/png"),
    ("webp", "image/webp"),
];

pub struct ImageData {
    pub filename: String,
    pub data: Vec<u8>,
    pub mime_type: String,
}

pub fn from_file(path: &PathBuf) -> ImageData {
    if !path.exists() {
        error!("File {:?} does not exist", path);
        return from_demo();
    }

    let name = path
        .file_name()
        .and_then(|n| n.to_str())
        .unwrap_or("")
        .to_string();

    let mime_type = match detect_mime_type(path) {
        Some(m) => m.to_string(),
        None => {
            error!("Could not detect mime type for {:?}", path);
            return from_demo();
        }
    };

    match fs::read(path) {
        Ok(data) => ImageData {
            filename: name,
            data,
            mime_type,
        },
        Err(e) => {
            error!("Could not read file {:?}: {}", path, e);
            from_demo()
        }
    }
}

pub fn from_demo() -> ImageData {
    gio::resources_lookup_data(
        "/io/github/dynobo/sphereview/assets/demo.webp",
        gio::ResourceLookupFlags::NONE,
    )
    .map(|bytes| ImageData {
        filename: "demo.webp".to_string(),
        data: bytes.to_vec(),
        mime_type: "image/webp".to_string(),
    })
    .expect("demo.webp should always be available in GIO resources")
}

pub fn detect_mime_type<P: AsRef<std::path::Path>>(path: P) -> Option<&'static str> {
    path.as_ref()
        .extension()
        .and_then(std::ffi::OsStr::to_str)
        .and_then(|ext| {
            SUPPORTED_FILE_TYPES
                .iter()
                .find(|(e, _)| e.eq_ignore_ascii_case(ext))
                .map(|(_, mt)| *mt)
        })
}
