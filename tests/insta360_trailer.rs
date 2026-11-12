//! Tests for the Insta360 trailer reader.
//!
//! Two kinds of fixture are used here:
//!
//! * **Synthetic** files, built byte by byte below. They carry no third-party
//!   content and cover the parser's structure: record walk, padding, truncation.
//! * **One real calibration string**, taken from a real `.insp` file. The
//!   calibration is a fact read out of the file, not creative content, so it is
//!   reproduced here as a regression vector. Its provenance:
//!
//!   Provenance of the real vector:
//!
//!   ```text
//!   file:    IMG_20180318_205851_239.insp (5,352,736 bytes, JPEG 6912x3456)
//!   source:  https://github.com/hakanson/Insta360-images-20180318
//!   sha256:  b7d62cf2220be2e49e9869b7bbf15549561d2bd3542a0b70435cc47d0b4b105c
//!   trailer: version 3, 260,604 bytes, records 0x0300 (41552), 0x0200 (218696),
//!            0x0101 (266); field 2 = "Insta360 ONE", field 3 = "v1.16.7.1_build2"
//!   ```
//!
//! The image itself is not committed: that repository has no licence, so the
//! bytes stay local. Tests that need it are gated behind an environment variable:
//!
//! ```sh
//! SPHEREVIEW_TEST_INSP=/path/to/photo.insp cargo test --test insta360_trailer
//! ```
//!
//! Without it, that test prints a note and returns.
//!
//! # The committed fixture
//!
//! `tests/fixtures/synthetic.insp` is generated, not captured, so it belongs to
//! this project and needs no camera:
//!
//! ```sh
//! python3 tools/make_synthetic_insp.py tests/fixtures/synthetic.insp \
//!     --equirect tests/fixtures/synthetic-equirect.jpg
//! ```
//!
//! It is a 2048x1024 JPEG holding two fisheye circles, with a trailer in the
//! layout a camera writes and a calibration using the geometry of a real file
//! (radius, off-centre circles, the 90 and 94.116 degree lens rolls).
//! `synthetic-equirect.jpg` is the view it was projected from, for comparing the
//! rendered sphere against by eye. It proves the wiring - file, trailer,
//! parameters, projection - but not a camera's conventions, because the pair is
//! generated with the same mapping the shader uses.

use sphereview::insta360;

/// Calibration string from the real 2018 Insta360 ONE file (fields below).
const REAL_CALIBRATION: &str = "2_1636.891_1707.637_1748.886_0.000_0.000_90.000_\
1653.479_5198.825_1730.561_0.544_-1.614_94.116_6912_3456_1293";

// ---------------------------------------------------------------------------
// Synthetic .insp construction
// ---------------------------------------------------------------------------

fn varint(mut value: u64) -> Vec<u8> {
    let mut out = Vec::new();
    loop {
        let byte = (value & 0x7f) as u8;
        value >>= 7;
        if value == 0 {
            out.push(byte);
            break;
        }
        out.push(byte | 0x80);
    }
    out
}

fn field_bytes(field: u32, payload: &[u8]) -> Vec<u8> {
    let mut out = varint(u64::from((field << 3) | 2));
    out.extend(varint(payload.len() as u64));
    out.extend_from_slice(payload);
    out
}

fn field_varint(field: u32, value: u64) -> Vec<u8> {
    let mut out = varint(u64::from(field << 3));
    out.extend(varint(value));
    out
}

/// One trailer record: the payload, then its `[u16 id][u32 size]` footer.
fn record(id: u16, payload: &[u8]) -> Vec<u8> {
    let mut out = payload.to_vec();
    out.extend_from_slice(&id.to_le_bytes());
    out.extend_from_slice(&(payload.len() as u32).to_le_bytes());
    out
}

/// A metadata record (id 0x0101) holding the given protobuf fields.
fn metadata_record(
    model: Option<&str>,
    calibration: Option<&str>,
    size: Option<(u32, u32)>,
) -> Vec<u8> {
    let mut payload = Vec::new();
    if let Some(model) = model {
        payload.extend(field_bytes(2, model.as_bytes()));
    }
    if let Some(calibration) = calibration {
        payload.extend(field_bytes(5, calibration.as_bytes()));
    }
    if let Some((width, height)) = size {
        let mut dims = field_varint(1, u64::from(width));
        dims.extend(field_varint(2, u64::from(height)));
        payload.extend(field_bytes(19, &dims));
    }
    record(0x0101, &payload)
}

/// Assembles a whole file: a JPEG-like body, the records, `pad` zero bytes, then
/// the 40-byte footer (`trailer_size`, `version`, 32-byte magic).
fn build(records: &[Vec<u8>], pad: usize, version: u32) -> Vec<u8> {
    let mut file = vec![0xFF, 0xD8, 0xFF, 0xDB, 0x00, 0x43, 0x00, 0xFF, 0xD9]; // JPEG-ish body
    let trailer_start = file.len();

    for record in records {
        file.extend_from_slice(record);
    }
    file.extend(std::iter::repeat_n(0u8, pad));

    let trailer_size = (file.len() - trailer_start + 40) as u32;
    file.extend_from_slice(&trailer_size.to_le_bytes());
    file.extend_from_slice(&version.to_le_bytes());
    file.extend_from_slice(insta360::MAGIC);

    file
}

fn synthetic(calibration: Option<&str>, model: Option<&str>) -> Vec<u8> {
    build(
        &[
            record(0x0300, &[0xAB; 40]),               // opaque: gyro data
            record(0x0200, &[0xFF, 0xD8, 0xFF, 0xD9]), // opaque: preview
            metadata_record(model, calibration, Some((6912, 3456))),
        ],
        32,
        3,
    )
}

// ---------------------------------------------------------------------------
// The real calibration string
// ---------------------------------------------------------------------------

#[test]
fn real_calibration_parses_into_two_lenses() {
    let calibration =
        insta360::parse_calibration(REAL_CALIBRATION).expect("real vector should parse");

    assert_eq!(calibration.lens_count, 2);
    assert_eq!((calibration.width, calibration.height), (6912, 3456));
    assert_eq!(calibration.lens.len(), 2);

    let lens0 = calibration.lens[0];
    assert!(
        (lens0.radius - 1636.891).abs() < 0.01,
        "lens 0 radius: {lens0:?}"
    );
    assert!(
        (lens0.center[0] - 1707.637).abs() < 0.01,
        "lens 0 cx: {lens0:?}"
    );
    assert!(
        (lens0.center[1] - 1748.886).abs() < 0.01,
        "lens 0 cy: {lens0:?}"
    );
    assert!(
        (lens0.rotation[0] - 0.0).abs() < 0.01,
        "lens 0 rx: {lens0:?}"
    );
    assert!(
        (lens0.rotation[2] - 90.0).abs() < 0.01,
        "lens 0 rz: {lens0:?}"
    );

    let lens1 = calibration.lens[1];
    assert!(
        (lens1.radius - 1653.479).abs() < 0.01,
        "lens 1 radius: {lens1:?}"
    );
    assert!(
        (lens1.center[0] - 5198.825).abs() < 0.01,
        "lens 1 cx: {lens1:?}"
    );
    assert!(
        (lens1.center[1] - 1730.561).abs() < 0.01,
        "lens 1 cy: {lens1:?}"
    );
    assert!(
        (lens1.rotation[1] - -1.614).abs() < 0.01,
        "lens 1 ry: {lens1:?}"
    );
    assert!(
        (lens1.rotation[2] - 94.116).abs() < 0.01,
        "lens 1 rz: {lens1:?}"
    );
}

/// The two lenses are mounted a few degrees apart; that offset is what the
/// shader needs to hide the seam. It is the difference of the two z rotations.
#[test]
fn real_calibration_yields_relative_lens_yaw() {
    let calibration =
        insta360::parse_calibration(REAL_CALIBRATION).expect("real vector should parse");
    let yaw = calibration.relative_yaw_degrees().expect("two lenses");

    assert!((yaw - 4.116).abs() < 0.01, "relative yaw was {yaw}");
}

#[test]
fn calibration_with_too_few_fields_is_rejected() {
    assert!(insta360::parse_calibration("2_1636.891_1707.637_1748.886_0.000").is_none());
    assert!(insta360::parse_calibration("").is_none());
    assert!(insta360::parse_calibration("garbage").is_none());
}

#[test]
fn calibration_with_non_numeric_field_is_rejected() {
    let broken = REAL_CALIBRATION.replace("1636.891", "1636,891");
    assert!(insta360::parse_calibration(&broken).is_none());
}

#[test]
fn calibration_tolerates_unknown_trailing_fields() {
    let extended = format!("{REAL_CALIBRATION}_42_7");
    let calibration = insta360::parse_calibration(&extended).expect("trailing fields are ignored");
    assert_eq!(calibration.lens.len(), 2);
}

#[test]
fn calibration_rejects_a_lens_count_that_does_not_match() {
    // Claims three lenses but carries the values for two.
    let wrong = REAL_CALIBRATION.replacen('2', "3", 1);
    assert!(insta360::parse_calibration(&wrong).is_none());
}

// ---------------------------------------------------------------------------
// The trailer itself
// ---------------------------------------------------------------------------

#[test]
fn synthetic_file_yields_model_and_calibration() {
    let file = synthetic(Some(REAL_CALIBRATION), Some("Insta360 ONE"));
    let trailer = insta360::read(&file).expect("synthetic trailer should be found");

    assert_eq!(trailer.version, 3);
    assert_eq!(trailer.model.as_deref(), Some("Insta360 ONE"));
    let calibration = trailer.calibration.expect("calibration should be present");
    assert_eq!((calibration.width, calibration.height), (6912, 3456));
}

/// The padding between the last record and the footer reads as five records of
/// id 0 and length 0. A walk that trusts them either loops or misses the start,
/// so they have to be swallowed.
#[test]
fn padding_of_zero_records_is_swallowed() {
    // The records and the footer stay the same; only the unknown pad width changes.
    const OLD_PAD: usize = 32;
    const FOOTER: usize = 40;

    for pad in [0usize, 2, 8, 16, 32, 64] {
        let original = synthetic(Some(REAL_CALIBRATION), Some("Insta360 ONE"));
        let trailer_size = u32::from_le_bytes(
            original[original.len() - FOOTER..original.len() - FOOTER + 4]
                .try_into()
                .unwrap(),
        ) as usize;
        let trailer_start = original.len() - trailer_size;
        let records_len = trailer_size - OLD_PAD - FOOTER;

        let mut file = original;
        file.truncate(trailer_start + records_len);
        file.extend(std::iter::repeat_n(0u8, pad));
        let size = (records_len + pad + FOOTER) as u32;
        file.extend_from_slice(&size.to_le_bytes());
        file.extend_from_slice(&3u32.to_le_bytes());
        file.extend_from_slice(insta360::MAGIC);

        let trailer = insta360::read(&file).unwrap_or_else(|| panic!("pad {pad} should parse"));
        assert_eq!(trailer.model.as_deref(), Some("Insta360 ONE"), "pad {pad}");
        assert_eq!(trailer.width, Some(6912), "pad {pad}");
    }
}

#[test]
fn file_without_magic_is_not_an_insta360_file() {
    assert!(insta360::read(b"not a panorama at all").is_none());

    let mut file = synthetic(Some(REAL_CALIBRATION), Some("Insta360 ONE"));
    let end = file.len();
    file[end - 1] ^= 0xFF; // corrupt the last magic byte
    assert!(insta360::read(&file).is_none());
}

#[test]
fn truncated_trailer_is_rejected() {
    let file = synthetic(Some(REAL_CALIBRATION), Some("Insta360 ONE"));
    // Keep the footer but drop the records it points at.
    let truncated = &file[file.len() - 78..];
    assert!(insta360::read(truncated).is_none());
}

#[test]
fn trailer_size_beyond_the_file_is_rejected() {
    let mut file = synthetic(Some(REAL_CALIBRATION), Some("Insta360 ONE"));
    let size_at = file.len() - 40;
    file[size_at..size_at + 4].copy_from_slice(&u32::MAX.to_le_bytes());
    assert!(insta360::read(&file).is_none());
}

#[test]
fn metadata_without_calibration_still_yields_the_model_and_size() {
    let file = synthetic(None, Some("Insta360 X3"));
    let trailer = insta360::read(&file).expect("trailer should be found");
    assert_eq!(trailer.model.as_deref(), Some("Insta360 X3"));
    assert_eq!((trailer.width, trailer.height), (Some(6912), Some(3456)));
    assert!(trailer.calibration.is_none());
}

#[test]
fn body_len_marks_where_the_jpeg_ends() {
    let file = synthetic(Some(REAL_CALIBRATION), Some("Insta360 ONE"));
    let trailer = insta360::read(&file).expect("trailer should be found");

    assert!(
        trailer.body_len < file.len(),
        "body must exclude the trailer"
    );
    assert_eq!(
        &file[trailer.body_len - 2..trailer.body_len],
        &[0xFF, 0xD9],
        "body should end with the JPEG end-of-image marker"
    );
    assert!(trailer.body_len >= 9, "body should keep the whole JPEG");
}

// ---------------------------------------------------------------------------
// What the webview receives
// ---------------------------------------------------------------------------

#[test]
fn params_json_carries_what_the_shader_needs() {
    let file = synthetic(Some(REAL_CALIBRATION), Some("Insta360 ONE"));
    let trailer = insta360::read(&file).expect("trailer should be found");
    let json = trailer
        .params_json()
        .expect("calibration should produce params");

    for expected in [
        "\"lens_count\":2",
        "\"radius\":1636.891",
        "\"cx\":1707.637",
        "\"cy\":1748.886",
        "\"rotation\":[0,0,90]",
        "\"width\":6912",
        "\"height\":3456",
        "\"fov_deg\":194",
        "\"yaw_deg\":4.116",
    ] {
        assert!(json.contains(expected), "missing {expected} in {json}");
    }
}

// ---------------------------------------------------------------------------
// A real camera file, when one is available
// ---------------------------------------------------------------------------

/// Opt-in: `.insp` originals are photographs, so none are committed here.
/// Point `SPHEREVIEW_TEST_INSP` at any `.insp` to run this; without it, it does
/// nothing.
///
/// A file known to work is `IMG_20180318_205851_239.insp` from
/// <https://github.com/hakanson/Insta360-images-20180318> (5,352,736 bytes, JPEG
/// 6912x3456, trailer v3, Insta360 ONE). That repository has no licence, so the
/// image stays local: download it, do not redistribute it.
#[test]
fn real_file_parses() {
    let Ok(path) = std::env::var("SPHEREVIEW_TEST_INSP") else {
        eprintln!("SPHEREVIEW_TEST_INSP unset, skipping the real-file test");
        return;
    };

    let bytes = std::fs::read(&path).unwrap_or_else(|e| panic!("cannot read {path}: {e}"));
    let trailer =
        insta360::read(&bytes).unwrap_or_else(|| panic!("{path} did not parse as an .insp"));

    let calibration = trailer
        .calibration
        .as_ref()
        .unwrap_or_else(|| panic!("{path} has no calibration"));
    assert!(calibration.lens.len() >= 2, "{path}: fewer than two lenses");
    assert!(
        calibration.width > 0 && calibration.height > 0,
        "{path}: no image size"
    );

    // The image stated by the metadata must match what the JPEG actually holds.
    let body_len = bytes
        .len()
        .saturating_sub(sizeof_trailer(&bytes).unwrap_or_default());
    let (width, height) =
        jpeg_size(&bytes[..body_len]).unwrap_or_else(|| panic!("{path}: no JPEG frame header"));
    assert_eq!(
        (u32::from(width), u32::from(height)),
        (calibration.width, calibration.height),
        "{path}: calibration disagrees with the JPEG"
    );

    eprintln!(
        "{path}: {} | {}x{} | lens radii {:?} | yaw {:.3}°",
        trailer.model.as_deref().unwrap_or("unknown model"),
        calibration.width,
        calibration.height,
        calibration
            .lens
            .iter()
            .map(|l| l.radius)
            .collect::<Vec<_>>(),
        calibration.relative_yaw_degrees().unwrap_or_default()
    );
}

/// Where the JPEG ends: the marker that precedes the trailer.
fn sizeof_trailer(bytes: &[u8]) -> Option<usize> {
    let magic_at = bytes.len().checked_sub(insta360::MAGIC.len())?;
    if !bytes.ends_with(insta360::MAGIC) {
        return None;
    }
    let size = u32::from_le_bytes(
        bytes[magic_at.checked_sub(8)?..magic_at - 4]
            .try_into()
            .ok()?,
    ) as usize;
    Some(size)
}

/// Width and height from the JPEG's start-of-frame marker.
fn jpeg_size(bytes: &[u8]) -> Option<(u16, u16)> {
    let mut cursor = 2;
    while cursor + 9 < bytes.len() {
        if bytes[cursor] != 0xFF {
            cursor += 1;
            continue;
        }
        match bytes[cursor + 1] {
            0xC0..=0xC3 => {
                let height = u16::from_be_bytes([bytes[cursor + 5], bytes[cursor + 6]]);
                let width = u16::from_be_bytes([bytes[cursor + 7], bytes[cursor + 8]]);
                return Some((width, height));
            }
            0xD0..=0xD9 => cursor += 2,
            0xFF => cursor += 1,
            _ => {
                let length =
                    usize::from(u16::from_be_bytes([bytes[cursor + 2], bytes[cursor + 3]]));
                cursor += 2 + length;
            }
        }
    }
    None
}

#[test]
fn params_json_is_absent_without_calibration() {
    let file = synthetic(None, Some("Insta360 X3"));
    let trailer = insta360::read(&file).expect("trailer should be found");
    assert!(trailer.params_json().is_none());
}

/// A full-resolution 72 MP photo has a body far larger than any window we would
/// scan, and the trailer can sit megabytes from the end if the camera appended
/// something after it. The scan must not assume the trailer starts at the end of
/// what it looked at.
#[test]
fn trailer_is_found_in_a_large_file() {
    let mut file = vec![0xFF, 0xD8];
    file.extend(std::iter::repeat_n(0x5A, 4 * 1024 * 1024));
    let body_end = file.len();

    let records = [
        record(0x0200, &[0xFF; 1024]),
        metadata_record(
            Some("Insta360 X5"),
            Some(REAL_CALIBRATION),
            Some((11904, 5952)),
        ),
    ];
    let mut trailer = Vec::new();
    for record in &records {
        trailer.extend_from_slice(record);
    }
    trailer.extend(std::iter::repeat_n(0u8, 32));
    let size = (trailer.len() + 40) as u32;
    trailer.extend_from_slice(&size.to_le_bytes());
    trailer.extend_from_slice(&3u32.to_le_bytes());
    trailer.extend_from_slice(insta360::MAGIC);
    file.extend_from_slice(&trailer);

    assert!(body_end > 4 * 1024 * 1024);
    let parsed = insta360::read(&file).expect("trailer should be found");
    assert_eq!(parsed.model.as_deref(), Some("Insta360 X5"));
    // The size comes from the metadata record; the calibration string keeps the
    // dimensions the lenses were calibrated at, which are not always the same.
    assert_eq!((parsed.width, parsed.height), (Some(11904), Some(5952)));
    assert_eq!(parsed.calibration.unwrap().width, 6912);
}
