//! Registration of the file types SphereView can open.
//!
//! Two files have to agree for the desktop to open an image in SphereView by
//! double-clicking it: the MIME type database, which says what a file is, and
//! the desktop entry, which says the application handles that type. Getting them
//! out of step is silent - nothing breaks, the file just opens in the wrong
//! application - so both are checked here.

use sphereview::image::SUPPORTED_FILE_TYPES;
use std::fs;
use std::path::PathBuf;

/// The type SphereView claims for Insta360 photos.
///
/// It has one of its own because the marker that identifies these files (the
/// camera's trailer) sits at the end of the file, where shared-mime-info cannot
/// look, so the extension is what the type is matched on.
const INSP_MIME_TYPE: &str = "image/x-insta360-insp";

const MIME_FILE: &str = "io.github.dynobo.sphereview.mime.xml";
const DESKTOP_FILE: &str = "io.github.dynobo.sphereview.desktop";

fn project_file(name: &str) -> String {
    let path = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join(name);
    fs::read_to_string(&path).unwrap_or_else(|e| panic!("cannot read {}: {e}", path.display()))
}

/// The MIME types the desktop entry says SphereView opens.
fn desktop_mime_types() -> Vec<String> {
    project_file(DESKTOP_FILE)
        .lines()
        .find_map(|line| line.strip_prefix("MimeType="))
        .unwrap_or_default()
        .split(';')
        .filter(|mime_type| !mime_type.is_empty())
        .map(str::to_string)
        .collect()
}

/// Every MIME type the database declares, with the file globs that match it.
fn declared_mime_types() -> Vec<(String, Vec<String>)> {
    let contents = project_file(MIME_FILE);
    let document = roxmltree::Document::parse(&contents)
        .unwrap_or_else(|e| panic!("{MIME_FILE} should be valid XML: {e}"));

    document
        .descendants()
        .filter(|node| node.has_tag_name("mime-type"))
        .filter_map(|node| {
            let mime_type = node.attribute("type")?.to_string();
            let globs = node
                .children()
                .filter(|child| child.has_tag_name("glob"))
                .filter_map(|child| child.attribute("pattern").map(str::to_string))
                .collect();
            Some((mime_type, globs))
        })
        .collect()
}

#[test]
fn insta360_photos_have_a_type_of_their_own() {
    let declared = declared_mime_types();
    let (_, globs) = declared
        .iter()
        .find(|(mime_type, _)| mime_type == INSP_MIME_TYPE)
        .unwrap_or_else(|| panic!("{MIME_FILE} should declare {INSP_MIME_TYPE}"));

    assert!(
        globs.iter().any(|glob| glob.eq_ignore_ascii_case("*.insp")),
        "{MIME_FILE} should match *.insp, found {globs:?}"
    );
}

#[test]
fn the_desktop_entry_offers_the_insta360_type() {
    assert!(
        desktop_mime_types()
            .iter()
            .any(|mime_type| mime_type == INSP_MIME_TYPE),
        "{DESKTOP_FILE} should list {INSP_MIME_TYPE} in MimeType"
    );
}

#[test]
fn every_supported_extension_is_registered() {
    let desktop = desktop_mime_types();
    let declared = declared_mime_types();

    for (extension, mime_type) in SUPPORTED_FILE_TYPES {
        // `.insp` is the odd one: the image itself is a JPEG, which is what the
        // viewer is handed, but the file claims a type of its own.
        let expected = if *extension == "insp" {
            INSP_MIME_TYPE
        } else {
            mime_type
        };

        assert!(
            desktop.iter().any(|declared| declared == expected),
            ".{extension} opens in SphereView, but {DESKTOP_FILE} does not list {expected}"
        );

        if expected == INSP_MIME_TYPE {
            assert!(
                declared.iter().any(|(declared, _)| declared == expected),
                ".{extension} has its own type, but {MIME_FILE} does not declare {expected}"
            );
        }
    }
}
