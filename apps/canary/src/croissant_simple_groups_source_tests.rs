    use std::fs;
    use std::os::unix::fs::PermissionsExt;
    use std::process::Command;

    use sha2::{Digest, Sha256};
    use tempfile::TempDir;

    use crate::croissant_simple_groups_source::{
        FavaSourceProvenance, PinnedFavaExecutable, SOURCE_PATHS, clean_fava_source_against,
        git, text,
    };

    const SOURCE_IMAGE: &str =
        "7777777777777777777777777777777777777777777777777777777777777777";
    const SOURCE_MANIFEST: &str =
        "5555555555555555555555555555555555555555555555555555555555555555";

    fn source_claim(
        root: &std::path::Path,
        executable: &PinnedFavaExecutable,
        revision: &str,
        build_tree: &str,
        clean: bool,
    ) -> crate::CanaryResult<FavaSourceProvenance> {
        let mut arguments = vec!["ls-tree", "-r", "--full-tree", "HEAD", "--"];
        arguments.extend(SOURCE_PATHS);
        let tree_sha256 = hex::encode(Sha256::digest(git(root, &arguments)?));
        clean_fava_source_against(
            root,
            executable,
            revision,
            build_tree,
            &tree_sha256,
            SOURCE_MANIFEST,
            clean,
            true,
            SOURCE_IMAGE,
        )
    }

    #[test]
    #[allow(
        clippy::too_many_lines,
        reason = "one committed fixture exercises every tracked, untracked, stale, and dirty build input"
    )]
    fn production_source_gate_refuses_tracked_and_relevant_untracked_changes() {
        let executable_file = TempDir::new().expect("pinned executable");
        let executable_path = executable_file.path().join("canary");
        fs::write(&executable_path, b"pinned canary bytes").expect("pinned bytes");
        fs::set_permissions(&executable_path, fs::Permissions::from_mode(0o500))
            .expect("pin permissions");
        let executable = PinnedFavaExecutable::open_for_test(&executable_path).expect("pin");
        let repository = TempDir::new().expect("source repository");
        fs::create_dir_all(repository.path().join("apps/canary/src")).expect("canary source");
        fs::create_dir_all(repository.path().join("crates/example/src")).expect("crate source");
        fs::create_dir_all(repository.path().join(".cargo")).expect("Cargo config directory");
        fs::write(repository.path().join("Cargo.toml"), "[workspace]\n").expect("manifest");
        fs::write(repository.path().join("Cargo.lock"), "version = 4\n").expect("lock");
        fs::write(repository.path().join("rust-toolchain.toml"), "[toolchain]\nchannel='stable'\n")
            .expect("toolchain");
        fs::write(repository.path().join(".cargo/config.toml"), "[build]\n")
            .expect("Cargo config");
        fs::write(
            repository.path().join("apps/canary/src/lib.rs"),
            "pub fn proof() {}\n",
        )
        .expect("canary file");
        fs::write(
            repository.path().join("crates/example/src/lib.rs"),
            "pub fn value() {}\n",
        )
        .expect("crate file");
        for arguments in [
            vec!["init"],
            vec!["config", "user.email", "canary@example.invalid"],
            vec!["config", "user.name", "Canary"],
            vec!["add", "."],
            vec!["commit", "-m", "fixture"],
        ] {
            assert!(
                Command::new("git")
                    .args(arguments)
                    .current_dir(repository.path())
                    .status()
                    .expect("git subprocess")
                    .success()
            );
        }
        let revision = text(git(repository.path(), &["rev-parse", "HEAD"]).unwrap()).unwrap();
        let build_tree =
            text(git(repository.path(), &["rev-parse", "HEAD^{tree}"]).unwrap()).unwrap();
        source_claim(repository.path(), &executable, &revision, &build_tree, true)
            .expect("committed inputs are clean");
        fs::write(
            repository.path().join("apps/canary/src/lib.rs"),
            "pub fn changed() {}\n",
        )
        .expect("tracked mutation");
        assert!(
            source_claim(repository.path(), &executable, &revision, &build_tree, true)
                .is_err()
        );
        assert!(
            Command::new("git")
                .args(["checkout", "--", "apps/canary/src/lib.rs"])
                .current_dir(repository.path())
                .status()
                .expect("git restore")
                .success()
        );
        fs::write(repository.path().join("rust-toolchain.toml"), "[toolchain]\nchannel='beta'\n")
            .expect("toolchain mutation");
        assert!(
            source_claim(repository.path(), &executable, &revision, &build_tree, true)
                .is_err()
        );
        assert!(
            Command::new("git")
                .args(["checkout", "--", "rust-toolchain.toml"])
                .current_dir(repository.path())
                .status()
                .expect("git restore")
                .success()
        );
        fs::write(repository.path().join(".cargo/config"), "[net]\noffline=true\n")
            .expect("hostile alternate Cargo config");
        assert!(
            source_claim(repository.path(), &executable, &revision, &build_tree, true)
                .is_err()
        );
        fs::remove_file(repository.path().join(".cargo/config")).expect("config cleanup");
        fs::write(
            repository.path().join("apps/canary/src/untracked.rs"),
            "hostile\n",
        )
        .expect("untracked mutation");
        assert!(
            source_claim(repository.path(), &executable, &revision, &build_tree, true)
                .is_err()
        );
        fs::remove_file(repository.path().join("apps/canary/src/untracked.rs"))
            .expect("untracked cleanup");
        assert!(
            source_claim(
                repository.path(),
                &executable,
                "0000000000000000000000000000000000000000",
                &build_tree,
                true,
            )
            .is_err(),
            "stale build revision was accepted"
        );
        assert!(
            source_claim(
                repository.path(),
                &executable,
                &revision,
                "0000000000000000000000000000000000000000",
                true,
            )
            .is_err(),
            "stale build tree was accepted"
        );
        assert!(
            source_claim(
                repository.path(),
                &executable,
                &revision,
                &build_tree,
                false,
            )
            .is_err(),
            "dirty build provenance was accepted"
        );
        let mut tree_arguments = vec!["ls-tree", "-r", "--full-tree", "HEAD", "--"];
        tree_arguments.extend(SOURCE_PATHS);
        let tree_sha256 = hex::encode(Sha256::digest(
            git(repository.path(), &tree_arguments).unwrap(),
        ));
        assert!(
            clean_fava_source_against(
                repository.path(),
                &executable,
                &revision,
                &build_tree,
                &tree_sha256,
                SOURCE_MANIFEST,
                true,
                false,
                SOURCE_IMAGE,
            )
            .is_err(),
            "mutable compiler-input provenance was accepted"
        );
        assert!(
            clean_fava_source_against(
                repository.path(),
                &executable,
                &revision,
                &build_tree,
                &tree_sha256,
                SOURCE_MANIFEST,
                true,
                true,
                "not-an-image",
            )
            .is_err(),
            "unbound compiler-input image was accepted"
        );
        assert!(
            clean_fava_source_against(
                repository.path(),
                &executable,
                &revision,
                &build_tree,
                &"0".repeat(64),
                SOURCE_MANIFEST,
                true,
                true,
                SOURCE_IMAGE,
            )
            .is_err(),
            "wrong compiler-input tree listing was accepted"
        );
    }

    #[test]
    fn opened_executable_retains_original_after_path_replacement_and_deletion() {
        let directory = TempDir::new().expect("target fixture");
        let executable = directory.path().join("canary");
        fs::write(&executable, b"reviewed canary bytes").expect("reviewed target");
        fs::set_permissions(&executable, fs::Permissions::from_mode(0o500))
            .expect("reviewed permissions");
        let pinned = PinnedFavaExecutable::open_for_test(&executable).expect("open exact image");
        let replacement = directory.path().join("replacement");
        fs::write(&replacement, b"replaced reusable target").expect("replacement bytes");
        fs::rename(&replacement, &executable).expect("replace target path");
        fs::remove_file(&executable).expect("delete replacement path");
        let retained = directory.path().join("retained-canary");
        pinned.retain(&retained).expect("retain opened original image");
        assert_eq!(fs::read(retained).unwrap(), b"reviewed canary bytes");
    }
