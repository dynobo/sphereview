# SphereView

**_Image viewer for equirectangular photospheres and panoramas._**

[![Coverage Status](https://img.shields.io/coverallsCoverage/github/dynobo/sphereview?label=Coverage&branch=main)](https://coveralls.io/github/dynobo/sphereview)
[![CodeQL](https://img.shields.io/github/actions/workflow/status/dynobo/sphereview/cicd.yaml?label=CodeQL&branch=main)](https://github.com/dynobo/sphereview/security/code-scanning/tools/CodeQL/status/)
[![GitHub](https://img.shields.io/github/downloads/dynobo/sphereview/total?label=Github%20downloads&color=blue)](https://hanadigital.github.io/grev/?user=dynobo&repo=sphereview)
[![Flathub](https://img.shields.io/flathub/downloads/com.github.dynobo.sphereview?label=Flathub%20downloads&color=blue)](https://flathub.org/apps/details/com.github.dynobo.sphereview)
[![AUR](https://img.shields.io/aur/votes/sphereview?label=AUR%20votes&color=blue)](https://aur.archlinux.org/packages/sphereview)

<a href="https://www.buymeacoffee.com/dynobo" target="_blank"><img src="https://cdn.buymeacoffee.com/buttons/v2/default-yellow.png" alt="Buy Me A Coffee" style="height: 40px !important;" ></a>

**Links:** [Source Code](https://github.com/dynobo/sphereview) |
[Releases](https://github.com/dynobo/sphereview/releases) |
[Changelog](https://github.com/dynobo/sphereview/blob/main/CHANGELOG)

[![Screencast](https://user-images.githubusercontent.com/11071876/189767585-8bc45c18-8392-411d-84dc-cef1cb5dbc47.gif)](https://raw.githubusercontent.com/dynobo/sphereview/main/assets/sphereview.gif)

This my very first Rust Application. It stands on the shoulders of the fabulous JavaScript library [Photo Sphere Viewer](https://photo-sphere-viewer.js.org/) which deserves all credits for the image rendering and controls. SphereView just wraps it in a GTK interface to provide a desktop integration.

## Installation

- **Install FlatPak (recommended, comming soon)**
  _or_
- Download [latest release](https://github.com/dynobo/sphereview/releases), make executable and run.

## Features

- Simplicity: Lean user interface with minimal functionality
- Supported formats: JPEG, PNG, WEBP
- Supported projection: Equirectanglar (e.g. used by Google Camera)
- Privacy: 100% offline, no telemetry

## Development

Prerequisites for development are: [rust](https://www.rust-lang.org/tools/install), [node.js](https://nodejs.org/en/download) (v24.x)


```sh
git clone https://github.com/dynobo/sphereview.git

# Change into directory with html/js  
cd sphereview/resources/photosphereviewer

# Install node.js dependencies
npm install

# Change back to repository root
cd ../..

# Install rust dependencies and run application
cargo run 
```

## Similar projects

- [FSPViewer](https://www.fsoft.it/FSPViewer/)

## Credits

- [Photo Sphere Viewer](https://github.com/mistic100/Photo-Sphere-Viewer) -  Displays 360° sphere panoramas. 
- [three.js](https://github.com/mrdoob/three.js) - 3D rendering engine.
- [GTK4](https://www.gtk.org/), [libadwaita](https://github.com/GNOME/libadwaita) and their [rust](https://docs.rs/gtk4/latest/gtk4/) [bindings](https://docs.rs/libadwaita/latest/libadwaita/) - UI Framework.
- [Poly Haven](https://polyhaven.com) - CC0 Licensed 3D assets. Source of the demo image.

## Certifications

![WOMM](https://raw.githubusercontent.com/dynobo/lmdiag/master/badge.png)