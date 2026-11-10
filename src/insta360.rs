//! Reader for the trailer an Insta360 camera appends to a `.insp` photo.
//!
//! A `.insp` is an ordinary JPEG — the two fisheye circles side by side — with a
//! block of the camera's own data bolted onto the end. Standard decoders ignore
//! it. Everything needed to project the two circles onto a sphere lives in that
//! block, in plain protobuf, without any of the vendor's code.
//!
//! Reading it backwards from the end of the file:
//!
//! ```text
//! ...record data...
//! [pad]                  unknown width, zero bytes on every file measured
//! uint32   trailer_size  total trailer length, this footer and magic included
//! uint32   version       3 on the files measured so far
//! char[32] MAGIC
//! ```
//!
//! Each record is data followed by its own six-byte footer, so the records are
//! walked backwards. The pad width is not announced: the reader tries every
//! plausible width and keeps the walk that lands exactly on the start of the
//! trailer. That check is the parser's correctness proof — a wrong width leaves
//! the walk somewhere other than the boundary — with one wrinkle: a pad that is
//! a multiple of six also reads as a run of zero-length records, so candidates
//! that swallow fewer of those win.
//!
//! Layout and field meanings: `golblick`'s `docs/formats/insta360.md` (facts
//! about the container, measured from camera files), plus the calibration string
//! verified against a real Insta360 ONE file in `tests/insta360_trailer.rs`.
//!
//! What the trailer does *not* carry: which way is up. Insta360's own app shows
//! every photo level, but the camera's attitude at capture is not in the file for
//! the models measured here (ONE RS, ONE X2). Searching a photo for it turns up:
//!
//!  - the records: `0x0101` metadata, `0x0200` a downscaled copy of the same two
//!    fisheye circles, `0x0900` and `0x0b00` empty. No gyro record.
//!  - the metadata's own fields: identity, model, firmware, the calibration, the
//!    image size, the DCIM path, and small rotations (doubles and near-identity
//!    quaternions) of a degree or two. Those are lens alignment refinements, not
//!    a camera attitude.
//!  - the JPEG's XMP: absent, so no `GPano:PosePitchDegrees`/`PoseRollDegrees`.
//!  - the EXIF MakerNote: empty.
//!
//! Both files measured were shot rolled, one by 180° and one by 90°, which rules
//! out a per-model convention as well. A viewer can only reproduce the frame the
//! sensors recorded; levelling it needs the IMU at capture, or a manual control.

/// Marks the end of the trailer, and so the end of the file.
pub const MAGIC: &[u8; 32] = b"8db42d694ccc418790edff439fe026bf";

/// `[u32 trailer_size][u32 version][char[32] magic]`
const FOOTER_LEN: usize = 40;
/// `[u16 id][u32 size]`, after each record's data
const RECORD_FOOTER_LEN: usize = 6;
/// Widest padding between the last record and the footer worth trying.
const MAX_PAD_LEN: usize = 64;
/// How far from the end of the file the trailer may end.
const MAGIC_SCAN_LEN: usize = 1024 * 1024;

/// What a lens sees across its fisheye circle. Not in the file: measured per
/// camera.
///
/// The rim of the circle is at *half* this angle from the lens axis, so this is a
/// field of view, not a maximum angle. 194° is the value measured for the OneR,
/// ONE RS, X2, X3 and X5. Other models are unmeasured, so this is a default
/// rather than a fact.
pub const DEFAULT_FOV_DEG: f32 = 194.0;

/// A record id holding the camera's model, firmware and calibration.
const METADATA_RECORD_ID: u16 = 0x0101;

#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Lens {
    /// Radius of the fisheye circle, in pixels of the image the file holds.
    pub radius: f32,
    /// Centre of that circle, in pixels, in the same coordinates.
    pub center: [f32; 2],
    /// Small rotations of this lens about its own axes, in degrees.
    pub rotation: [f32; 3],
}

#[derive(Debug, Clone, PartialEq)]
pub struct Calibration {
    pub lens_count: u32,
    pub lens: Vec<Lens>,
    pub width: u32,
    pub height: u32,
    /// The last value in the string. Meaning not established; kept for probes.
    pub trailing: Option<u32>,
}

impl Calibration {
    /// How far the two lenses are rotated against each other, in degrees.
    ///
    /// The lenses sit back to back, so what the shader needs is the offset
    /// between their z rotations, not their absolute values.
    pub fn relative_yaw_degrees(&self) -> Option<f32> {
        match self.lens.as_slice() {
            [first, second, ..] => Some(second.rotation[2] - first.rotation[2]),
            _ => None,
        }
    }
}

#[derive(Debug, Clone, PartialEq)]
pub struct Trailer {
    pub version: u32,
    pub model: Option<String>,
    pub calibration: Option<Calibration>,
    /// Image size as the file states it, independent of the calibration string.
    pub width: Option<u32>,
    pub height: Option<u32>,
    /// Length of the JPEG in front of the trailer, where the image ends.
    pub body_len: usize,
}

impl Trailer {
    /// The parameters the viewer's dual-fisheye projection needs, as JSON.
    ///
    /// `None` when the file carries no calibration: without it there is nothing
    /// to project with, and the caller should fall back to an equirectangular
    /// interpretation.
    pub fn params_json(&self) -> Option<String> {
        let calibration = self.calibration.as_ref()?;

        let lenses: Vec<String> = calibration
            .lens
            .iter()
            .map(|lens| {
                format!(
                    "{{\"radius\":{},\"cx\":{},\"cy\":{},\"rotation\":[{},{},{}]}}",
                    lens.radius,
                    lens.center[0],
                    lens.center[1],
                    lens.rotation[0],
                    lens.rotation[1],
                    lens.rotation[2]
                )
            })
            .collect();

        Some(format!(
            "{{\"lens_count\":{},\"lens\":[{}],\"width\":{},\"height\":{},\
\"fov_deg\":{},\"yaw_deg\":{:.3},\"model\":\"{}\"}}",
            calibration.lens_count,
            lenses.join(","),
            calibration.width,
            calibration.height,
            DEFAULT_FOV_DEG,
            calibration.relative_yaw_degrees().unwrap_or(0.0),
            escape_json(self.model.as_deref().unwrap_or_default())
        ))
    }
}

/// Reads the trailer of a `.insp` file, or `None` if this is not one.
pub fn read(bytes: &[u8]) -> Option<Trailer> {
    let magic_at = find_magic(bytes)?;
    let size_at = magic_at.checked_sub(8)?;
    let trailer_size = u32::from_le_bytes(bytes[size_at..size_at + 4].try_into().ok()?) as usize;
    let version = u32::from_le_bytes(bytes[size_at + 4..magic_at].try_into().ok()?);

    let trailer_end = magic_at + MAGIC.len();
    let trailer_start = trailer_end.checked_sub(trailer_size)?;
    // The trailer has to hold at least its own footer, in front of the magic.
    if trailer_start >= size_at {
        return None;
    }

    let records = read_records(bytes, size_at, trailer_start)?;

    let mut model = None;
    let mut calibration = None;
    let mut width = None;
    let mut height = None;
    for (id, payload) in records {
        if id != METADATA_RECORD_ID {
            continue;
        }
        let metadata = Metadata::parse(payload);
        model = metadata.model;
        width = metadata.width;
        height = metadata.height;
        calibration = metadata.calibration.as_deref().and_then(parse_calibration);
        break;
    }

    Some(Trailer {
        version,
        model,
        calibration,
        width,
        height,
        body_len: trailer_start,
    })
}

/// The location of the magic, or `None` if the file does not end with one.
fn find_magic(bytes: &[u8]) -> Option<usize> {
    if bytes.len() < FOOTER_LEN {
        return None;
    }
    if bytes.ends_with(MAGIC) {
        return Some(bytes.len() - MAGIC.len());
    }
    // Tolerate trailing bytes after the trailer, but do not scan a whole 72 MP
    // photo looking for something that is not there.
    let window_start = bytes.len().saturating_sub(MAGIC_SCAN_LEN + MAGIC.len());
    let window = &bytes[window_start..];
    window
        .windows(MAGIC.len())
        .rposition(|candidate| candidate == MAGIC)
        .map(|offset| window_start + offset)
}

/// A record's id and the bytes of its payload.
type Records<'a> = Vec<(u16, &'a [u8])>;

/// Walks the records backwards, choosing the padding width whose walk lands
/// exactly on the start of the trailer.
fn read_records(bytes: &[u8], footer_at: usize, trailer_start: usize) -> Option<Records<'_>> {
    // Padding width, zero-length records swallowed, and the records themselves.
    let mut best: Option<(usize, usize, Records<'_>)> = None;

    for padding in 0..=MAX_PAD_LEN {
        let Some(end) = footer_at.checked_sub(padding) else {
            break;
        };
        let Some((swallowed, records)) = walk(bytes, end, trailer_start) else {
            continue;
        };
        // A pad that is a multiple of six reads as zero-length records. Prefer
        // the width that invents the fewest of them, then the smallest width.
        let better = match &best {
            None => true,
            Some((best_swallowed, _, _)) => swallowed < *best_swallowed,
        };
        if better {
            best = Some((swallowed, padding, records));
            if swallowed == 0 {
                break;
            }
        }
    }

    best.map(|(_, _, records)| records)
}

/// One backwards walk. Returns the records and how many zero-length ones were
/// swallowed on the way, or `None` if the walk does not land on `start`.
fn walk(bytes: &[u8], end: usize, start: usize) -> Option<(usize, Records<'_>)> {
    let mut records = Vec::new();
    let mut swallowed = 0;
    let mut cursor = end;

    while cursor > start {
        let footer_at = cursor.checked_sub(RECORD_FOOTER_LEN)?;
        if footer_at < start {
            return None;
        }
        let id = u16::from_le_bytes(bytes[footer_at..footer_at + 2].try_into().ok()?);
        let size =
            u32::from_le_bytes(bytes[footer_at + 2..footer_at + 6].try_into().ok()?) as usize;

        if id == 0 && size == 0 {
            swallowed += 1;
            cursor = footer_at;
            continue;
        }

        let data_at = footer_at.checked_sub(size)?;
        if data_at < start {
            return None;
        }
        records.push((id, &bytes[data_at..footer_at]));
        cursor = data_at;
    }

    if cursor == start {
        Some((swallowed, records))
    } else {
        None
    }
}

/// The parts of the metadata record this reader cares about.
#[derive(Default)]
struct Metadata {
    model: Option<String>,
    calibration: Option<String>,
    width: Option<u32>,
    height: Option<u32>,
}

impl Metadata {
    fn parse(payload: &[u8]) -> Self {
        let mut metadata = Metadata::default();

        for (field, value) in fields(payload) {
            let Field::Bytes(value) = value else {
                continue;
            };
            match field {
                // Camera model, e.g. "Insta360 X3".
                2 => metadata.model = std::str::from_utf8(value).ok().map(str::to_string),
                // The lens calibration, as the plain text `parse_calibration` reads.
                5 => metadata.calibration = std::str::from_utf8(value).ok().map(str::to_string),
                // Image size: a sub-message of two varints, width then height.
                19 => {
                    for (field, value) in fields(value) {
                        let Field::Varint(value) = value else {
                            continue;
                        };
                        match field {
                            1 => metadata.width = u32::try_from(value).ok(),
                            2 => metadata.height = u32::try_from(value).ok(),
                            _ => {}
                        }
                    }
                }
                _ => {}
            }
        }

        metadata
    }
}

/// Parses the calibration string, e.g.
///
/// ```text
/// 2_1636.891_1707.637_1748.886_0.000_0.000_90.000_…_6912_3456_1293
/// │ └───── one lens: radius, x, y, rx, ry, rz ────┘   W    H    ?
/// └ lens count
/// ```
pub fn parse_calibration(text: &str) -> Option<Calibration> {
    let mut parts = text.split('_');

    let lens_count: u32 = parts.next()?.parse().ok()?;
    if lens_count == 0 {
        return None;
    }

    let mut lens = Vec::with_capacity(lens_count as usize);
    for _ in 0..lens_count {
        lens.push(Lens {
            radius: number(parts.next()?)?,
            center: [number(parts.next()?)?, number(parts.next()?)?],
            rotation: [
                number(parts.next()?)?,
                number(parts.next()?)?,
                number(parts.next()?)?,
            ],
        });
    }

    let width = parts.next()?.parse().ok()?;
    let height = parts.next()?.parse().ok()?;
    let trailing = parts.next().and_then(|value| value.parse().ok());

    Some(Calibration {
        lens_count,
        lens,
        width,
        height,
        trailing,
    })
}

fn number(value: &str) -> Option<f32> {
    let value: f32 = value.parse().ok()?;
    value.is_finite().then_some(value)
}

enum Field<'a> {
    Varint(u64),
    Bytes(&'a [u8]),
}

/// Walks the protobuf wire format, without a schema.
///
/// Only the length-delimited and varint forms are decoded: those are the only
/// ones the metadata record uses. An unknown form ends the walk, which costs
/// nothing here because every field we want comes before it.
fn fields(payload: &[u8]) -> Vec<(u64, Field<'_>)> {
    let mut fields = Vec::new();
    let mut cursor = 0;

    while cursor < payload.len() {
        let Some((key, next)) = varint(payload, cursor) else {
            return fields;
        };
        cursor = next;
        let field = key >> 3;
        let wire = key & 0x7;

        match wire {
            0 => {
                let Some((value, next)) = varint(payload, cursor) else {
                    return fields;
                };
                cursor = next;
                fields.push((field, Field::Varint(value)));
            }
            2 => {
                let Some((len, next)) = varint(payload, cursor) else {
                    return fields;
                };
                let end = next.saturating_add(len as usize);
                if end > payload.len() {
                    return fields;
                }
                fields.push((field, Field::Bytes(&payload[next..end])));
                cursor = end;
            }
            _ => return fields,
        }
    }

    fields
}

fn varint(payload: &[u8], mut cursor: usize) -> Option<(u64, usize)> {
    let mut value = 0u64;
    for shift in (0..64).step_by(7) {
        let byte = *payload.get(cursor)?;
        cursor += 1;
        value |= u64::from(byte & 0x7f) << shift;
        if byte & 0x80 == 0 {
            return Some((value, cursor));
        }
    }
    None
}

fn escape_json(value: &str) -> String {
    value
        .chars()
        .flat_map(|character| match character {
            '"' => "\\\"".chars().collect::<Vec<_>>(),
            '\\' => "\\\\".chars().collect(),
            control if control.is_control() => " ".chars().collect(),
            other => vec![other],
        })
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn varint_decodes_multi_byte_values() {
        assert_eq!(varint(&[0x00], 0), Some((0, 1)));
        assert_eq!(varint(&[0x7f], 0), Some((127, 1)));
        assert_eq!(varint(&[0x80, 0x01], 0), Some((128, 2)));
        // 6912 = 0x1b00, the way the image width appears in a real file.
        assert_eq!(varint(&[0x80, 0x36], 0), Some((6912, 2)));
        assert_eq!(varint(&[0x80], 0), None);
    }

    #[test]
    fn fields_stop_at_an_unknown_wire_type() {
        // field 1, wire 5 (fixed32), which the metadata record does not use.
        let payload = [0x0d, 0x00, 0x00, 0x00, 0x00];
        assert!(fields(&payload).is_empty());
    }

    #[test]
    fn json_escaping_keeps_the_document_parseable() {
        assert_eq!(escape_json("a\"b\\c"), "a\\\"b\\\\c");
    }
}
