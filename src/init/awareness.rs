//! LSPZ.md awareness file and CLAUDE.md reference management.

use std::fs;
use std::path::{Path, PathBuf};

use super::constants::{CLAUDE_DIR_ENV, CLAUDE_MD, LSPZ_MD, LSPZ_MD_REF, LSPZ_SLIM};
use crate::error::LspzError;
use crate::languages::generate_language_table;

/// Resolve the target directory for awareness files.
fn target_dir(global: bool) -> Result<PathBuf, LspzError> {
    if global {
        if let Ok(dir) = std::env::var(CLAUDE_DIR_ENV) {
            return Ok(PathBuf::from(dir));
        }
        dirs::home_dir()
            .map(|home| home.join(".claude"))
            .ok_or_else(|| LspzError::Config("Cannot determine home directory".into()))
    } else {
        Ok(Path::new(".").join(".claude"))
    }
}

/// Build the full LSPZ.md content (static template + dynamic language table).
fn build_lspz_md_content() -> String {
    let lang_table = generate_language_table();
    format!("{LSPZ_SLIM}\n## Mappings\n\n{lang_table}\n")
}

/// Write LSPZ.md to the target directory.
///
/// Returns `true` if the file was written, `false` if it already exists with the same content.
pub fn write_lspz_md(global: bool, dry_run: bool) -> Result<bool, LspzError> {
    let dir = target_dir(global)?;
    let path = dir.join(LSPZ_MD);
    let content = build_lspz_md_content();

    if path.exists() {
        let existing = fs::read_to_string(&path)
            .map_err(|e| LspzError::Config(format!("Failed to read {}: {e}", path.display())))?;
        if existing == content {
            return Ok(false);
        }
    }

    if dry_run {
        return Ok(true);
    }

    fs::create_dir_all(&dir)
        .map_err(|e| LspzError::Config(format!("Failed to create {}: {e}", dir.display())))?;

    fs::write(&path, &content)
        .map_err(|e| LspzError::Config(format!("Failed to write {}: {e}", path.display())))?;

    Ok(true)
}

/// Remove LSPZ.md from the target directory.
///
/// Returns `true` if the file was removed, `false` if it didn't exist.
pub fn remove_lspz_md(global: bool, dry_run: bool) -> Result<bool, LspzError> {
    let dir = target_dir(global)?;
    let path = dir.join(LSPZ_MD);

    if !path.exists() {
        return Ok(false);
    }

    if dry_run {
        return Ok(true);
    }

    fs::remove_file(&path)
        .map_err(|e| LspzError::Config(format!("Failed to remove {}: {e}", path.display())))?;

    Ok(true)
}

/// Add `@LSPZ.md` reference to CLAUDE.md if not already present.
///
/// Returns `true` if CLAUDE.md was modified, `false` if the reference already exists.
pub fn patch_claude_md_ref(global: bool, dry_run: bool) -> Result<bool, LspzError> {
    let dir = target_dir(global)?;
    let path = dir.join(CLAUDE_MD);

    if path.exists() {
        let content = fs::read_to_string(&path)
            .map_err(|e| LspzError::Config(format!("Failed to read {}: {e}", path.display())))?;

        if content.contains(LSPZ_MD_REF) {
            return Ok(false);
        }

        if dry_run {
            return Ok(true);
        }

        let new_content = format!("{content}\n{LSPZ_MD_REF}\n");
        fs::write(&path, &new_content)
            .map_err(|e| LspzError::Config(format!("Failed to write {}: {e}", path.display())))?;
    } else {
        if dry_run {
            return Ok(true);
        }

        fs::create_dir_all(&dir)
            .map_err(|e| LspzError::Config(format!("Failed to create {}: {e}", dir.display())))?;

        fs::write(&path, format!("{LSPZ_MD_REF}\n"))
            .map_err(|e| LspzError::Config(format!("Failed to write {}: {e}", path.display())))?;
    }

    Ok(true)
}

/// Remove `@LSPZ.md` reference from CLAUDE.md.
///
/// Returns `true` if CLAUDE.md was modified, `false` if the reference didn't exist.
pub fn remove_claude_md_ref(global: bool, dry_run: bool) -> Result<bool, LspzError> {
    let dir = target_dir(global)?;
    let path = dir.join(CLAUDE_MD);

    if !path.exists() {
        return Ok(false);
    }

    let content = fs::read_to_string(&path)
        .map_err(|e| LspzError::Config(format!("Failed to read {}: {e}", path.display())))?;

    if !content.contains(LSPZ_MD_REF) {
        return Ok(false);
    }

    if dry_run {
        return Ok(true);
    }

    // Remove the reference line and any trailing blank line
    let new_content = content
        .lines()
        .filter(|line| line.trim() != LSPZ_MD_REF.trim())
        .collect::<Vec<_>>()
        .join("\n");

    // Clean up trailing blank lines
    let new_content = new_content.trim_end_matches('\n');

    fs::write(path, format!("{new_content}\n"))
        .map_err(|e| LspzError::Config(format!("Failed to write CLAUDE.md: {e}")))?;

    Ok(true)
}

/// Check if LSPZ.md exists in the target directory.
pub fn lspz_md_exists(global: bool) -> bool {
    target_dir(global)
        .map(|dir| dir.join(LSPZ_MD).exists())
        .unwrap_or(false)
}

/// Check if CLAUDE.md contains the @LSPZ.md reference.
pub fn claude_md_has_ref(global: bool) -> bool {
    target_dir(global)
        .and_then(|dir| {
            let path = dir.join(CLAUDE_MD);
            if path.exists() {
                fs::read_to_string(&path)
                    .map(|c| c.contains(LSPZ_MD_REF))
                    .map_err(|e| LspzError::Config(format!("Failed to read CLAUDE.md: {e}")))
            } else {
                Ok(false)
            }
        })
        .unwrap_or(false)
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::env;
    use std::sync::Mutex;
    use tempfile::TempDir;

    static ENV_MUTEX: Mutex<()> = Mutex::new(());

    /// Helper: set LSPZ_CLAUDE_DIR to a temp dir, run the test, then restore.
    /// Uses a mutex to serialize access to the process-wide env var.
    fn with_temp_claude_dir<F: FnOnce(&Path)>(f: F) {
        let _guard = ENV_MUTEX.lock().unwrap_or_else(|e| e.into_inner());
        let dir = TempDir::new().unwrap();
        // SAFETY: Mutex guard ensures no concurrent env access.
        unsafe {
            env::set_var(CLAUDE_DIR_ENV, dir.path());
        }
        f(dir.path());
        // SAFETY: Same as above.
        unsafe {
            env::remove_var(CLAUDE_DIR_ENV);
        }
    }

    #[test]
    fn write_lspz_md_creates_file() {
        with_temp_claude_dir(|dir| {
            let changed = write_lspz_md(true, false).unwrap();
            assert!(changed);

            let path = dir.join(LSPZ_MD);
            assert!(path.exists());
            let content = fs::read_to_string(&path).unwrap();
            assert!(content.starts_with(LSPZ_SLIM));
            assert!(content.contains("Mappings"));
        });
    }

    #[test]
    fn write_lspz_md_idempotent() {
        with_temp_claude_dir(|_| {
            write_lspz_md(true, false).unwrap();
            let changed = write_lspz_md(true, false).unwrap();
            assert!(!changed);
        });
    }

    #[test]
    fn write_lspz_md_dry_run() {
        with_temp_claude_dir(|dir| {
            let changed = write_lspz_md(true, true).unwrap();
            assert!(changed);
            assert!(!dir.join(LSPZ_MD).exists());
        });
    }

    #[test]
    fn remove_lspz_md_removes_file() {
        with_temp_claude_dir(|dir| {
            write_lspz_md(true, false).unwrap();
            let removed = remove_lspz_md(true, false).unwrap();
            assert!(removed);
            assert!(!dir.join(LSPZ_MD).exists());
        });
    }

    #[test]
    fn remove_lspz_md_no_file() {
        with_temp_claude_dir(|_| {
            let removed = remove_lspz_md(true, false).unwrap();
            assert!(!removed);
        });
    }

    #[test]
    fn patch_claude_md_ref_creates_file() {
        with_temp_claude_dir(|dir| {
            let changed = patch_claude_md_ref(true, false).unwrap();
            assert!(changed);

            let content = fs::read_to_string(dir.join(CLAUDE_MD)).unwrap();
            assert!(content.contains(LSPZ_MD_REF));
        });
    }

    #[test]
    fn patch_claude_md_ref_appends_to_existing() {
        with_temp_claude_dir(|dir| {
            fs::write(dir.join(CLAUDE_MD), "# Existing content\n").unwrap();
            let changed = patch_claude_md_ref(true, false).unwrap();
            assert!(changed);

            let content = fs::read_to_string(dir.join(CLAUDE_MD)).unwrap();
            assert!(content.starts_with("# Existing content"));
            assert!(content.contains(LSPZ_MD_REF));
        });
    }

    #[test]
    fn patch_claude_md_ref_idempotent() {
        with_temp_claude_dir(|_| {
            patch_claude_md_ref(true, false).unwrap();
            let changed = patch_claude_md_ref(true, false).unwrap();
            assert!(!changed);
        });
    }

    #[test]
    fn patch_claude_md_ref_dry_run() {
        with_temp_claude_dir(|dir| {
            let changed = patch_claude_md_ref(true, true).unwrap();
            assert!(changed);
            assert!(!dir.join(CLAUDE_MD).exists());
        });
    }

    #[test]
    fn remove_claude_md_ref_removes_line() {
        with_temp_claude_dir(|dir| {
            patch_claude_md_ref(true, false).unwrap();
            let removed = remove_claude_md_ref(true, false).unwrap();
            assert!(removed);

            let content = fs::read_to_string(dir.join(CLAUDE_MD)).unwrap();
            assert!(!content.contains(LSPZ_MD_REF));
        });
    }

    #[test]
    fn remove_claude_md_ref_preserves_other_content() {
        with_temp_claude_dir(|dir| {
            fs::write(dir.join(CLAUDE_MD), "# Title\n@AGENTS.md\n").unwrap();
            patch_claude_md_ref(true, false).unwrap();
            remove_claude_md_ref(true, false).unwrap();

            let content = fs::read_to_string(dir.join(CLAUDE_MD)).unwrap();
            assert!(content.contains("# Title"));
            assert!(content.contains("@AGENTS.md"));
            assert!(!content.contains(LSPZ_MD_REF));
        });
    }

    #[test]
    fn remove_claude_md_ref_no_ref() {
        with_temp_claude_dir(|dir| {
            fs::write(dir.join(CLAUDE_MD), "# No ref here\n").unwrap();
            let removed = remove_claude_md_ref(true, false).unwrap();
            assert!(!removed);
        });
    }

    #[test]
    fn remove_claude_md_ref_no_file() {
        with_temp_claude_dir(|_| {
            let removed = remove_claude_md_ref(true, false).unwrap();
            assert!(!removed);
        });
    }

    #[test]
    fn lspz_md_exists_true() {
        with_temp_claude_dir(|_| {
            write_lspz_md(true, false).unwrap();
            assert!(lspz_md_exists(true));
        });
    }

    #[test]
    fn lspz_md_exists_false() {
        with_temp_claude_dir(|_| {
            assert!(!lspz_md_exists(true));
        });
    }

    #[test]
    fn claude_md_has_ref_true() {
        with_temp_claude_dir(|_| {
            patch_claude_md_ref(true, false).unwrap();
            assert!(claude_md_has_ref(true));
        });
    }

    #[test]
    fn claude_md_has_ref_false() {
        with_temp_claude_dir(|_| {
            assert!(!claude_md_has_ref(true));
        });
    }
}
