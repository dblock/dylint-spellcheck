use crate::support::{diagnostics, lint, project};
use std::fs;

#[test]
#[cfg_attr(dylint_lib = "general", allow(non_thread_safe_call_in_test))]
fn shared_formats_preserve_configuration_and_reviewed_vocabulary_across_runs() {
    let directory = project();
    let root = directory.path();
    for (path, source) in [
        (
            "src/main.rs",
            include_str!("../fixtures/project/main.rs.txt"),
        ),
        ("guide.md", include_str!("../fixtures/project/guide.md")),
        ("notes.txt", include_str!("../fixtures/project/notes.txt")),
    ] {
        fs::write(root.join(path), source).unwrap();
    }
    let configuration = "# Keep this comment.\n[dylint-spellcheck]\ninclude = [\"*.md\", \"*.txt\"]\nexclude = [\"excluded.txt\"]\ndictionaries = [\"crate:dylint-spellcheck/data/en\", \"existing.txt\", \".spellcheck.txt\"]\n";
    fs::write(root.join("dylint.toml"), configuration).unwrap();
    fs::write(
        root.join("existing.txt"),
        "# Keep vocabulary comments.\nQuuxify\n",
    )
    .unwrap();
    fs::write(root.join("excluded.txt"), "Excludedword\n").unwrap();
    for _ in 0..2 {
        let output = lint(root, false);
        assert!(output.status.success(), "{}", diagnostics(&output));
        assert_eq!(
            fs::read_to_string(root.join(".spellcheck.tmp")).unwrap(),
            "wrold\n"
        );
    }
    fs::rename(root.join(".spellcheck.tmp"), root.join(".spellcheck.txt")).unwrap();
    for _ in 0..2 {
        let output = lint(root, true);
        assert!(output.status.success(), "{}", diagnostics(&output));
        assert!(
            !diagnostics(&output).contains("misspelled word"),
            "{}",
            diagnostics(&output)
        );
    }
    assert_eq!(
        fs::read_to_string(root.join("dylint.toml")).unwrap(),
        configuration
    );
    assert_eq!(
        fs::read_to_string(root.join("existing.txt")).unwrap(),
        "# Keep vocabulary comments.\nQuuxify\n"
    );
    assert_eq!(
        fs::read_to_string(root.join(".spellcheck.txt")).unwrap(),
        "wrold\n"
    );
    assert_eq!(
        fs::read_to_string(root.join("guide.md")).unwrap(),
        include_str!("../fixtures/project/guide.md")
    );
    fs::write(root.join("new.txt"), "Newprojectword\n").unwrap();
    let output = lint(root, false);
    assert!(
        diagnostics(&output).contains("`Newprojectword`"),
        "{}",
        diagnostics(&output)
    );
    assert_eq!(
        fs::read_to_string(root.join(".spellcheck.txt")).unwrap(),
        "wrold\n"
    );
    assert!(!root.join(".spellcheck.tmp").exists());
}

#[test]
#[cfg_attr(dylint_lib = "general", allow(non_thread_safe_call_in_test))]
fn invalid_configuration_and_malformed_documents_fail_explicitly() {
    let directory = project();
    let root = directory.path();
    for (configuration, expected) in [
        (
            "[dylint-spellcheck]\nunknown = true\n",
            "invalid dylint-spellcheck configuration",
        ),
        (
            "[dylint-spellcheck]\ndictionaries = [\"missing\"]\n",
            "existing directory",
        ),
        (
            "[dylint-spellcheck]\ninclude = [\"missing.md\"]\n",
            "could not read spelling input",
        ),
    ] {
        fs::write(root.join("dylint.toml"), configuration).unwrap();
        let output = lint(root, false);
        assert!(!output.status.success(), "{}", diagnostics(&output));
        assert!(
            diagnostics(&output).contains(expected),
            "{}",
            diagnostics(&output)
        );
    }
    fs::write(
        root.join("dylint.toml"),
        "[dylint-spellcheck]\ninclude = [\"guide.md\"]\n",
    )
    .unwrap();
    fs::write(root.join("guide.md"), "---\ntitle: [broken\n---\n").unwrap();
    let output = lint(root, false);
    assert!(!output.status.success(), "{}", diagnostics(&output));
    assert!(
        diagnostics(&output).contains("invalid YAML front matter"),
        "{}",
        diagnostics(&output)
    );
    fs::write(root.join("guide.md"), "hello world\n").unwrap();
    for source in [
        "// ---\n// title: [broken\n// ---\nfn main() {}\n",
        "#![doc = \"---\\ntitle: [broken\\n---\\n\"]\nfn main() {}\n",
    ] {
        fs::write(root.join("src/main.rs"), source).unwrap();
        let output = lint(root, false);
        assert!(!output.status.success(), "{}", diagnostics(&output));
        assert!(
            diagnostics(&output).contains("invalid YAML front matter"),
            "{}",
            diagnostics(&output)
        );
    }
}

#[test]
#[cfg_attr(dylint_lib = "general", allow(non_thread_safe_call_in_test))]
fn expanded_doc_expressions_and_empty_docs_keep_valid_source_mapping() {
    let directory = project();
    let root = directory.path();
    fs::write(root.join("src/main.rs"), "#![allow(dead_code)]\n#[doc = concat!(\"wro\", \"ld\")]\nfn generated() {}\n#[doc = \"\"]\nfn empty() {}\n#[doc = \"hel\\\n    lo world\"]\nfn continued() {}\nfn main() {}\n").unwrap();
    let output = lint(root, false);
    assert!(output.status.success(), "{}", diagnostics(&output));
    assert!(
        diagnostics(&output).contains("`wrold`"),
        "{}",
        diagnostics(&output)
    );
    assert!(
        diagnostics(&output).contains("src/main.rs:2:"),
        "{}",
        diagnostics(&output)
    );
}
