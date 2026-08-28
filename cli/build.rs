#[path = "build_support/template_inventory.rs"]
mod template_inventory;

use std::env;
use std::fs;
use std::path::PathBuf;

fn main() -> std::io::Result<()> {
    let manifest = PathBuf::from(env::var_os("CARGO_MANIFEST_DIR").expect("manifest dir"));
    let parent = manifest.join("../plugin/assets/templates");
    println!("cargo:rerun-if-changed={}", parent.display());
    let mut roots = fs::read_dir(&parent)?.collect::<Result<Vec<_>, _>>()?;
    roots.sort_by(|a, b| a.file_name().cmp(&b.file_name()));
    let out = PathBuf::from(env::var_os("OUT_DIR").expect("out dir")).join("template_inventory.rs");
    fs::write(
        &out,
        "pub(crate) static EMBEDDED_ROOTS: &[crate::template_registry::EmbeddedTemplateRoot] = &[\n",
    )?;
    for root in roots {
        let metadata = fs::symlink_metadata(root.path())?;
        if metadata.file_type().is_symlink() {
            return Err(std::io::Error::new(
                std::io::ErrorKind::InvalidData,
                "template asset root may not be a symlink",
            ));
        }
        if !metadata.is_dir() {
            return Err(std::io::Error::new(
                std::io::ErrorKind::InvalidData,
                "template asset root must be a directory",
            ));
        }
        let key_name = root.file_name();
        let key = key_name.to_str().ok_or_else(|| {
            std::io::Error::new(
                std::io::ErrorKind::InvalidData,
                "template asset root must be UTF-8",
            )
        })?;
        template_inventory::validate_root_key(key)?;
        let entries = template_inventory::collect_tree(&root.path())?;
        for entry in &entries {
            println!(
                "cargo:rerun-if-changed={}",
                root.path().join(&entry.relative).display()
            );
        }
        template_inventory::emit_inventory(&root.path(), &entries, &out, &key)?;
    }
    use std::io::Write;
    let mut file = fs::OpenOptions::new().append(true).open(&out)?;
    file.write_all(b"];\n")?;
    Ok(())
}
