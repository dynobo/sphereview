use std::net::TcpStream;
use std::process::Command;

fn run_command(command: &str, args: &[&str], dir: &str) {
    let status = Command::new(command)
        .args(args)
        .current_dir(dir)
        .status()
        .expect(&format!("Failed: {} {}", command, args.join(" ")));
    if !status.success() {
        panic!("Failed: {} {}", command, args.join(" "));
    } else {
        println!("cargo:warning=Succeeded: {} {}", command, args.join(" "));
    }
}

fn main() {
    println!("cargo:rerun-if-changed=resources/photosphereviewer/index.html");
    println!("cargo:rerun-if-changed=resources/photosphereviewer/src/viewer.js");
    println!("cargo:rerun-if-changed=resources/photosphereviewer/src/style.css");
    println!("cargo:rerun-if-changed=resources/photosphereviewer/public/demo.webp");
    println!("cargo:rerun-if-changed=resources/data/window.blp");
    println!("cargo:rerun-if-changed=resources/data/shortcuts.blp");
    println!("cargo:rerun-if-changed=resources/io.github.dynobo.sphereview.gresource.xml");
    println!("cargo:rerun-if-changed=resources/io.github.dynobo.sphereview.gresource");

    if TcpStream::connect(("8.8.8.8", 53)).is_ok() {
        run_command("npm", &["install"], "resources/photosphereviewer");
    } else {
        run_command("npm", &["ci", "--offline"], "resources/photosphereviewer");
    }

    run_command("npm", &["run", "build"], "resources/photosphereviewer");

    run_command(
        "glib-compile-resources",
        &[
            "--sourcedir=resources",
            "resources/io.github.dynobo.sphereview.gresource.xml",
            "--target=resources/io.github.dynobo.sphereview.gresource",
        ],
        ".",
    );
}
