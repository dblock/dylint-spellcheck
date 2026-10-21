use cargo_metadata::Metadata;
use rustc_session::Session;
use rustc_span::Symbol;
use std::path::PathBuf;

pub fn files(
    sess: &Session,
    metadata: &Metadata,
    include: &[String],
    exclude: &[String],
) -> Result<Vec<PathBuf>, String> {
    if include.is_empty() {
        return Ok(Vec::new());
    }
    let manifest = metadata.workspace_root.join("Cargo.toml");
    let package = metadata
        .workspace_packages()
        .into_iter()
        .min_by_key(|package| (package.manifest_path != manifest, &package.manifest_path))
        .ok_or("could not find a workspace package for Markdown checks")?;
    let target = package
        .targets
        .iter()
        .filter(|target| {
            target.is_lib()
                || target.is_rlib()
                || target.is_dylib()
                || target.is_cdylib()
                || target.is_staticlib()
                || target.is_proc_macro()
                || target.is_bin()
        })
        .min_by_key(|target| {
            (
                target.is_bin(),
                !target.required_features.is_empty(),
                &target.name,
            )
        })
        .ok_or("could not find a library or binary target for Markdown checks")?;
    let source = sess
        .local_crate_source_file()
        .and_then(rustc_span::RealFileName::into_local_path)
        .map(|path| path.canonicalize())
        .transpose()
        .map_err(|error| {
            format!("could not resolve source path for Markdown ownership: {error}")
        })?;
    let target_source = target.src_path.canonicalize().map_err(|error| {
        format!("could not resolve target path for Markdown ownership: {error}")
    })?;
    if sess.opts.test || source.as_deref() != Some(target_source.as_path()) {
        return Ok(Vec::new());
    }
    let selection = crate::files::select(metadata.workspace_root.as_std_path(), include, exclude)?;
    for input in selection.inputs {
        sess.file_depinfo
            .lock()
            .insert(Symbol::intern(&input.to_string_lossy()));
    }
    for path in &selection.files {
        crate::markdown::supports(path)?;
    }
    Ok(selection.files.into_iter().collect())
}
