use std::process::Command;

fn main() {
    println!("cargo:rerun-if-changed=resources/photosphereviewer/index.html");
    println!("cargo:rerun-if-changed=resources/photosphereviewer/src/viewer.js");
    println!("cargo:rerun-if-changed=resources/photosphereviewer/src/style.css");
    println!("cargo:rerun-if-changed=resources/photosphereviewer/public/demo.webp");
    println!("cargo:rerun-if-changed=resources/data/window.blp");
    println!("cargo:rerun-if-changed=resources/data/shortcuts.blp");

    let status = Command::new("npm")
        .arg("run")
        .arg("build")
        .current_dir("resources/photosphereviewer")
        .status()
        .expect("Failed to run npm build!");
    if !status.success() {
        panic!("npm run build failed!");
    } else {
        println!("cargo:warning=npm run build succeeded")
    }
}
