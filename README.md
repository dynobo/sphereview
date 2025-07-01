<p align="center">
<img src="https://raw.githubusercontent.com/dynobo/sphereview/refs/heads/main/resources/assets/title.png" alt="SphereView" /> 
</p>

<p align="center">
<em><strong>Image viewer for 360° equirectangular photospheres and panoramas.</strong></em>
</p>

<p align="center">
<a href="https://github.com/dynobo/sphereview/actions/workflows/cicd.yaml" target="_blank"><img src="https://github.com/dynobo/sphereview/actions/workflows/cicd.yaml/badge.svg?branch=main" alt="CI/CD"></a>
<a href="https://github.com/dynobo/sphereview/actions/workflows/github-code-scanning/codeql"><img src="https://github.com/dynobo/sphereview/actions/workflows/github-code-scanning/codeql/badge.svg?branch=main" alt="CodeQL"></a>
<a href="https://hanadigital.github.io/grev/?user=dynobo&repo=sphereview"><img src="https://img.shields.io/github/downloads/dynobo/sphereview/total?label=Github%20downloads&color=blue" alt="GitHub"></a>
<a href="https://flathub.org/apps/details/io.github.dynobo.sphereview"><img src="https://img.shields.io/flathub/downloads/io.github.dynobo.sphereview?label=Flathub%20downloads&color=blue" alt="Flathub"></a>
</p>

<p align="center">
Links: <a href="https://github.com/dynobo/sphereview">Source Code</a> |
<a href="https://github.com/dynobo/sphereview/releases">Releases</a> |
<a href="https://github.com/dynobo/sphereview/blob/main/CHANGELOG">Changelog</a>
</p>
<br/>

This my very first Rust Application. It stands on the shoulders of the fabulous JavaScript library [Photo Sphere Viewer](https://photo-sphere-viewer.js.org/) which deserves all credits for the image rendering and controls. SphereView just wraps it in a GTK interface to provide a desktop integration.

## Installation

- Install from FlatHub (_comming soon!_) 

## Features

- View panoramas and photospheres interactively
- Lean interface with minimal features
- 100% offline, no telemetry
- Supports Equirectanglar projection (e.g. used by Google Camera)
- Supports formats like JPEG, PNG, WEBP

## Screencast

<p align="center">
<img src="https://raw.githubusercontent.com/dynobo/sphereview/refs/heads/main/resources/assets/screencast.gif" alt="Screencast of SphereView in action" width="700"/> 
</p>

## Development

```sh
# Install dependencies (Debian/Ubuntu)
sudo apt install -y \
  rustup \
  nodejs npm \
  libgtk-4-dev \
  libadwaita-1-dev \
  libglib2.0-dev \
  libwebkitgtk-6.0-dev \
  blueprint-compiler

# Clone repo
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
