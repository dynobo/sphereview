#!/usr/bin/env python3
"""Builds a synthetic Insta360 photo for testing SphereView's projection.

The output is a real `.insp`: a JPEG holding two fisheye circles side by side,
with an Insta360 trailer behind it carrying a lens calibration. Nothing in it
came from a camera, so it can be committed without anyone's copyright, and it
needs no camera to regenerate.

What it is good for:

* the Rust side end to end — trailer parsing, the JPEG/trailer split, the
  parameters handed to the viewer (`cargo test`)
* the shader and the adapter wiring — open the file in SphereView and compare
  against the equirectangular image it was made from, which is what
  `--equirect` writes

What it cannot do: say anything about a *real* camera's conventions. The fisheye
pair is generated with the same mapping the shader uses, so the round trip is
consistent by construction. A wrong sign in the shader would agree with a wrong
sign here. Only a camera file can settle that; `tests/insta360_trailer.rs` says
how to run one.

Usage:
    python3 tools/make_synthetic_insp.py OUT.insp [--equirect REFERENCE.jpg]
"""

from __future__ import annotations

import argparse
import struct
from io import BytesIO

import numpy as np
from PIL import Image, ImageDraw

# The same lens geometry as a real file, scaled to the height used here. Taken
# from an Insta360 ONE photo: radius and centre of each circle, and the small
# rotations of the two lenses in the body.
HEIGHT = 1024
WIDTH = 2 * HEIGHT
# What each lens sees across its circle. Its rim is at half this angle from the
# lens axis - the first version of the viewer treated it as a half-angle, which
# stretched the sphere by 2x radially.
FOV_DEG = 194.0
LENS_RADIUS = (0.4734, 0.4784)          # as a fraction of the image height
LENS_CENTER = ((0.4941, 0.5061), (1.5044, 0.5008))  # x in image widths / height, y / height
LENS_ROTATION = ((0.0, 0.0, 90.0), (0.544, -1.614, 94.116))  # degrees

MAGIC = b"8db42d694ccc418790edff439fe026bf"


def equirectangular(height: int = HEIGHT // 2) -> Image.Image:
    """A latitude/longitude test card: recognisable in any direction."""
    width = 2 * height
    image = Image.new("RGB", (width, height), (24, 24, 28))
    draw = ImageDraw.Draw(image)

    # Sky above the horizon, ground below, as bands that make a tilt obvious.
    for y in range(height):
        latitude = 90.0 - 180.0 * (y + 0.5) / height
        if latitude > 0:
            shade = int(40 + 150 * (latitude / 90.0))
            colour = (shade // 2, shade // 2, shade)
        else:
            shade = int(40 + 110 * (-latitude / 90.0))
            colour = (shade, shade // 2, shade // 3)
        draw.line([(0, y), (width, y)], fill=colour)

    # Meridians every 30 degrees, thicker at the four cardinal directions.
    for step in range(12):
        longitude = -180.0 + 30.0 * step
        x = int((longitude + 180.0) / 360.0 * width)
        width_px = 3 if step % 3 == 0 else 1
        draw.line([(x, 0), (x, height)], fill=(230, 230, 230), width=width_px)

    # Parallels every 30 degrees.
    for step in range(1, 6):
        latitude = 90.0 - 30.0 * step
        y = int((90.0 - latitude) / 180.0 * height)
        draw.line([(0, y), (width, y)], fill=(230, 230, 230), width=1)

    # Labels, so a mirrored or rotated projection is obvious at a glance. Each is
    # centred on its meridian and drawn on both edges when it straddles the
    # antimeridian, so it comes out whole on the sphere.
    label_y = int(height * 0.42)
    for label, longitude in (("FRONT", 0.0), ("RIGHT", 90.0), ("BACK", 180.0), ("LEFT", -90.0)):
        x = (longitude + 180.0) / 360.0 * width
        for offset in (0.0, -width, width):
            draw.text((x + offset, label_y), label, fill=(255, 220, 80), anchor="mm")
    draw.text((width // 2, 10), "ZENITH", fill=(255, 220, 80), anchor="mm")
    draw.text((width // 2, height - 12), "NADIR", fill=(255, 220, 80), anchor="mm")

    return image


def direction_to_equirect(directions: np.ndarray) -> tuple[np.ndarray, np.ndarray]:
    """Longitude and latitude, in pixels, for unit vectors."""
    x, y, z = directions[..., 0], directions[..., 1], directions[..., 2]
    longitude = np.arctan2(x, z)
    latitude = np.arcsin(np.clip(y, -1.0, 1.0))
    return longitude, latitude


def sample(image: np.ndarray, longitude: np.ndarray, latitude: np.ndarray) -> np.ndarray:
    """Bilinear lookup in an equirectangular image array (height, width, 3)."""
    height, width = image.shape[:2]
    u = (longitude / (2 * np.pi) + 0.5) * width - 0.5
    v = (0.5 - latitude / np.pi) * height - 0.5
    u0 = np.floor(u).astype(int)
    v0 = np.floor(v).astype(int)
    fu = (u - u0)[..., None]
    fv = (v - v0)[..., None]
    u0 %= width
    u1 = (u0 + 1) % width
    v0 = np.clip(v0, 0, height - 1)
    v1 = np.clip(v0 + 1, 0, height - 1)
    top = image[v0, u0] * (1 - fu) + image[v0, u1] * fu
    bottom = image[v1, u0] * (1 - fu) + image[v1, u1] * fu
    return top * (1 - fv) + bottom * fv


def fisheye_pair(equirect: Image.Image) -> np.ndarray:
    """Projects the equirectangular image into two fisheye circles side by side.

    The mapping is the inverse of the one the viewer's shader applies, so the
    result is what the shader expects to find: for each pixel of each circle,
    the angle from that lens's axis picks the direction on the sphere.
    """
    pixels = np.asarray(equirect.convert("RGB"), dtype=np.float64)
    theta_rim = np.deg2rad(FOV_DEG / 2)

    out = np.zeros((HEIGHT, WIDTH, 3), dtype=np.float64)
    yy, xx = np.mgrid[0:HEIGHT, 0:WIDTH]

    for index in range(2):
        radius = LENS_RADIUS[index] * HEIGHT
        cx = LENS_CENTER[index][0] * HEIGHT
        cy = LENS_CENTER[index][1] * HEIGHT

        # Image coordinates, measured down from the top, in lens pixels.
        dx = xx - cx
        dy = cy - yy
        rho = np.hypot(dx, dy) / radius
        inside = rho <= 1.0
        theta = rho * theta_rim

        # Angle around the lens axis, then undone by the lens's roll.
        azimuth = np.arctan2(dy, dx) - np.deg2rad(LENS_ROTATION[index][2])
        sin_theta = np.sin(theta)
        # Lens 0 looks along +Z, lens 1 along -Z, and lens 1's x axis runs the
        # other way; this is the shader's convention, inside out.
        sense = 1.0 if index == 0 else -1.0
        x = sin_theta * np.cos(azimuth) * sense
        y = sin_theta * np.sin(azimuth)
        z = np.cos(theta) * sense

        longitude, latitude = direction_to_equirect(np.stack([x, y, z], axis=-1))
        colour = sample(pixels, longitude, latitude)

        out[inside] = colour[inside]

    return np.clip(out, 0, 255).astype(np.uint8)


# --- the trailer -----------------------------------------------------------


def varint(value: int) -> bytes:
    out = bytearray()
    while True:
        byte = value & 0x7F
        value >>= 7
        if value:
            out.append(byte | 0x80)
        else:
            out.append(byte)
            return bytes(out)


def field_bytes(field: int, payload: bytes) -> bytes:
    return varint((field << 3) | 2) + varint(len(payload)) + payload


def field_varint(field: int, value: int) -> bytes:
    return varint(field << 3) + varint(value)


def record(record_id: int, payload: bytes) -> bytes:
    return payload + struct.pack("<HI", record_id, len(payload))


def calibration_string() -> str:
    values = [2]
    for index in range(2):
        values.append(round(LENS_RADIUS[index] * HEIGHT, 3))
        values.append(round(LENS_CENTER[index][0] * HEIGHT, 3))
        values.append(round(LENS_CENTER[index][1] * HEIGHT, 3))
        values.extend(LENS_ROTATION[index])
    values.extend([WIDTH, HEIGHT, 1293])
    text = "_".join(f"{value:.3f}" if isinstance(value, float) else str(value) for value in values)
    return text


def metadata(image: Image.Image) -> bytes:
    payload = field_bytes(1, b"SYNTHETIC0000000")
    payload += field_bytes(2, b"SphereView synthetic")
    payload += field_bytes(3, b"v0.0.0_synthetic")
    payload += field_bytes(5, calibration_string().encode())
    dimensions = field_varint(1, image.width) + field_varint(2, image.height)
    payload += field_bytes(19, dimensions)
    return record(0x0101, payload)


def build(image: Image.Image, preview: Image.Image) -> bytes:
    body = BytesIO()
    image.save(body, format="JPEG", quality=92, subsampling=0)

    preview_bytes = BytesIO()
    preview.save(preview_bytes, format="JPEG", quality=80)

    records = (
        record(0x0300, bytes(range(256)) * 2)          # opaque, like a gyro record
        + record(0x0200, preview_bytes.getvalue())      # the camera's own preview
        + metadata(image)
    )
    trailer = records + bytes(32)
    trailer += struct.pack("<II", len(trailer) + 40, 3) + MAGIC

    return body.getvalue() + trailer


def main() -> None:
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("output", help="where to write the synthetic .insp")
    parser.add_argument(
        "--equirect",
        help="also write the equirectangular image the fisheye pair was made from",
    )
    arguments = parser.parse_args()

    reference = equirectangular()
    pair = Image.fromarray(fisheye_pair(reference))
    preview = pair.resize((pair.width // 4, pair.height // 4), Image.LANCZOS)

    with open(arguments.output, "wb") as handle:
        handle.write(build(pair, preview))

    if arguments.equirect:
        reference.save(arguments.equirect, format="JPEG", quality=92, subsampling=0)

    print(f"{arguments.output}: {pair.width}x{pair.height}")
    print(f"calibration: {calibration_string()}")


if __name__ == "__main__":
    main()
