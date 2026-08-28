#[path = "../build_support/template_inventory.rs"]
mod template_inventory;

use std::fs;

#[test]
fn collector_recurses_and_records_empty_directories() {
    let root = std::env::temp_dir().join(format!("mdp-inventory-{}", std::process::id()));
    let _ = fs::remove_dir_all(&root);
    fs::create_dir_all(root.join(".mdp/empty")).unwrap();
    fs::write(root.join(".mdp/manifest.yaml"), b"ok").unwrap();
    let entries = template_inventory::collect_tree(&root).unwrap();
    assert!(
        entries
            .iter()
            .any(|entry| entry.relative == ".mdp/empty" && entry.is_directory)
    );
    assert!(
        entries
            .iter()
            .any(|entry| entry.relative == ".mdp/manifest.yaml")
    );
    let _ = fs::remove_dir_all(root);
}

#[test]
fn validator_rejects_unsafe_duplicate_and_unsorted_entries() {
    use template_inventory::InventoryEntry;
    for entries in [
        vec![InventoryEntry {
            relative: "../escape".into(),
            is_directory: false,
        }],
        vec![
            InventoryEntry {
                relative: "a".into(),
                is_directory: false,
            },
            InventoryEntry {
                relative: "a".into(),
                is_directory: false,
            },
        ],
        vec![
            InventoryEntry {
                relative: "z".into(),
                is_directory: false,
            },
            InventoryEntry {
                relative: "a".into(),
                is_directory: false,
            },
        ],
    ] {
        assert!(template_inventory::validate_entries(&entries).is_err());
    }
}

#[cfg(unix)]
#[test]
fn collector_rejects_symlinks() {
    let root = std::env::temp_dir().join(format!("mdp-inventory-link-{}", std::process::id()));
    let _ = fs::remove_dir_all(&root);
    fs::create_dir_all(&root).unwrap();
    std::os::unix::fs::symlink("missing", root.join("link")).unwrap();
    assert!(template_inventory::collect_tree(&root).is_err());
    let _ = fs::remove_dir_all(root);
}

#[cfg(unix)]
#[test]
fn collector_rejects_non_regular_nodes() {
    let root = std::env::temp_dir().join(format!("mdp-inventory-fifo-{}", std::process::id()));
    let _ = fs::remove_dir_all(&root);
    fs::create_dir_all(&root).unwrap();
    let name = std::ffi::CString::new(root.join("pipe").to_str().unwrap()).unwrap();
    assert_eq!(unsafe { libc::mkfifo(name.as_ptr(), 0o600) }, 0);
    assert!(template_inventory::collect_tree(&root).is_err());
    let _ = fs::remove_dir_all(root);
}
