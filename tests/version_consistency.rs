use regex::Regex;
use roxmltree;
use std::fs;
use std::path::PathBuf;
use toml::Value;

fn get_project_root() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR"))
}

#[test]
fn version_consistency() {
    let app_version = sphereview::APP_VERSION;

    let cargo_version = get_cargo_version().expect("No version found in Cargo.toml");

    let appdata_version = get_appdata_version().expect("No release version found in appdata.xml");

    let changelog_version = get_changelog_version().expect("No version found in CHANGELOG");

    assert_eq!(
        app_version, cargo_version,
        "Version mismatch: APP_VERSION = {}, cargo.toml = {}",
        app_version, cargo_version
    );
    assert_eq!(
        app_version, appdata_version,
        "Version mismatch: APP_VERSION = {}, appdata.xml = {}",
        app_version, appdata_version
    );
    assert_eq!(
        app_version, changelog_version,
        "Version mismatch: APP_VERSION = {}, CHANGELOG = {}",
        app_version, changelog_version
    );
}

fn get_appdata_version() -> Option<String> {
    let root = get_project_root();
    let xml_content =
        fs::read_to_string(root.join("io.github.dynobo.sphereview.appdata.xml")).ok()?;
    let doc = roxmltree::Document::parse(&xml_content).ok()?;

    doc.descendants()
        .find(|n| n.has_tag_name("releases"))?
        .children()
        .find(|n| n.has_tag_name("release"))
        .and_then(|n| n.attribute("version"))
        .map(|v| v.to_string())
}

fn get_changelog_version() -> Option<String> {
    let root = get_project_root();
    let changelog_content = fs::read_to_string(root.join("CHANGELOG")).ok()?;
    let re = Regex::new(r"## v(\d+\.\d+\.\d+)").ok()?;
    let captures = re.captures(&changelog_content)?;
    Some(captures.get(1)?.as_str().to_string())
}

fn get_cargo_version() -> Option<String> {
    let root = get_project_root();
    let cargo_toml_content = fs::read_to_string(root.join("Cargo.toml")).ok()?;
    let cargo_toml: Value = toml::from_str(&cargo_toml_content).ok()?;
    let cargo_version = cargo_toml["package"]["version"].as_str()?;
    Some(cargo_version.to_string())
}
