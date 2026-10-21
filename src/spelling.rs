use kotoshu::dict::Dictionary;
use serde::Deserialize;
use std::{
    collections::HashSet,
    fs,
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
}

impl Checker {
    pub fn load(settings: Config, root: &Path) -> Result<Self, String> {
        let mut files = Vec::new();
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
            let text = fs::read_to_string(&path).map_err(|error| {
                format!("could not read vocabulary {}: {error}", path.display())
            })?;
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
        })
    }

    pub fn correct(&self, word: &str) -> bool {
        self.ignored_words.contains(word) || self.dictionary.correct(word)
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
                    vocabulary_files: vec!["missing.txt".into()],
                    ..Config::default()
                },
                root
            )
            .err()
            .unwrap()
            .contains("could not read vocabulary")
        );
    }
}
