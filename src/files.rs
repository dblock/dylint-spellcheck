use glob::{MatchOptions, Pattern};
use std::{
    collections::BTreeSet,
    path::{Path, PathBuf},
};
use walkdir::WalkDir;

pub struct Selection {
    pub files: BTreeSet<PathBuf>,
    pub inputs: BTreeSet<PathBuf>,
}

fn pattern(root: &Path, value: &str) -> Result<(PathBuf, Pattern), String> {
    let mut path = PathBuf::new();
    for component in root.join(value).components() {
        match component {
            std::path::Component::CurDir => {}
            std::path::Component::ParentDir => {
                path.pop();
            }
            _ => path.push(component.as_os_str()),
        }
    }
    let text = path
        .to_str()
        .ok_or_else(|| format!("non-UTF-8 file pattern: {}", path.display()))?;
    let pattern =
        Pattern::new(text).map_err(|error| format!("invalid file pattern {value:?}: {error}"))?;
    Ok((path, pattern))
}

pub fn select(root: &Path, include: &[String], exclude: &[String]) -> Result<Selection, String> {
    let exclusions = exclude
        .iter()
        .map(|value| pattern(root, value).map(|(_, pattern)| pattern))
        .collect::<Result<Vec<_>, _>>()?;
    let options = MatchOptions {
        require_literal_separator: true,
        ..MatchOptions::new()
    };
    let excluded = |path: &Path| {
        exclusions
            .iter()
            .any(|pattern| pattern.matches_path_with(path, options))
    };
    let mut selection = Selection {
        files: BTreeSet::new(),
        inputs: BTreeSet::new(),
    };
    for value in include {
        let (path, matcher) = pattern(root, value)?;
        let mut base = PathBuf::new();
        let mut depth = 0;
        let mut recursive = false;
        for component in path.components() {
            let text = component.as_os_str().to_string_lossy();
            if depth != 0 || text.contains(['*', '?', '[']) {
                depth += 1;
                recursive |= text == "**";
            } else {
                base.push(component.as_os_str());
            }
        }
        if depth == 0 {
            if !excluded(&path) {
                add(&mut selection, &path)?;
            }
            continue;
        }
        // A missing fixed prefix may appear later; track its nearest existing ancestor.
        let mut ancestor = base.as_path();
        while !ancestor.exists() {
            ancestor = ancestor
                .parent()
                .ok_or_else(|| format!("could not resolve pattern root: {}", base.display()))?;
        }
        selection.inputs.insert(ancestor.to_owned());
        if !base.exists() {
            continue;
        }
        let walk = WalkDir::new(&base).max_depth(if recursive { usize::MAX } else { depth });
        for entry in walk
            .into_iter()
            .filter_entry(|entry| !excluded(entry.path()))
        {
            let entry =
                entry.map_err(|error| format!("could not discover spelling inputs: {error}"))?;
            let path = entry.path();
            if entry.file_type().is_dir() {
                selection.inputs.insert(path.to_owned());
            } else if matcher.matches_path_with(path, options) {
                add(&mut selection, path)?;
            }
        }
    }
    Ok(selection)
}

fn add(selection: &mut Selection, path: &Path) -> Result<(), String> {
    let canonical = path
        .canonicalize()
        .map_err(|error| format!("could not read spelling input {}: {error}", path.display()))?;
    if !canonical.is_file() {
        return Err(format!("spelling input is not a file: {}", path.display()));
    }
    selection
        .inputs
        .extend([path.to_owned(), canonical.clone()]);
    selection.files.insert(canonical);
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::fs;

    #[test]
    #[cfg_attr(dylint_lib = "general", allow(non_thread_safe_call_in_test))]
    fn selects_patterns_excludes_files_and_tracks_empty_or_missing_directories() {
        let directory = tempfile::tempdir().unwrap();
        let root = directory.path();
        fs::create_dir_all(root.join("docs/nested")).unwrap();
        fs::write(root.join("docs/readme.md"), "").unwrap();
        fs::write(root.join("docs/nested/skip.md"), "").unwrap();
        let selection = select(
            root,
            &["docs/**/*.md".into(), "absent/**/*.markdown".into()],
            &["docs/nested/**".into()],
        )
        .unwrap();
        assert_eq!(selection.files.len(), 1);
        assert!(selection.inputs.contains(&root.join("docs/nested")));
        assert!(selection.inputs.contains(root));
        assert!(select(root, &["[".into()], &[]).is_err());
        assert!(select(root, &["missing.md".into()], &[]).is_err());
        assert!(select(root, &["docs".into()], &[]).is_err());
        let selection = select(
            root,
            &["./docs/readme.md".into()],
            &["docs/readme.md".into()],
        )
        .unwrap();
        assert!(selection.files.is_empty());
        let selection = select(root, &["docs/../docs/readme.md".into()], &[]).unwrap();
        assert!(
            selection
                .files
                .contains(&root.join("docs/readme.md").canonicalize().unwrap())
        );
    }
}
