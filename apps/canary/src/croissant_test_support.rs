use std::fs;
use std::path::{Path, PathBuf};
use std::process::Command;

pub(crate) fn committed_source_checkout(root: &Path) -> PathBuf {
    let source = root.join("committed-croissant-source");
    fs::create_dir(&source).expect("isolated source directory");
    fs::write(source.join("Cargo.toml"), "[package]\nname='fixture'\nversion='0.0.0'\n")
        .expect("isolated source input");
    for arguments in [
        &["init"][..],
        &["config", "user.email", "canary@example.invalid"],
        &["config", "user.name", "Canary"],
        &["add", "."],
        &["commit", "-m", "controlled fixture"],
    ] {
        assert!(
            Command::new("git")
                .args(arguments)
                .current_dir(&source)
                .status()
                .expect("isolated source git")
                .success()
        );
    }
    source
}
