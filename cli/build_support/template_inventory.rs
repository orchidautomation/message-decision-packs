use std::fs;
use std::io;
use std::path::Path;

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct InventoryEntry {
    pub relative: String,
    pub is_directory: bool,
}

fn valid_component(component: &str) -> bool {
    !component.is_empty()
        && component != "."
        && component != ".."
        && !component.contains('/')
        && !component.contains('\\')
}

pub fn validate_relative(relative: &str) -> io::Result<()> {
    if relative.is_empty() || relative.starts_with('/') || relative.contains('\\') {
        return Err(io::Error::new(
            io::ErrorKind::InvalidData,
            "unsafe relative path",
        ));
    }
    if relative.split('/').any(|part| !valid_component(part)) {
        return Err(io::Error::new(
            io::ErrorKind::InvalidData,
            "unsafe relative path",
        ));
    }
    Ok(())
}

pub fn validate_root_key(key: &str) -> io::Result<()> {
    if key.is_empty() || key == "." || key == ".." || key.contains('/') || key.contains('\\') {
        return Err(io::Error::new(
            io::ErrorKind::InvalidData,
            "unsafe template root key",
        ));
    }
    Ok(())
}

pub fn validate_entries(entries: &[InventoryEntry]) -> io::Result<()> {
    let mut sorted = entries.to_vec();
    sorted.sort_by(|a, b| a.relative.cmp(&b.relative));
    if sorted != entries {
        return Err(io::Error::new(
            io::ErrorKind::InvalidData,
            "inventory is not sorted",
        ));
    }
    for (index, entry) in entries.iter().enumerate() {
        validate_relative(&entry.relative)?;
        if entries[..index]
            .iter()
            .any(|previous| previous.relative == entry.relative)
        {
            return Err(io::Error::new(
                io::ErrorKind::InvalidData,
                "duplicate inventory entry",
            ));
        }
    }
    Ok(())
}

pub fn collect_tree(root: &Path) -> io::Result<Vec<InventoryEntry>> {
    fn walk(root: &Path, current: &Path, result: &mut Vec<InventoryEntry>) -> io::Result<()> {
        let mut children = fs::read_dir(current)?.collect::<Result<Vec<_>, _>>()?;
        children.sort_by(|a, b| a.file_name().cmp(&b.file_name()));
        for child in children {
            let path = child.path();
            let metadata = fs::symlink_metadata(&path)?;
            let relative = path
                .strip_prefix(root)
                .map_err(|_| io::Error::other("tree escape"))?;
            let relative = relative
                .to_str()
                .ok_or_else(|| io::Error::new(io::ErrorKind::InvalidData, "non-UTF-8 asset path"))?
                .replace('\\', "/");
            validate_relative(&relative)?;
            if metadata.file_type().is_symlink() {
                return Err(io::Error::new(
                    io::ErrorKind::InvalidData,
                    format!("symlink asset: {relative}"),
                ));
            }
            if metadata.is_dir() {
                result.push(InventoryEntry {
                    relative,
                    is_directory: true,
                });
                walk(root, &path, result)?;
            } else if metadata.is_file() {
                result.push(InventoryEntry {
                    relative,
                    is_directory: false,
                });
            } else {
                return Err(io::Error::new(
                    io::ErrorKind::InvalidData,
                    format!("non-regular asset: {relative}"),
                ));
            }
        }
        Ok(())
    }
    let mut result = Vec::new();
    walk(root, root, &mut result)?;
    result.sort_by(|a, b| {
        a.relative
            .cmp(&b.relative)
            .then_with(|| a.is_directory.cmp(&b.is_directory))
    });
    validate_entries(&result)?;
    Ok(result)
}

pub fn emit_inventory(
    root: &Path,
    entries: &[InventoryEntry],
    output: &Path,
    root_key: &str,
) -> io::Result<()> {
    validate_entries(entries)?;
    let mut source = String::from("&[\n");
    for entry in entries {
        if entry.is_directory {
            source.push_str(&format!("    crate::template_registry::EmbeddedTemplateEntry {{ relative: {:?}, bytes: &[], kind: \"directory\", is_directory: true }},\n", entry.relative));
        } else {
            let path = root.join(entry.relative.replace('/', std::path::MAIN_SEPARATOR_STR));
            source.push_str(&format!("    crate::template_registry::EmbeddedTemplateEntry {{ relative: {:?}, bytes: include_bytes!({:?}), kind: {:?}, is_directory: false }},\n", entry.relative, path.to_string_lossy(), file_kind(&entry.relative)));
        }
    }
    source.push_str("]");
    let _ = root_key;
    let mut module = fs::read_to_string(output).unwrap_or_default();
    module.push_str(&format!(
        "    crate::template_registry::EmbeddedTemplateRoot {{ key: {:?}, entries: {source} }},\n",
        root_key
    ));
    fs::write(output, module)
}

fn file_kind(path: &str) -> &'static str {
    if path.ends_with(".json") {
        "json-file"
    } else if path.ends_with(".md") {
        "markdown-file"
    } else {
        "yaml-file"
    }
}
