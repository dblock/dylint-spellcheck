use std::{fs, path::Path, process::Command};

mod support;

fn lint(root: &Path) -> std::process::Output {
    lint_command(root).output().unwrap()
}

fn lint_command(root: &Path) -> Command {
    let mut command = support::lint_command(root);
    command.env("CARGO_CACHE_RUSTC_INFO", "0");
    command
}

#[test]
fn collects_across_compiler_processes_and_rechecks_after_promotion() {
    let temp = tempfile::tempdir().unwrap();
    let root = temp.path();
    fs::write(root.join("Cargo.toml"), format!(
        "[workspace]\nmembers = [\"first\", \"second\", \"suppressed\"]\nresolver = \"2\"\n[workspace.metadata.dylint]\nlibraries = [{{ path = {:?} }}]\n",
        env!("CARGO_MANIFEST_DIR")
    )).unwrap();
    for (member, word) in [
        ("first", "Kotoshu"),
        ("second", "speling"),
        ("suppressed", "Suppressedword"),
    ] {
        fs::create_dir_all(root.join(member).join("src")).unwrap();
        fs::write(
            root.join(member).join("Cargo.toml"),
            format!("[package]\nname = \"{member}\"\nversion = \"0.1.0\"\nedition = \"2021\"\n"),
        )
        .unwrap();
        let allowance = if member == "suppressed" {
            "#![allow(spellcheck)]\n"
        } else {
            ""
        };
        fs::write(
            root.join(member).join("src/main.rs"),
            format!("{allowance}// {word}\n// Existingword\nfn main() {{}}\n"),
        )
        .unwrap();
    }
    let config = "[dylint-spellcheck]\ndictionaries = [\"crate:dylint-spellcheck/data/en\", \"existing.txt\", \".spellcheck.txt\"]\n";
    fs::write(root.join("existing.txt"), "Existingword\n").unwrap();
    fs::write(root.join("dylint.toml"), config).unwrap();
    let result = lint(&root.join("first"));
    assert!(
        result.status.success(),
        "{}",
        String::from_utf8_lossy(&result.stderr)
    );
    let stderr = String::from_utf8_lossy(&result.stderr);
    assert!(stderr.contains("misspelled word: `speling`"));
    assert!(!stderr.contains("did you mean"));
    assert!(stderr.contains("Collected spelling candidates in .spellcheck.tmp"));
    assert_eq!(
        fs::read_to_string(root.join(".spellcheck.tmp")).unwrap(),
        "Kotoshu\nspeling\n"
    );
    assert!(!root.join(".spellcheck.txt").exists());
    assert_eq!(
        fs::read_to_string(root.join("dylint.toml")).unwrap(),
        config
    );
    assert_eq!(
        fs::read_to_string(root.join("first/src/main.rs")).unwrap(),
        "// Kotoshu\n// Existingword\nfn main() {}\n"
    );

    fs::remove_file(root.join(".spellcheck.tmp")).unwrap();
    let result = lint(root);
    assert!(
        result.status.success(),
        "{}",
        String::from_utf8_lossy(&result.stderr)
    );
    assert_eq!(
        fs::read_to_string(root.join(".spellcheck.tmp")).unwrap(),
        "Kotoshu\nspeling\n"
    );

    // No source changes: the previously missing vocabulary must invalidate both targets.
    fs::rename(root.join(".spellcheck.tmp"), root.join(".spellcheck.txt")).unwrap();
    let result = lint(root);
    assert!(
        result.status.success(),
        "{}",
        String::from_utf8_lossy(&result.stderr)
    );
    let stderr = String::from_utf8_lossy(&result.stderr);
    assert!(
        !stderr.contains("Collected spelling candidates"),
        "{stderr}"
    );
    assert!(!stderr.contains("misspelled word"), "{stderr}");
    assert!(!root.join(".spellcheck.tmp").exists());

    fs::write(root.join(".spellcheck.txt"), "Kotoshu\n").unwrap();
    let result = lint(root);
    assert!(
        result.status.success(),
        "{}",
        String::from_utf8_lossy(&result.stderr)
    );
    let stderr = String::from_utf8_lossy(&result.stderr);
    assert!(stderr.contains("misspelled word: `speling`"), "{stderr}");
    assert!(stderr.contains("did you mean"), "{stderr}");
    assert!(!root.join(".spellcheck.tmp").exists());
    assert_eq!(
        fs::read_to_string(root.join(".spellcheck.txt")).unwrap(),
        "Kotoshu\n"
    );
    assert_eq!(
        fs::read_to_string(root.join("existing.txt")).unwrap(),
        "Existingword\n"
    );

    fs::remove_file(root.join(".spellcheck.txt")).unwrap();
    let result = lint_command(root)
        .env("RUSTFLAGS", "-D warnings")
        .output()
        .unwrap();
    let stderr = String::from_utf8_lossy(&result.stderr);
    assert!(!result.status.success(), "{stderr}");
    assert!(stderr.contains("misspelled word"), "{stderr}");
    let candidates = fs::read_to_string(root.join(".spellcheck.tmp")).unwrap();
    assert!(candidates.contains("Kotoshu") || candidates.contains("speling"));
    assert!(!candidates.contains("Suppressedword"));
    assert!(!root.join(".spellcheck.txt").exists());
}
