use std::{fs, path::Path, process::Command};

#[test]
#[cfg_attr(dylint_lib = "general", allow(non_thread_safe_call_in_test))]
fn root_package_owns_documents_and_prefers_a_binary_without_required_features() {
    let temp = tempfile::tempdir().unwrap();
    let root = temp.path();
    fs::write(root.join("Cargo.toml"), format!(
        "[package]\nname = \"root\"\nversion = \"0.1.0\"\nedition = \"2021\"\n[workspace]\nmembers = [\"aaa\"]\n[workspace.metadata.dylint]\nlibraries = [{{ path = {:?} }}]\n[features]\nextra = []\n[[bin]]\nname = \"a\"\npath = \"src/a.rs\"\nrequired-features = [\"extra\"]\n[[bin]]\nname = \"z\"\npath = \"src/z.rs\"\n",
        env!("CARGO_MANIFEST_DIR")
    )).unwrap();
    fs::create_dir(root.join("src")).unwrap();
    fs::write(root.join("src/a.rs"), "fn main() {}\n").unwrap();
    fs::write(root.join("src/z.rs"), "fn main() {}\n").unwrap();
    fs::create_dir_all(root.join("aaa/src")).unwrap();
    fs::write(
        root.join("aaa/Cargo.toml"),
        "[package]\nname = \"aaa\"\nversion = \"0.1.0\"\nedition = \"2021\"\n",
    )
    .unwrap();
    fs::write(root.join("aaa/src/main.rs"), "fn main() {}\n").unwrap();
    fs::write(
        root.join("dylint.toml"),
        "[dylint-spellcheck]\ninclude = [\"README.md\"]\n",
    )
    .unwrap();
    fs::write(root.join("README.md"), "hello helo\n").unwrap();
    let output = lint(&root.join("aaa"), false);
    assert!(output.status.success(), "{}", diagnostics(&output));
    assert_eq!(
        diagnostics(&output)
            .matches("warning: misspelled word: `helo`")
            .count(),
        1,
        "{}",
        diagnostics(&output)
    );
    let output = Command::new(std::env::var_os("CARGO").unwrap_or_else(|| "cargo".into()))
        .current_dir(root)
        .env("CARGO_TERM_COLOR", "never")
        .args([
            "dylint",
            "--all",
            "--",
            "--workspace",
            "--all-targets",
            "--all-features",
        ])
        .output()
        .unwrap();
    assert!(output.status.success(), "{}", diagnostics(&output));
    assert_eq!(
        diagnostics(&output)
            .matches("warning: misspelled word: `helo`")
            .count(),
        1,
        "{}",
        diagnostics(&output)
    );
}

fn lint(root: &Path, strict: bool) -> std::process::Output {
    let mut command = Command::new(std::env::var_os("CARGO").unwrap_or_else(|| "cargo".into()));
    command
        .current_dir(root)
        .env("CARGO_TERM_COLOR", "never")
        .args(["dylint", "--all", "--", "--workspace", "--all-targets"]);
    if strict {
        command.env("RUSTFLAGS", "-D warnings");
    }
    command.output().unwrap()
}

fn diagnostics(output: &std::process::Output) -> String {
    String::from_utf8_lossy(&output.stderr).into_owned()
}

#[test]
#[cfg_attr(dylint_lib = "general", allow(non_thread_safe_call_in_test))]
fn documents_recheck_without_source_edits_and_report_once_per_workspace() {
    let temp = tempfile::tempdir().unwrap();
    let root = temp.path();
    fs::write(root.join("Cargo.toml"), format!(
        "[workspace]\nmembers = [\"first\", \"second\"]\nresolver = \"2\"\n[workspace.metadata.dylint]\nlibraries = [{{ path = {:?} }}]\n",
        env!("CARGO_MANIFEST_DIR")
    )).unwrap();
    for member in ["first", "second"] {
        fs::create_dir_all(root.join(member).join("src")).unwrap();
        fs::write(
            root.join(member).join("Cargo.toml"),
            format!("[package]\nname = \"{member}\"\nversion = \"0.1.0\"\nedition = \"2021\"\n"),
        )
        .unwrap();
        fs::write(root.join(member).join("src/main.rs"), "fn main() {}\n").unwrap();
    }
    fs::write(root.join("first/src/lib.rs"), "").unwrap();
    fs::create_dir(root.join("docs")).unwrap();
    fs::write(root.join("dylint.toml"), "[dylint-spellcheck]\ninclude = [\"README.md\", \"./README.md\", \"docs/**/*.md\", \"future/**/*.markdown\"]\nexclude = [\"docs/generated/**\"]\nvocabulary_files = [\".spellcheck.txt\"]\n").unwrap();
    fs::write(root.join(".spellcheck.txt"), "Kotoshu\n").unwrap();
    let readme = "# hello\n\nKotoshu [world](wrongword) `typpo`\n\n```rust\nwrongword\n```\n\n    wrongword\n\n<!-- typpo -->\n";
    fs::write(root.join("README.md"), readme).unwrap();
    fs::write(root.join("docs/guide.md"), "hello world\n").unwrap();
    fs::create_dir(root.join("docs/generated")).unwrap();
    fs::write(root.join("docs/generated/skip.md"), "excludedtyppo\n").unwrap();

    let output = lint(&root.join("second"), false);
    assert!(output.status.success(), "{}", diagnostics(&output));
    assert!(
        !diagnostics(&output).contains("misspelled word"),
        "{}",
        diagnostics(&output)
    );

    // Only documents change: Cargo must not replay stale diagnostics.
    fs::write(root.join("README.md"), "# hello helo\n").unwrap();
    fs::write(root.join("docs/guide.md"), "world wrld\n").unwrap();
    let output = lint(root, false);
    let stderr = diagnostics(&output);
    assert!(output.status.success(), "{stderr}");
    assert_eq!(
        stderr.matches("warning: misspelled word: `helo`").count(),
        1,
        "{stderr}"
    );
    assert_eq!(
        stderr.matches("warning: misspelled word: `wrld`").count(),
        1,
        "{stderr}"
    );
    assert!(stderr.contains("README.md:1:9"), "{stderr}");
    assert!(stderr.contains("docs/guide.md:1:7"), "{stderr}");
    assert!(stderr.contains("did you mean"), "{stderr}");
    let output = lint(root, true);
    assert!(!output.status.success(), "{}", diagnostics(&output));
    assert!(
        diagnostics(&output).contains("misspelled word: `helo`"),
        "{}",
        diagnostics(&output)
    );

    fs::write(root.join("README.md"), readme).unwrap();
    fs::write(root.join("docs/guide.md"), "hello world\n").unwrap();
    let output = lint(root, false);
    assert!(output.status.success(), "{}", diagnostics(&output));
    assert!(
        !diagnostics(&output).contains("misspelled word"),
        "{}",
        diagnostics(&output)
    );

    fs::create_dir_all(root.join("future/nested")).unwrap();
    fs::write(root.join("future/nested/new.markdown"), "hello helo\n").unwrap();
    let output = lint(root, false);
    assert!(output.status.success(), "{}", diagnostics(&output));
    assert!(
        diagnostics(&output).contains("new.markdown"),
        "{}",
        diagnostics(&output)
    );
    assert!(
        !diagnostics(&output).contains("excludedtyppo"),
        "{}",
        diagnostics(&output)
    );
    fs::remove_file(root.join("future/nested/new.markdown")).unwrap();
    fs::remove_file(root.join("docs/guide.md")).unwrap();
    let output = lint(root, false);
    assert!(output.status.success(), "{}", diagnostics(&output));
    assert!(
        !diagnostics(&output).contains("misspelled word"),
        "{}",
        diagnostics(&output)
    );
    fs::write(root.join("docs/guide.md"), "world wrld\n").unwrap();

    fs::remove_file(root.join(".spellcheck.txt")).unwrap();
    let output = lint(root, false);
    assert!(output.status.success(), "{}", diagnostics(&output));
    assert_eq!(
        fs::read_to_string(root.join(".spellcheck.tmp")).unwrap(),
        "Kotoshu\nwrld\n"
    );
    assert!(!diagnostics(&output).contains("did you mean"));
    fs::rename(root.join(".spellcheck.tmp"), root.join(".spellcheck.txt")).unwrap();
    let output = lint(root, false);
    assert!(output.status.success(), "{}", diagnostics(&output));
    assert!(
        !diagnostics(&output).contains("misspelled word"),
        "{}",
        diagnostics(&output)
    );

    fs::write(root.join("first/src/lib.rs"), "#![allow(spellcheck)]\n").unwrap();
    fs::remove_file(root.join(".spellcheck.txt")).unwrap();
    let output = lint(root, false);
    assert!(output.status.success(), "{}", diagnostics(&output));
    assert!(
        !diagnostics(&output).contains("misspelled word"),
        "{}",
        diagnostics(&output)
    );
    assert_eq!(
        fs::read_to_string(root.join(".spellcheck.tmp")).unwrap(),
        ""
    );

    // Library ownership also applies to non-Rust-linkable library crate types.
    fs::write(root.join("first/src/lib.rs"), "").unwrap();
    fs::write(root.join("first/Cargo.toml"), "[package]\nname = \"first\"\nversion = \"0.1.0\"\nedition = \"2021\"\n[lib]\ncrate-type = [\"cdylib\"]\n").unwrap();
    let output = lint(root, false);
    assert!(output.status.success(), "{}", diagnostics(&output));
    assert_eq!(
        diagnostics(&output)
            .matches("warning: misspelled word: `wrld`")
            .count(),
        1,
        "{}",
        diagnostics(&output)
    );

    fs::write(
        root.join("dylint.toml"),
        "[dylint-spellcheck]\ninclude = [\"missing.md\"]\n",
    )
    .unwrap();
    let output = lint(root, false);
    assert!(!output.status.success(), "{}", diagnostics(&output));
    assert!(
        diagnostics(&output).contains("could not read spelling input"),
        "{}",
        diagnostics(&output)
    );
    fs::write(root.join("unsupported.txt"), "hello\n").unwrap();
    fs::write(
        root.join("dylint.toml"),
        "[dylint-spellcheck]\ninclude = [\"unsupported.txt\"]\n",
    )
    .unwrap();
    let output = lint(root, false);
    assert!(!output.status.success(), "{}", diagnostics(&output));
    assert!(
        diagnostics(&output).contains("unsupported spelling input format"),
        "{}",
        diagnostics(&output)
    );
}
