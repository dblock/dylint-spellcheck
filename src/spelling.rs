use kotoshu::dict::Dictionary;
use serde::Deserialize;
use std::{
    collections::{BTreeMap, BTreeSet, HashSet},
    fs::{self, OpenOptions},
    io::{self, Write},
    path::{Path, PathBuf},
};

#[derive(Deserialize)]
#[serde(default, deny_unknown_fields)]
pub struct Config {
    pub dictionaries: Dictionaries,
    pub include: Vec<String>,
    pub exclude: Vec<String>,
}

#[derive(Deserialize)]
#[serde(untagged)]
pub enum Dictionaries {
    Paths(Vec<PathBuf>),
    Languages(BTreeMap<String, Vec<PathBuf>>),
}

struct Inputs {
    bundled_english: bool,
    dictionaries: BTreeSet<(PathBuf, PathBuf)>,
    vocabulary: BTreeSet<PathBuf>,
    tracked: BTreeSet<PathBuf>,
}

impl Default for Config {
    fn default() -> Self {
        Self {
            dictionaries: Dictionaries::Paths(vec!["crate:dylint-spellcheck/data/en".into()]),
            include: Vec::new(),
            exclude: Vec::new(),
        }
    }
}

impl Config {
    pub fn input_files(&self, root: &Path) -> Result<Vec<PathBuf>, String> {
        let inputs = self.inputs(root)?;
        Ok(inputs
            .vocabulary
            .into_iter()
            .chain(
                inputs
                    .dictionaries
                    .into_iter()
                    .flat_map(|(aff, dic)| [aff, dic]),
            )
            .chain(inputs.tracked)
            .collect())
    }

    fn inputs(&self, root: &Path) -> Result<Inputs, String> {
        let groups: Vec<_> = match &self.dictionaries {
            Dictionaries::Paths(entries) => vec![(None, entries)],
            Dictionaries::Languages(languages) => languages
                .iter()
                .map(|(language, entries)| (Some(language.as_str()), entries))
                .collect(),
        };
        let mut inputs = Inputs {
            bundled_english: false,
            dictionaries: BTreeSet::new(),
            vocabulary: BTreeSet::new(),
            tracked: BTreeSet::new(),
        };
        let mut metadata = None;
        for (language, entries) in groups {
            if let Some(language) = language
                && (language.is_empty()
                    || !language
                        .bytes()
                        .all(|byte| byte.is_ascii_alphanumeric() || byte == b'_' || byte == b'-'))
            {
                return Err(format!("invalid dictionary language: {language:?}"));
            }
            for entry in entries {
                if let Some(resource) = entry.to_string_lossy().strip_prefix("crate:") {
                    let (name, relative) = resource.split_once('/').ok_or_else(|| format!("dictionary crate reference requires a resource path: crate:{resource}/<path>"))?;
                    if name.is_empty()
                        || !name.bytes().all(|byte| {
                            byte.is_ascii_alphanumeric() || byte == b'_' || byte == b'-'
                        })
                        || relative.is_empty()
                        || Path::new(relative)
                            .components()
                            .any(|component| !matches!(component, std::path::Component::Normal(_)))
                    {
                        return Err(format!(
                            "invalid dictionary crate reference: crate:{resource}"
                        ));
                    }
                    if metadata.is_none() {
                        metadata = Some(
                            cargo_metadata::MetadataCommand::new()
                                .current_dir(root)
                                .env_remove("RUSTC_WORKSPACE_WRAPPER")
                                .env_remove("RUSTC_WRAPPER")
                                .env_remove("RUSTC")
                                .features(cargo_metadata::CargoOpt::AllFeatures)
                                .other_options(vec![
                                    "--offline".into(),
                                    "--locked".into(),
                                    "--filter-platform".into(),
                                    rustc_session::config::host_tuple().into(),
                                ])
                                .exec()
                                .map_err(|error| {
                                    format!("could not resolve dictionary crates offline: {error}")
                                })?,
                        );
                    }
                    let metadata = metadata.as_ref().unwrap();
                    let packages: Vec<_> = metadata
                        .packages
                        .iter()
                        .filter(|package| package.name.as_str() == name)
                        .collect();
                    inputs.tracked.extend(
                        metadata
                            .packages
                            .iter()
                            .map(|package| package.manifest_path.clone().into_std_path_buf()),
                    );
                    inputs.tracked.insert(
                        metadata
                            .workspace_root
                            .join("Cargo.lock")
                            .into_std_path_buf(),
                    );
                    if packages.is_empty() && name == "dylint-spellcheck" {
                        if relative != "data/en" {
                            return Err(format!(
                                "unknown packaged dictionary path: crate:{resource} (available: crate:dylint-spellcheck/data/en)"
                            ));
                        }
                        if let Some(language) = language
                            && language != "en"
                        {
                            return Err(format!(
                                "crate:dylint-spellcheck has no dictionary for language {language}"
                            ));
                        }
                        inputs.bundled_english = true;
                    } else {
                        if packages.len() != 1 {
                            return Err(format!(
                                "dictionary crate {name:?} must resolve to exactly one local package; found {}",
                                packages.len()
                            ));
                        }
                        let package = packages[0];
                        let crate_root = package.manifest_path.parent().unwrap();
                        let path = crate_root.join(relative).into_std_path_buf();
                        let canonical = path.canonicalize().map_err(|error| {
                            format!(
                                "could not read crate dictionary resource {}: {error}",
                                path.display()
                            )
                        })?;
                        let canonical_root = crate_root.canonicalize().map_err(|error| {
                            format!("could not resolve dictionary crate root: {error}")
                        })?;
                        if !canonical.starts_with(canonical_root) {
                            return Err(format!(
                                "dictionary crate resource escapes its package: crate:{resource}"
                            ));
                        }
                        if canonical
                            .extension()
                            .is_some_and(|extension| extension == "txt")
                            && canonical.is_file()
                        {
                            inputs.vocabulary.insert(canonical);
                            inputs.tracked.insert(path);
                        } else {
                            inputs.tracked.insert(path);
                            inputs.add_directory(&canonical, language)?;
                        }
                    }
                    continue;
                }
                let path = root.join(entry);
                if path.extension().is_some_and(|extension| extension == "txt") && !path.is_dir() {
                    inputs.vocabulary.insert(path);
                } else {
                    if !path.is_dir() {
                        return Err(format!(
                            "dictionary entry must be a .txt file or an existing directory: {}",
                            path.display()
                        ));
                    }
                    inputs.add_directory(&path, language)?;
                }
            }
        }
        Ok(inputs)
    }
}

impl Inputs {
    fn add_directory(&mut self, path: &Path, language: Option<&str>) -> Result<(), String> {
        self.tracked.insert(path.to_owned());
        if let Some(language) = language {
            self.dictionaries.insert((
                path.join(format!("{language}.aff")),
                path.join(format!("{language}.dic")),
            ));
            return Ok(());
        }
        let mut found = false;
        for entry in fs::read_dir(path).map_err(|error| {
            format!(
                "could not read dictionary directory {}: {error}",
                path.display()
            )
        })? {
            let file = entry
                .map_err(|error| {
                    format!(
                        "could not read dictionary entry in {}: {error}",
                        path.display()
                    )
                })?
                .path();
            if file
                .extension()
                .is_some_and(|extension| extension == "aff" || extension == "dic")
            {
                self.dictionaries
                    .insert((file.with_extension("aff"), file.with_extension("dic")));
                found = true;
            }
        }
        if !found {
            return Err(format!(
                "dictionary directory contains no .aff/.dic pairs: {}",
                path.display()
            ));
        }
        Ok(())
    }
}

pub struct Checker {
    dictionaries: Vec<Dictionary>,
    vocabulary: HashSet<String>,
    pub files: Vec<PathBuf>,
    pub missing: Vec<PathBuf>,
}

impl Checker {
    pub fn load(settings: Config, root: &Path) -> Result<Self, String> {
        let inputs = settings.inputs(root)?;
        let mut files = Vec::new();
        let mut missing = Vec::new();
        let mut dictionaries = Vec::new();
        if inputs.bundled_english {
            dictionaries.push(
                Dictionary::load_from_sources(
                    include_str!("../data/en/en.aff"),
                    include_str!("../data/en/en.dic"),
                )
                .map_err(|error| format!("could not load bundled English dictionary: {error}"))?,
            );
        }
        for (aff, dic) in inputs.dictionaries {
            dictionaries.push(Dictionary::load(&aff, &dic).map_err(|error| {
                format!(
                    "could not load spelling dictionary {}: {error}",
                    dic.display()
                )
            })?);
            files.extend([aff, dic]);
        }
        let mut vocabulary = HashSet::new();
        for path in inputs.vocabulary {
            let text = match fs::read_to_string(&path) {
                Ok(text) => text,
                Err(error)
                    if error.kind() == io::ErrorKind::NotFound
                        && fs::symlink_metadata(&path)
                            .is_err_and(|error| error.kind() == io::ErrorKind::NotFound) =>
                {
                    if path.extension().is_none_or(|extension| extension != "txt")
                        || !path.parent().is_some_and(Path::is_dir)
                    {
                        return Err(format!(
                            "missing vocabulary must be a .txt file with an existing parent: {}",
                            path.display()
                        ));
                    }
                    missing.push(path);
                    continue;
                }
                Err(error) => {
                    return Err(format!(
                        "could not read vocabulary {}: {error}",
                        path.display()
                    ));
                }
            };
            for line in text.lines().map(str::trim) {
                if !line.is_empty() && !line.starts_with('#') {
                    vocabulary.insert(line.to_owned());
                }
            }
            files.push(path);
        }
        Ok(Self {
            dictionaries,
            vocabulary,
            files,
            missing,
        })
    }

    pub fn correct(&self, word: &str) -> bool {
        self.vocabulary.contains(word)
            || self
                .dictionaries
                .iter()
                .any(|dictionary| dictionary.correct(word))
    }

    pub fn collect_candidates(
        vocabulary: &Path,
        words: &BTreeSet<String>,
    ) -> Result<PathBuf, String> {
        let candidate = vocabulary.with_extension("tmp");
        let lock_path = candidate.with_extension("tmp.lock");
        let result = (|| -> io::Result<()> {
            // Lock a separate file: replacing the candidate must not replace the locked inode.
            let lock = OpenOptions::new()
                .create(true)
                .truncate(false)
                .read(true)
                .write(true)
                .open(lock_path)?;
            lock.lock()?;
            let mut collected = match fs::read_to_string(&candidate) {
                Ok(text) => text
                    .lines()
                    .filter(|line| !line.is_empty())
                    .map(String::from)
                    .collect::<BTreeSet<_>>(),
                Err(error) if error.kind() == io::ErrorKind::NotFound => BTreeSet::new(),
                Err(error) => return Err(error),
            };
            collected.extend(words.iter().cloned());
            let mut temporary = tempfile::NamedTempFile::new_in(candidate.parent().unwrap())?;
            for word in collected {
                writeln!(temporary, "{word}")?;
            }
            temporary.as_file().sync_all()?;
            temporary.persist(&candidate).map_err(|error| error.error)?;
            Ok(())
        })();
        result.map_err(|error| {
            format!(
                "could not collect vocabulary {}: {error}",
                candidate.display()
            )
        })?;
        Ok(candidate)
    }

    pub fn suggestions(&self, word: &str) -> Vec<String> {
        let mut suggestions = Vec::new();
        for dictionary in &self.dictionaries {
            for suggestion in dictionary.suggest(word, 3) {
                if self.correct(&suggestion.word) && !suggestions.contains(&suggestion.word) {
                    suggestions.push(suggestion.word);
                }
                if suggestions.len() == 3 {
                    return suggestions;
                }
            }
        }
        suggestions
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn defaults_to_real_english_and_reports_misspellings() {
        let checker = Checker::load(Config::default(), Path::new(".")).unwrap();
        assert!(checker.correct("spelling"));
        assert!(!checker.correct("speling"));
        assert!(checker.suggestions("speling").contains(&"spelling".into()));
    }

    #[test]
    fn explicit_resources_replace_defaults_and_language_keys_do_not_load_dictionaries() {
        let root = Path::new(".");
        for dictionaries in [
            Dictionaries::Paths(Vec::new()),
            Dictionaries::Languages(BTreeMap::from([("en".into(), Vec::new())])),
        ] {
            let checker = Checker::load(
                Config {
                    dictionaries,
                    ..Config::default()
                },
                root,
            )
            .unwrap();
            assert!(!checker.correct("hello"));
            assert!(checker.suggestions("helo").is_empty());
        }
        let checker = Checker::load(
            Config {
                dictionaries: Dictionaries::Languages(BTreeMap::from([(
                    "en".into(),
                    vec!["crate:dylint-spellcheck/data/en".into()],
                )])),
                ..Config::default()
            },
            root,
        )
        .unwrap();
        assert!(checker.correct("hello"));
        for dictionaries in [
            Dictionaries::Paths(vec!["crate:unknown/data/en".into()]),
            Dictionaries::Languages(BTreeMap::from([(
                "de".into(),
                vec!["crate:dylint-spellcheck/data/en".into()],
            )])),
        ] {
            assert!(
                Checker::load(
                    Config {
                        dictionaries,
                        ..Config::default()
                    },
                    root
                )
                .is_err()
            );
        }
    }
    #[test]
    fn resolves_dictionary_and_vocabulary_paths_from_config_root() {
        let root = Path::new(env!("CARGO_MANIFEST_DIR")).join("tests/fixtures");
        let checker = Checker::load(
            Config {
                dictionaries: Dictionaries::Languages(BTreeMap::from([(
                    "en".into(),
                    vec![".".into(), "vocabulary.txt".into()],
                )])),
                ..Config::default()
            },
            &root,
        )
        .unwrap();
        assert!(checker.correct("Kotoshu"));
        assert!(!checker.correct("kotoshu"));
        assert!(checker.correct("hello"));
        assert!(!checker.correct("spelling"));
        assert_eq!(checker.files.len(), 3);
    }

    #[test]
    #[cfg_attr(dylint_lib = "general", allow(non_thread_safe_call_in_test))]
    fn local_crate_resources_override_embedded_english_without_loading_it() {
        let directory = tempfile::tempdir().unwrap();
        let root = directory.path();
        fs::create_dir(root.join("src")).unwrap();
        fs::write(root.join("src/lib.rs"), "").unwrap();
        let manifest = "[package]\nname = \"consumer\"\nversion = \"0.1.0\"\nedition = \"2021\"\n";
        fs::write(root.join("Cargo.toml"), manifest).unwrap();
        let lock = || {
            cargo_metadata::MetadataCommand::new()
                .current_dir(root)
                .other_options(vec!["--offline".into()])
                .exec()
                .unwrap();
        };
        lock();
        let settings = || Config {
            dictionaries: Dictionaries::Paths(vec!["crate:dylint-spellcheck/data/en".into()]),
            ..Config::default()
        };
        let embedded = Checker::load(settings(), root).unwrap();
        assert!(embedded.correct("spelling"));
        assert!(embedded.suggestions("speling").contains(&"spelling".into()));
        assert!(!embedded.correct("Localword"));

        fs::create_dir_all(root.join("local/src")).unwrap();
        fs::create_dir_all(root.join("local/data/en")).unwrap();
        fs::write(root.join("local/src/lib.rs"), "").unwrap();
        fs::write(
            root.join("local/Cargo.toml"),
            "[package]\nname = \"dylint-spellcheck\"\nversion = \"0.0.1\"\nedition = \"2021\"\n",
        )
        .unwrap();
        fs::write(root.join("local/data/en/en.aff"), "SET UTF-8\n").unwrap();
        fs::write(root.join("local/data/en/en.dic"), "1\nLocalword\n").unwrap();
        fs::write(root.join("Cargo.toml"), format!("{manifest}\n[dependencies]\nrenamed = {{ package = \"dylint-spellcheck\", path = \"local\" }}\n")).unwrap();
        lock();
        let local = Checker::load(settings(), root).unwrap();
        assert!(local.correct("Localword"));
        assert!(!local.correct("spelling"));
        assert!(!local.suggestions("speling").contains(&"spelling".into()));
        assert!(
            local
                .files
                .contains(&root.join("local/data/en/en.dic").canonicalize().unwrap())
        );

        fs::remove_file(root.join("local/data/en/en.dic")).unwrap();
        assert!(Checker::load(settings(), root).is_err());
        fs::write(root.join("local/data/en/en.dic"), "1\nLocalword\n").unwrap();

        // Non-special crate names and user-selected resource paths use the same resolver.
        fs::write(
            root.join("local/Cargo.toml"),
            "[package]\nname = \"other-dictionaries\"\nversion = \"0.0.1\"\nedition = \"2021\"\n",
        )
        .unwrap();
        fs::write(root.join("Cargo.toml"), format!("{manifest}\n[dependencies]\nrenamed = {{ package = \"other-dictionaries\", path = \"local\" }}\n")).unwrap();
        fs::rename(root.join("local/data/en"), root.join("local/resources")).unwrap();
        fs::write(root.join("local/words.txt"), "Projectword\n").unwrap();
        lock();
        let checker = Checker::load(
            Config {
                dictionaries: Dictionaries::Languages(BTreeMap::from([(
                    "en".into(),
                    vec![
                        "crate:other-dictionaries/resources".into(),
                        "crate:other-dictionaries/words.txt".into(),
                    ],
                )])),
                ..Config::default()
            },
            root,
        )
        .unwrap();
        assert!(checker.correct("Localword"));
        assert!(checker.correct("Projectword"));
        assert!(!checker.correct("spelling"));
        fs::remove_file(root.join("local/resources/en.dic")).unwrap();
        assert!(
            Checker::load(
                Config {
                    dictionaries: Dictionaries::Paths(vec![
                        "crate:other-dictionaries/resources".into()
                    ]),
                    ..Config::default()
                },
                root
            )
            .is_err()
        );
        for reference in [
            "crate:other-dictionaries",
            "crate:missing/resources",
            "crate:other-dictionaries/../src",
            "crate:other-dictionaries/missing",
        ] {
            assert!(
                Checker::load(
                    Config {
                        dictionaries: Dictionaries::Paths(vec![reference.into()]),
                        ..Config::default()
                    },
                    root
                )
                .is_err(),
                "{reference}"
            );
        }
    }

    #[test]
    fn rejects_incomplete_or_unreadable_configuration() {
        let root = Path::new(env!("CARGO_MANIFEST_DIR"));
        assert!(
            Checker::load(
                Config {
                    dictionaries: Dictionaries::Paths(vec!["missing".into()]),
                    ..Config::default()
                },
                root
            )
            .err()
            .unwrap()
            .contains("existing directory")
        );
        assert!(
            Checker::load(
                Config {
                    dictionaries: Dictionaries::Languages(BTreeMap::from([(
                        "de".into(),
                        vec!["tests/fixtures".into()]
                    )])),
                    ..Config::default()
                },
                root
            )
            .err()
            .unwrap()
            .contains("could not load")
        );
        assert!(
            Checker::load(
                Config {
                    dictionaries: Dictionaries::Paths(vec![
                        "nonexistent-parent/missing.txt".into()
                    ]),
                    ..Config::default()
                },
                root
            )
            .err()
            .unwrap()
            .contains("existing parent")
        );
    }

    #[test]
    #[cfg_attr(dylint_lib = "general", allow(non_thread_safe_call_in_test))]
    fn discovers_multiple_languages_and_accumulates_dictionaries_and_vocabulary() {
        let directory = tempfile::tempdir().unwrap();
        let root = directory.path();
        fs::create_dir(root.join("dicts")).unwrap();
        for (language, word) in [("en", "Customword"), ("de", "Zusatzwort")] {
            fs::write(root.join(format!("dicts/{language}.aff")), "SET UTF-8\n").unwrap();
            fs::write(
                root.join(format!("dicts/{language}.dic")),
                format!("1\n{word}\n"),
            )
            .unwrap();
        }
        fs::write(root.join("words.txt"), "# accepted\n\nProjectword\n").unwrap();
        for dictionaries in [
            Dictionaries::Paths(vec!["dicts".into(), "words.txt".into()]),
            Dictionaries::Languages(BTreeMap::from([
                ("en".into(), vec!["dicts".into(), "words.txt".into()]),
                ("de".into(), vec!["dicts".into()]),
            ])),
        ] {
            let config = Config {
                dictionaries,
                ..Config::default()
            };
            assert!(
                config
                    .input_files(root)
                    .unwrap()
                    .contains(&root.join("dicts"))
            );
            let checker = Checker::load(config, root).unwrap();
            for word in ["Customword", "Zusatzwort", "Projectword"] {
                assert!(checker.correct(word), "{word}");
            }
            assert!(!checker.correct("projectword"));
            assert!(!checker.correct("spelling"));
        }
        fs::remove_file(root.join("dicts/de.aff")).unwrap();
        assert!(
            Checker::load(
                Config {
                    dictionaries: Dictionaries::Paths(vec!["dicts".into()]),
                    ..Config::default()
                },
                root
            )
            .err()
            .unwrap()
            .contains("could not load")
        );
    }

    #[test]
    #[cfg_attr(dylint_lib = "general", allow(non_thread_safe_call_in_test))]
    fn collects_sorted_candidates_without_losing_concurrent_writes() {
        let directory = tempfile::tempdir().unwrap();
        let vocabulary = directory.path().join(".spellcheck.txt");
        std::thread::scope(|scope| {
            for word in ["Kotoshu", "speling", "Kotoshu", "helo"] {
                let vocabulary = &vocabulary;
                scope.spawn(move || {
                    Checker::collect_candidates(vocabulary, &BTreeSet::from([word.into()])).unwrap()
                });
            }
        });
        assert_eq!(
            fs::read_to_string(vocabulary.with_extension("tmp")).unwrap(),
            "Kotoshu\nhelo\nspeling\n"
        );
        assert!(!vocabulary.exists());
        fs::remove_file(vocabulary.with_extension("tmp")).unwrap();
        Checker::collect_candidates(&vocabulary, &BTreeSet::from(["fresh".into()])).unwrap();
        assert_eq!(
            fs::read_to_string(vocabulary.with_extension("tmp")).unwrap(),
            "fresh\n"
        );
    }

    #[test]
    #[cfg_attr(dylint_lib = "general", allow(non_thread_safe_call_in_test))]
    fn missing_vocabulary_collects_but_existing_vocabulary_is_read_only() {
        let directory = tempfile::tempdir().unwrap();
        let settings = || Config {
            dictionaries: Dictionaries::Paths(vec![".spellcheck.txt".into()]),
            ..Config::default()
        };
        let checker = Checker::load(settings(), directory.path()).unwrap();
        assert_eq!(
            checker.missing,
            vec![directory.path().join(".spellcheck.txt")]
        );
        fs::write(&checker.missing[0], "Kotoshu\n").unwrap();
        let checker = Checker::load(settings(), directory.path()).unwrap();
        assert!(checker.missing.is_empty());
        assert!(checker.correct("Kotoshu"));
        fs::create_dir(directory.path().join("invalid.txt")).unwrap();
        assert!(
            Checker::load(
                Config {
                    dictionaries: Dictionaries::Paths(vec!["invalid.txt".into()]),
                    ..Config::default()
                },
                directory.path()
            )
            .is_err()
        );
    }

    #[test]
    #[cfg_attr(dylint_lib = "general", allow(non_thread_safe_call_in_test))]
    fn reports_candidate_write_failures() {
        let directory = tempfile::tempdir().unwrap();
        fs::create_dir(directory.path().join("words.tmp")).unwrap();
        assert!(
            Checker::collect_candidates(&directory.path().join("words.txt"), &BTreeSet::new())
                .unwrap_err()
                .contains("could not collect vocabulary")
        );
    }
}
