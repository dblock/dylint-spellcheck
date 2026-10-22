use std::{fs, path::Path};

use crate::support;
use support::{diagnostics, lint, project};

#[test]
#[cfg_attr(dylint_lib = "general", allow(non_thread_safe_call_in_test))]
fn rustdoc_attributes_check_literals_and_included_files() {
    let temp = project();
    let root = temp.path();
    fs::write(
        root.join("src/main.rs"),
        include_str!("../fixtures/rustdoc.rs.txt"),
    )
    .unwrap();
    fs::write(root.join("hidden.md"), "Hiddenincludewrongword\n").unwrap();
    fs::write(
        root.join("README.md"),
        "hello Includedwrongword\n\n```\nSkippedwrongword\n```\n",
    )
    .unwrap();
    fs::write(root.join("dylint.toml"), "[dylint-spellcheck]\ninclude = [\"README.md\"]\ndictionaries = [\"crate:dylint-spellcheck/data/en\", \".spellcheck.txt\"]\n").unwrap();
    let output = lint(root, false);
    let stderr = diagnostics(&output);
    assert!(output.status.success(), "{stderr}");
    assert!(stderr.contains("`helo`"), "{stderr}");
    assert!(stderr.contains("src/main.rs:3:16"), "{stderr}");
    assert!(stderr.contains("`wrld`"), "{stderr}");
    assert!(stderr.contains("README.md:1:7"), "{stderr}");
    assert_eq!(
        stderr
            .matches("misspelled word: `Includedwrongword`")
            .count(),
        1,
        "{stderr}"
    );
    assert!(
        !stderr.contains("misspelled word: `Hiddenwrongword`"),
        "{stderr}"
    );
    assert!(
        !stderr.contains("misspelled word: `Hiddenincludewrongword`"),
        "{stderr}"
    );
    assert!(
        !stderr.contains("misspelled word: `Skippedwrongword`"),
        "{stderr}"
    );
    assert_eq!(
        fs::read_to_string(root.join(".spellcheck.tmp")).unwrap(),
        "Includedwrongword\nhelo\nwrld\n"
    );
    fs::write(root.join("dylint.toml"), "[dylint-spellcheck]\ndictionaries = [\"crate:dylint-spellcheck/data/en\", \".spellcheck.txt\"]\n").unwrap();
    let output = lint(root, false);
    assert!(output.status.success(), "{}", diagnostics(&output));
    fs::write(root.join("README.md"), "Changedwrongword\n").unwrap();
    let output = lint(root, false);
    assert!(
        diagnostics(&output).contains("`Changedwrongword`"),
        "{}",
        diagnostics(&output)
    );
}

#[test]
#[cfg_attr(dylint_lib = "general", allow(non_thread_safe_call_in_test))]
fn prose_boundaries_are_shared_by_comments_markdown_and_plain_text() {
    let temp = project();
    let root = temp.path();
    fs::create_dir(root.join("dicts")).unwrap();
    fs::write(root.join("dicts/de.aff"), "SET UTF-8\n").unwrap();
    fs::write(
        root.join("dicts/de.dic"),
        "4\nGrüße\ncafe\u{301}\ncan't\nworld's\n",
    )
    .unwrap();
    fs::write(root.join("dylint.toml"), "[dylint-spellcheck]\ninclude = [\"notes.md\", \"notes.txt\"]\n[dylint-spellcheck.dictionaries]\nde = [\"dicts\", \".spellcheck.txt\"]\n").unwrap();
    let text = "Grüße cafe\u{301} can't can’t world's world’s wrong_token sha256 0xdeadbeef 1st v2beta\n'he\u{301}lo' ‘isn’t’ wrld\n";
    fs::write(root.join("notes.md"), text).unwrap();
    fs::write(root.join("notes.txt"), text).unwrap();
    fs::write(
        root.join("src/main.rs"),
        format!(
            "// {}\nfn main() {{}}\n",
            text.trim_end().replace('\n', "\n// ")
        ),
    )
    .unwrap();
    let output = lint(root, false);
    let stderr = diagnostics(&output);
    assert!(output.status.success(), "{stderr}");
    assert_eq!(
        stderr.matches("warning: misspelled word:").count(),
        9,
        "{stderr}"
    );
    for word in ["he\u{301}lo", "isn’t", "wrld"] {
        assert_eq!(
            stderr
                .matches(&format!("misspelled word: `{word}`"))
                .count(),
            3,
            "{stderr}"
        );
    }
    for location in [
        "notes.md:2:2",
        "notes.md:2:10",
        "notes.md:2:17",
        "notes.txt:2:2",
        "notes.txt:2:10",
        "notes.txt:2:17",
        "src/main.rs:2:5",
        "src/main.rs:2:13",
        "src/main.rs:2:20",
    ] {
        assert!(stderr.contains(location), "{stderr}");
    }
    assert_eq!(
        fs::read_to_string(root.join(".spellcheck.tmp")).unwrap(),
        "he\u{301}lo\nisn’t\nwrld\n"
    );
}

#[test]
#[cfg_attr(dylint_lib = "general", allow(non_thread_safe_call_in_test))]
fn plain_text_is_literal_excludes_resources_and_rechecks_edits() {
    let temp = project();
    let root = temp.path();
    fs::write(root.join("dylint.toml"), "[dylint-spellcheck]\ninclude = [\"*.txt\", \"*.TXT\"]\ndictionaries = [\"crate:dylint-spellcheck/data/en\", \"vocabulary.txt\", \".spellcheck.txt\"]\n").unwrap();
    fs::write(
        root.join("vocabulary.txt"),
        "# Vocabularywrongword\nProjectword\n",
    )
    .unwrap();
    #[cfg(unix)]
    std::os::unix::fs::symlink(root.join("vocabulary.txt"), root.join("alias.txt")).unwrap();
    fs::write(
        root.join("notes.TXT"),
        "Projectword `helo`\nworld **wrld**\nhttps://ignoredword.test a@ignoredword.test\n{{ Plaintextwrongword }}\n",
    )
    .unwrap();
    let output = lint(root, false);
    let stderr = diagnostics(&output);
    assert!(output.status.success(), "{stderr}");
    assert!(stderr.contains("notes.TXT:1:14"), "{stderr}");
    assert!(stderr.contains("notes.TXT:2:9"), "{stderr}");
    assert!(stderr.contains("notes.TXT:4:4"), "{stderr}");
    assert_eq!(
        fs::read_to_string(root.join(".spellcheck.tmp")).unwrap(),
        "Plaintextwrongword\nhelo\nwrld\n"
    );
    fs::rename(root.join(".spellcheck.tmp"), root.join(".spellcheck.txt")).unwrap();
    let output = lint(root, false);
    assert!(output.status.success(), "{}", diagnostics(&output));
    assert!(
        !diagnostics(&output).contains("misspelled word"),
        "{}",
        diagnostics(&output)
    );
    fs::write(root.join("notes.TXT"), "Changedwrongword\n").unwrap();
    let output = lint(root, false);
    assert!(
        diagnostics(&output).contains("`Changedwrongword`"),
        "{}",
        diagnostics(&output)
    );
}

#[test]
#[cfg_attr(dylint_lib = "general", allow(non_thread_safe_call_in_test))]
fn scoped_suppression_controls_diagnostics_and_candidates() {
    let temp = project();
    let root = temp.path();
    let source = "#![allow(dead_code)]\n#[allow(spellcheck)]\n/// Docwrongword\nfn hidden() {\n    // Bodywrongword\n    #[warn(spellcheck)]\n    {\n        // Visiblewrongword\n    }\n}\nfn main() {\n    #[allow(spellcheck)]\n    {\n        // Blockwrongword\n    }\n}\n";
    fs::write(
        root.join("src/main.rs"),
        format!("{source}\n#[allow(spellcheck)]\nmod external;\n"),
    )
    .unwrap();
    fs::write(root.join("src/external.rs"), "// Externalwrongword\n").unwrap();
    fs::write(root.join("dylint.toml"), "[dylint-spellcheck]\ninclude = [\"README.md\"]\ndictionaries = [\"crate:dylint-spellcheck/data/en\", \".spellcheck.txt\"]\n").unwrap();
    fs::write(root.join("README.md"), "<!-- spellcheck:disable -->\nHiddenwrongword\n<!-- spellcheck:enable -->\nDocumentwrongword\nInlinewrongword <!-- spellcheck:disable -->\nTodowrongword <!-- spellcheck:todo -->\n").unwrap();
    let output = lint(root, false);
    let stderr = diagnostics(&output);
    assert!(output.status.success(), "{stderr}");
    assert!(stderr.contains("`Visiblewrongword`"), "{stderr}");
    assert!(stderr.contains("`Documentwrongword`"), "{stderr}");
    for word in [
        "Docwrongword",
        "Bodywrongword",
        "Blockwrongword",
        "Hiddenwrongword",
        "Inlinewrongword",
        "Todowrongword",
        "Externalwrongword",
    ] {
        assert!(!stderr.contains(&format!("`{word}`")), "{stderr}");
    }
    assert_eq!(
        fs::read_to_string(root.join(".spellcheck.tmp")).unwrap(),
        "Documentwrongword\nVisiblewrongword\n"
    );
    fs::write(
        root.join("src/main.rs"),
        source.replace("warn(spellcheck)", "deny(spellcheck)"),
    )
    .unwrap();
    let output = lint(root, false);
    assert!(!output.status.success(), "{}", diagnostics(&output));
    assert!(diagnostics(&output).contains("error: misspelled word: `Visiblewrongword`"));
}

#[test]
#[cfg_attr(dylint_lib = "general", allow(non_thread_safe_call_in_test))]
fn liquid_templates_are_excluded_from_markdown_diagnostics_and_candidates() {
    let temp = project();
    let root = temp.path();
    fs::write(
        root.join("dylint.toml"),
        "[dylint-spellcheck]\ninclude = [\"README.md\"]\ndictionaries = [\"crate:dylint-spellcheck/data/en\", \".spellcheck.txt\"]\n",
    )
    .unwrap();
    fs::write(
        root.join("README.md"),
        "---\ntitle: \"{{ Frontmatterwrongword }}\"\n---\n\
         café {{\n  Expressionwrongword\n}} Afterliquidword\n\
         {% if Conditionwrongword\n%} Tagvisibleword\n\
         {% assign ignored = \"<!-- spellcheck:disable -->\" %}\n\
         Stillvisibleword\n\
         `{{ Inlinecodewrongword }}` [Linkvisibleword](https://ignoredword.test)\n\
         ```liquid\n{{ Fencedwrongword }}\n```\nTailvisibleword\n",
    )
    .unwrap();

    let output = lint(root, false);
    let stderr = diagnostics(&output);
    assert!(output.status.success(), "{stderr}");
    for word in [
        "Afterliquidword",
        "Tagvisibleword",
        "Stillvisibleword",
        "Linkvisibleword",
        "Tailvisibleword",
    ] {
        assert!(stderr.contains(&format!("`{word}`")), "{stderr}");
    }
    for word in [
        "Frontmatterwrongword",
        "Expressionwrongword",
        "Conditionwrongword",
        "Inlinecodewrongword",
        "Fencedwrongword",
        "ignoredword",
    ] {
        assert!(!stderr.contains(&format!("`{word}`")), "{stderr}");
    }
    assert!(stderr.contains("README.md:6:4"), "{stderr}");

    let candidates = fs::read_to_string(root.join(".spellcheck.tmp")).unwrap();
    for word in [
        "Afterliquidword",
        "Tagvisibleword",
        "Stillvisibleword",
        "Linkvisibleword",
        "Tailvisibleword",
    ] {
        assert!(candidates.lines().any(|candidate| candidate == word));
    }
    for word in [
        "Frontmatterwrongword",
        "Expressionwrongword",
        "Conditionwrongword",
        "Inlinecodewrongword",
        "Fencedwrongword",
    ] {
        assert!(!candidates.lines().any(|candidate| candidate == word));
    }
}

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
    let output = support::lint_command(root)
        .arg("--all-features")
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
    fs::create_dir(root.join("dicts")).unwrap();
    fs::write(root.join("dicts/en.aff"), "SET UTF-8\n").unwrap();
    fs::write(root.join("dicts/en.dic"), "1\nhelo\n").unwrap();
    fs::write(
        root.join("dylint.toml"),
        "[dylint-spellcheck]\ninclude = [\"README.md\"]\ndictionaries = [\"crate:dylint-spellcheck/data/en\", \"dicts\"]\n",
    )
    .unwrap();
    let output = lint(root, false);
    assert!(output.status.success(), "{}", diagnostics(&output));
    assert!(
        !diagnostics(&output).contains("misspelled word"),
        "{}",
        diagnostics(&output)
    );
    fs::write(root.join("README.md"), "hello helo wrld\n").unwrap();
    let output = lint(root, false);
    assert!(
        diagnostics(&output).contains("misspelled word: `wrld`"),
        "{}",
        diagnostics(&output)
    );
    // New pairs and edits must invalidate checks without Rust source changes.
    fs::write(root.join("dicts/de.aff"), "SET UTF-8\n").unwrap();
    fs::write(root.join("dicts/de.dic"), "1\nwrld\n").unwrap();
    let output = lint(root, false);
    assert!(output.status.success(), "{}", diagnostics(&output));
    assert!(
        !diagnostics(&output).contains("misspelled word"),
        "{}",
        diagnostics(&output)
    );
    fs::write(root.join("dicts/en.dic"), "1\nworld\n").unwrap();
    let output = lint(root, false);
    assert!(
        diagnostics(&output).contains("misspelled word: `helo`"),
        "{}",
        diagnostics(&output)
    );
}

#[test]
#[cfg_attr(dylint_lib = "general", allow(non_thread_safe_call_in_test))]
fn front_matter_configuration_spans_suppression_and_candidates() {
    let temp = tempfile::tempdir_in(Path::new(env!("CARGO_MANIFEST_DIR")).join("target")).unwrap();
    let root = temp.path();
    fs::create_dir(root.join("src")).unwrap();
    fs::write(
        root.join("Cargo.toml"),
        "[package]\nname = \"front-matter\"\nversion = \"0.1.0\"\nedition = \"2021\"\n[workspace]\n",
    )
    .unwrap();
    fs::write(
        root.join("src/main.rs"),
        "/// ---\n/// title: Rustfrontwrongword\n/// ---\n/// Rustbodywrongword\nfn main() { attribute(); }\n#[doc = \"---\\ntitle: Attributefrontwrongword\\n---\\nAttributebodywrongword\"]\nfn attribute() {}\n",
    )
    .unwrap();
    let settings = "[dylint-spellcheck]\ninclude = [\"README.md\"]\ndictionaries = [\"crate:dylint-spellcheck/data/en\", \".spellcheck.txt\"]\n";
    let source = "---\ntitle: \"h\\x65lo\"\nnested:\n  values: [Arraywrongword, true, false, null, .nan]\n  hidden: Excludedwrongword\nsuppressed: 'Hiddenwrongword <!-- spellcheck:todo -->'\n---\nBodywrongword\n";
    fs::write(root.join("README.md"), source).unwrap();
    fs::write(
        root.join("dylint.toml"),
        format!(
            "{settings}[dylint-spellcheck.markdown.front_matter]\nexclude = [\"nested.hidden\"]\n"
        ),
    )
    .unwrap();
    let output = lint(root, false);
    let stderr = diagnostics(&output);
    assert!(output.status.success(), "{stderr}");
    assert!(stderr.contains("README.md:2:9"), "{stderr}");
    assert!(stderr.contains("h\\x65lo"), "{stderr}");
    assert!(stderr.contains("src/main.rs:2:12"), "{stderr}");
    assert_eq!(
        fs::read_to_string(root.join(".spellcheck.tmp")).unwrap(),
        "Arraywrongword\nAttributebodywrongword\nAttributefrontwrongword\nBodywrongword\nRustbodywrongword\nRustfrontwrongword\nhelo\n"
    );
    for selection in ["enabled = false", "include = []"] {
        fs::remove_file(root.join(".spellcheck.tmp")).unwrap();
        fs::write(
            root.join("dylint.toml"),
            format!("{settings}[dylint-spellcheck.markdown.front_matter]\n{selection}\n"),
        )
        .unwrap();
        let output = lint(root, false);
        assert!(output.status.success(), "{}", diagnostics(&output));
        assert_eq!(
            fs::read_to_string(root.join(".spellcheck.tmp")).unwrap(),
            "Attributebodywrongword\nBodywrongword\nRustbodywrongword\n"
        );
    }
    fs::write(root.join("README.md"), "---\ntitle: [broken\n---\n").unwrap();
    let output = lint(root, false);
    assert!(!output.status.success(), "{}", diagnostics(&output));
    let stderr = diagnostics(&output);
    assert!(stderr.contains("invalid YAML front matter"), "{stderr}");
    assert!(stderr.contains("README.md"), "{stderr}");
    fs::write(root.join("README.md"), source).unwrap();
    for invalid in ["include = [\"nested..title\"]", "enabled = \"false\""] {
        fs::write(
            root.join("dylint.toml"),
            format!("{settings}[dylint-spellcheck.markdown.front_matter]\n{invalid}\n"),
        )
        .unwrap();
        let output = lint(root, false);
        assert!(!output.status.success(), "{}", diagnostics(&output));
        assert!(diagnostics(&output).contains("front_matter"));
    }
    for (invalid, field) in [
        (
            "[dylint-spellcheck.front_matter]\nenabled = false\n",
            "front_matter",
        ),
        ("[dylint-spellcheck.markdown]\nenabled = false\n", "enabled"),
        (
            "[dylint-spellcheck.markdown.front_matter]\nunknown = []\n",
            "unknown",
        ),
    ] {
        fs::write(root.join("dylint.toml"), format!("{settings}{invalid}")).unwrap();
        let output = lint(root, false);
        let stderr = diagnostics(&output);
        assert!(!output.status.success(), "{stderr}");
        assert!(
            stderr.contains(&format!("unknown field `{field}`")),
            "{stderr}"
        );
    }
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
    fs::write(root.join("dylint.toml"), "[dylint-spellcheck]\ninclude = [\"README.md\", \"./README.md\", \"docs/**/*.md\", \"future/**/*.markdown\"]\nexclude = [\"docs/generated/**\"]\n[dylint-spellcheck.dictionaries]\nen = [\"crate:dylint-spellcheck/data/en\", \".spellcheck.txt\"]\n").unwrap();
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
    fs::write(root.join("unsupported.rst"), "hello\n").unwrap();
    fs::write(
        root.join("dylint.toml"),
        "[dylint-spellcheck]\ninclude = [\"unsupported.rst\"]\n",
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
