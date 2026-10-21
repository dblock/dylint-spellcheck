use kotoshu::dict::Dictionary;
use serde::Deserialize;
use std::{
    collections::{BTreeSet, HashSet},
    fs::{self, OpenOptions},
    io::{self, Write},
    path::{Path, PathBuf},
};

#[derive(Default, Deserialize)]
#[serde(default, deny_unknown_fields)]
pub struct Config {
    pub aff: Option<PathBuf>,
    pub dic: Option<PathBuf>,
    pub vocabulary_files: Vec<PathBuf>,
    pub ignored_words: HashSet<String>,
}

pub struct Checker {
    dictionary: Dictionary,
    pub ignored_words: HashSet<String>,
    pub files: Vec<PathBuf>,
    pub missing: Vec<PathBuf>,
}

impl Checker {
    pub fn load(settings: Config, root: &Path) -> Result<Self, String> {
        let mut files = Vec::new();
        let mut missing = Vec::new();
        let dictionary = match (&settings.aff, &settings.dic) {
            (None, None) => Dictionary::load_from_sources(
                include_str!("../data/en/en.aff"),
                include_str!("../data/en/en.dic"),
            ),
            (Some(aff), Some(dic)) => {
                files.extend([root.join(aff), root.join(dic)]);
                Dictionary::load(&files[0], &files[1])
            }
            _ => return Err("aff and dic must be configured together".into()),
        }
        .map_err(|error| format!("could not load spelling dictionary: {error}"))?;
        let mut ignored_words = settings.ignored_words;
        for file in settings.vocabulary_files {
            let path = root.join(file);
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
                    ignored_words.insert(line.to_owned());
                }
            }
            files.push(path);
        }
        Ok(Self {
            dictionary,
            ignored_words,
            files,
            missing,
        })
    }

    pub fn correct(&self, word: &str) -> bool {
        self.ignored_words.contains(word) || self.dictionary.correct(word)
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
        self.dictionary
            .suggest(word, 3)
            .into_iter()
            .map(|suggestion| suggestion.word)
            .collect()
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
    fn resolves_dictionary_and_vocabulary_paths_from_config_root() {
        let root = Path::new(env!("CARGO_MANIFEST_DIR")).join("tests/fixtures");
        let checker = Checker::load(
            Config {
                aff: Some("en.aff".into()),
                dic: Some("en.dic".into()),
                vocabulary_files: vec!["vocabulary.txt".into()],
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
    fn rejects_incomplete_or_unreadable_configuration() {
        let root = Path::new(env!("CARGO_MANIFEST_DIR"));
        assert!(
            Checker::load(
                Config {
                    aff: Some("missing".into()),
                    ..Config::default()
                },
                root
            )
            .err()
            .unwrap()
            .contains("configured together")
        );
        assert!(
            Checker::load(
                Config {
                    aff: Some("missing".into()),
                    dic: Some("missing".into()),
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
                    vocabulary_files: vec!["nonexistent-parent/missing.txt".into()],
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
    fn missing_vocabulary_collects_but_existing_vocabulary_is_read_only() {
        let directory = tempfile::tempdir().unwrap();
        let settings = || Config {
            vocabulary_files: vec![".spellcheck.txt".into()],
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
                    vocabulary_files: vec!["invalid.txt".into()],
                    ..Config::default()
                },
                directory.path()
            )
            .is_err()
        );
    }

    #[test]
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
