use std::{
    path::{Path, PathBuf},
    process::Command,
    sync::OnceLock,
};

// Integration fixtures need the build-time checkout, not an installed runtime path.
#[cfg_attr(dylint_lib = "general", allow(abs_home_path))]
fn library() -> &'static Path {
    static LIBRARY: OnceLock<PathBuf> = OnceLock::new();
    LIBRARY.get_or_init(|| {
        let root = Path::new(env!("CARGO_MANIFEST_DIR"));
        let output = Command::new("rustup")
            .current_dir(root)
            .args(["show", "active-toolchain"])
            .output()
            .expect("could not identify lint toolchain");
        assert!(output.status.success(), "{}", diagnostics(&output));
        let toolchain = String::from_utf8(output.stdout).expect("non-UTF-8 toolchain name");
        let toolchain = toolchain
            .split_whitespace()
            .next()
            .expect("missing toolchain");
        if let Some(profile) = std::env::var_os("LLVM_PROFILE_FILE") {
            // Keep the instrumented library where `cargo-llvm-cov` discovers report objects.
            let target = Path::new(&profile)
                .parent()
                .expect("missing coverage directory");
            let output = Command::new(std::env::var_os("CARGO").unwrap_or_else(|| "cargo".into()))
                .current_dir(root)
                .args(["build", "--lib", "--locked", "--target-dir"])
                .arg(target)
                .env("RUSTFLAGS", "-C instrument-coverage")
                .env_remove("CARGO_ENCODED_RUSTFLAGS")
                .env_remove("RUSTC_WRAPPER")
                .output()
                .expect("could not build instrumented lint library");
            assert!(
                output.status.success(),
                "{}",
                String::from_utf8_lossy(&output.stderr)
            );
            let directory = target.join("debug");
            let library = directory.join(format!(
                "{}dylint_spellcheck@{toolchain}{}",
                std::env::consts::DLL_PREFIX,
                std::env::consts::DLL_SUFFIX
            ));
            std::fs::copy(
                directory.join(format!(
                    "{}dylint_spellcheck{}",
                    std::env::consts::DLL_PREFIX,
                    std::env::consts::DLL_SUFFIX
                )),
                &library,
            )
            .unwrap();
            return library;
        }
        let output = Command::new(std::env::var_os("CARGO").unwrap_or_else(|| "cargo".into()))
            .current_dir(root)
            .args(["dylint", "list", "--path", "."])
            .env_remove("RUSTFLAGS")
            .output()
            .expect("could not build shared integration-test lint library");
        assert!(
            output.status.success(),
            "could not build shared lint library:\n{}",
            String::from_utf8_lossy(&output.stderr)
        );
        let metadata = cargo_metadata::MetadataCommand::new()
            .current_dir(root)
            .no_deps()
            .exec()
            .expect("could not resolve shared lint build directory");
        let library = metadata
            .target_directory
            .join("dylint/libraries")
            .join(toolchain)
            .join("release")
            .join(format!(
                "{}dylint_spellcheck@{toolchain}{}",
                std::env::consts::DLL_PREFIX,
                std::env::consts::DLL_SUFFIX,
            ))
            .into_std_path_buf();
        assert!(
            library.is_file(),
            "shared lint library not found: {}",
            library.display()
        );
        library
    })
}

pub fn lint_command(root: &Path) -> Command {
    let mut command = Command::new(std::env::var_os("CARGO").unwrap_or_else(|| "cargo".into()));
    command
        .current_dir(root)
        .env("CARGO_TERM_COLOR", "never")
        .env("CARGO_CACHE_RUSTC_INFO", "0")
        .args(["dylint", "--no-metadata", "--lib-path"])
        .arg(library())
        .args(["--", "--workspace", "--all-targets"]);
    command
}

pub fn lint(root: &Path, strict: bool) -> std::process::Output {
    let mut command = lint_command(root);
    if strict {
        command.env("RUSTFLAGS", "-D warnings");
    }
    command.output().expect("could not run consumer lint")
}

pub fn diagnostics(output: &std::process::Output) -> String {
    String::from_utf8_lossy(&output.stderr).into_owned()
}

pub fn project() -> tempfile::TempDir {
    let directory = tempfile::tempdir().unwrap();
    std::fs::create_dir(directory.path().join("src")).unwrap();
    std::fs::write(
        directory.path().join("Cargo.toml"),
        "[package]\nname = \"consumer\"\nversion = \"0.1.0\"\nedition = \"2021\"\n",
    )
    .unwrap();
    std::fs::write(directory.path().join("src/main.rs"), "fn main() {}\n").unwrap();
    directory
}
