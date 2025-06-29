use std::process::Command;

fn run_command(
    command: &str,
    args: &[&str],
    dir: &str,
    success_message: &str,
    failure_message: &str,
) {
    let status = Command::new(command)
        .args(args)
        .current_dir(dir)
        .status()
        .expect(failure_message);
    if !status.success() {
        panic!("{}", failure_message);
    } else {
        println!("cargo:warning={}", success_message);
    }
}

fn main() {
    println!("cargo:rerun-if-changed=resources/photosphereviewer/index.html");
    println!("cargo:rerun-if-changed=resources/photosphereviewer/src/viewer.js");
    println!("cargo:rerun-if-changed=resources/photosphereviewer/src/style.css");
    println!("cargo:rerun-if-changed=resources/photosphereviewer/public/demo.webp");
    println!("cargo:rerun-if-changed=resources/data/window.blp");
    println!("cargo:rerun-if-changed=resources/data/shortcuts.blp");
    println!("cargo:rerun-if-changed=resources/com.github.dynobo.sphereview.gresource.xml");
    println!("cargo:rerun-if-changed=resources/com.github.dynobo.sphereview.gresource");

    run_command(
        "npm",
        &["install"],
        "resources/photosphereviewer",
        "npm run install succeeded",
        "Failed to npm install!",
    );
    run_command(
        "npm",
        &["run", "build"],
        "resources/photosphereviewer",
        "npm run build succeeded",
        "Failed to npm run build!",
    );

    run_command(
        "glib-compile-resources",
        &[
            "--sourcedir=resources",
            "resources/com.github.dynobo.sphereview.gresource.xml",
            "--target=resources/com.github.dynobo.sphereview.gresource",
        ],
        ".",
        "glib-compile-resources succeeded",
        "Failed to compile gresource file!",
    );
}
