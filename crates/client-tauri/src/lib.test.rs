//! The bundles carry the crate's version, which a release sets to its tag's,
//! in every file that names it.

use std::path::Path;

fn json(path: &Path) -> serde_json::Value {
    let text = std::fs::read_to_string(path).expect("the file reads");
    serde_json::from_str(&text).expect("the file is JSON")
}

fn manifest_dir() -> &'static Path {
    Path::new(env!("CARGO_MANIFEST_DIR"))
}

#[test]
fn the_configuration_carries_the_crates_version() {
    // Android reads the version from the configuration alone, falling back to
    // 1.0 where it names none, and derives the versionCode from it.
    let config = json(&manifest_dir().join("tauri.conf.json"));
    assert_eq!(
        config["version"],
        serde_json::Value::from(env!("CARGO_PKG_VERSION"))
    );
    assert!(
        config.pointer("/bundle/android/versionCode").is_none(),
        "tauri.conf.json pins the Android versionCode"
    );
}

#[test]
fn the_npm_packages_carry_the_crates_version() {
    let root = manifest_dir().join("../..");
    for package in ["package.json", "packages/shell/package.json"] {
        let version = json(&root.join(package))["version"].clone();
        assert_eq!(
            version,
            serde_json::Value::from(env!("CARGO_PKG_VERSION")),
            "{package}"
        );
    }
}
