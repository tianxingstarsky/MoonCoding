//! Workspace path confinement helpers shared by tools.
//!
//! Rule: every filesystem path the agent touches must resolve under the
//! current workspace after canonicalize. Never fall back to a non-canonical
//! path for `starts_with` checks — that allows `..` escapes.

use std::path::{Component, Path, PathBuf};

/// Canonicalize `workspace`, failing closed if it cannot be resolved.
pub fn canonicalize_workspace(workspace: &Path) -> Result<PathBuf, String> {
    workspace.canonicalize().map_err(|error| {
        format!(
            "workspace unavailable ({}): {error}",
            workspace.display()
        )
    })
}

/// Resolve `user` under `workspace` and require the result stays inside it.
///
/// - Relative paths join the workspace root.
/// - Absolute paths are allowed only if they canonicalize inside the workspace.
/// - Canonicalize failure is a hard error (no `unwrap_or` fallback).
pub fn confine_to_workspace(workspace: &Path, user: &Path) -> Result<PathBuf, String> {
    let workspace = canonicalize_workspace(workspace)?;
    let candidate = if user.is_absolute() {
        user.to_path_buf()
    } else {
        workspace.join(user)
    };
    let canonical = candidate.canonicalize().map_err(|error| {
        format!(
            "path unavailable ({}): {error}",
            candidate.display()
        )
    })?;
    if !path_inside(&canonical, &workspace) {
        return Err(format!(
            "refused: {} is outside workspace {}",
            canonical.display(),
            workspace.display()
        ));
    }
    Ok(canonical)
}

/// Like [`confine_to_workspace`], but the path need not exist yet.
///
/// Validates components for `..` / absolute escapes, then ensures the nearest
/// existing ancestor is still inside the workspace and the final joined path
/// stays under that ancestor prefix.
pub fn confine_new_path(workspace: &Path, user: &Path) -> Result<PathBuf, String> {
    if user
        .components()
        .any(|component| matches!(component, Component::ParentDir))
    {
        return Err("refused: path must not contain '..'".to_string());
    }
    let workspace = canonicalize_workspace(workspace)?;
    let joined = if user.is_absolute() {
        user.to_path_buf()
    } else {
        workspace.join(user)
    };

    // Walk up until an existing ancestor can be canonicalized.
    let mut probe = joined.clone();
    let mut suffix = Vec::new();
    loop {
        if let Ok(canonical_ancestor) = probe.canonicalize() {
            if !path_inside(&canonical_ancestor, &workspace) {
                return Err(format!(
                    "refused: {} is outside workspace {}",
                    joined.display(),
                    workspace.display()
                ));
            }
            let mut rebuilt = canonical_ancestor;
            for part in suffix.into_iter().rev() {
                rebuilt.push(part);
            }
            if !path_inside(&rebuilt, &workspace) && rebuilt != workspace {
                // rebuilt may not exist; compare by stripping verbatim and checking prefix.
                if !path_inside_prefix(&rebuilt, &workspace) {
                    return Err(format!(
                        "refused: {} is outside workspace {}",
                        rebuilt.display(),
                        workspace.display()
                    ));
                }
            }
            return Ok(rebuilt);
        }
        match probe.file_name() {
            Some(name) => {
                suffix.push(name.to_os_string());
                if !probe.pop() {
                    break;
                }
            }
            None => break,
        }
    }
    Err(format!(
        "path unavailable ({}): no resolvable ancestor inside workspace",
        joined.display()
    ))
}

/// True if `path` is `root` or a descendant (handles Windows `\\?\` prefixes).
pub fn path_inside(path: &Path, root: &Path) -> bool {
    if path == root || path.starts_with(root) {
        return true;
    }
    let path = strip_verbatim(path);
    let root = strip_verbatim(root);
    path == root || path.starts_with(&root)
}

fn path_inside_prefix(path: &Path, root: &Path) -> bool {
    path_inside(path, root)
}

fn strip_verbatim(path: &Path) -> PathBuf {
    let text = path.to_string_lossy();
    if let Some(rest) = text.strip_prefix(r"\\?\") {
        PathBuf::from(rest)
    } else {
        path.to_path_buf()
    }
}

/// App directory names: `a-z`, `0-9`, hyphen only. No path separators, no `..`.
pub fn is_safe_app_name(name: &str) -> bool {
    !name.is_empty()
        && name.len() <= 64
        && name
            .chars()
            .all(|c| c.is_ascii_lowercase() || c.is_ascii_digit() || c == '-')
}

/// Relative file path inside an app package (no `..`, no absolute, single file tree).
pub fn is_safe_relative_file(value: &str) -> bool {
    let path = Path::new(value);
    !value.trim().is_empty()
        && !path.is_absolute()
        && path.components().all(|component| {
            matches!(component, Component::Normal(_) | Component::CurDir)
        })
}

/// Resolve `workspace/apps/<name>` and require it stays under `workspace/apps`.
pub fn confine_app_dir(workspace: &Path, name: &str) -> Result<PathBuf, String> {
    if !is_safe_app_name(name) {
        return Err(
            "invalid app name: use a-z, 0-9, hyphens only (max 64 chars)".to_string(),
        );
    }
    let workspace = canonicalize_workspace(workspace)?;
    let apps_root = workspace.join("apps");
    let apps_canon = if apps_root.exists() {
        apps_root.canonicalize().map_err(|error| {
            format!("apps directory unavailable ({}): {error}", apps_root.display())
        })?
    } else {
        // Creating the first app: ensure apps/ is under workspace once created.
        confine_new_path(&workspace, Path::new("apps"))?
    };
    let app_dir = apps_canon.join(name);
    if app_dir.exists() {
        let canonical = app_dir.canonicalize().map_err(|error| {
            format!("app directory unavailable ({}): {error}", app_dir.display())
        })?;
        if !path_inside(&canonical, &apps_canon) {
            return Err(format!(
                "refused: app path {} escapes apps/",
                canonical.display()
            ));
        }
        Ok(canonical)
    } else {
        if !path_inside_prefix(&app_dir, &apps_canon) {
            return Err("refused: app path escapes apps/".to_string());
        }
        Ok(app_dir)
    }
}

/// Join a relative entry under an already-confined app directory.
pub fn confine_app_entry(app_dir: &Path, entry: &str) -> Result<PathBuf, String> {
    if !is_safe_relative_file(entry) {
        return Err("entry must be a relative file inside the app directory".to_string());
    }
    let joined = app_dir.join(entry);
    if joined.exists() {
        let canonical = joined.canonicalize().map_err(|error| {
            format!("entry unavailable ({}): {error}", joined.display())
        })?;
        if !path_inside(&canonical, app_dir) {
            return Err(format!(
                "refused: entry {} escapes app directory",
                canonical.display()
            ));
        }
        Ok(canonical)
    } else if path_inside_prefix(&joined, app_dir) {
        Ok(joined)
    } else {
        Err("refused: entry escapes app directory".to_string())
    }
}

/// Reject path-like args that try to leave the workspace via `..` or absolute roots.
pub fn arg_looks_like_escape(arg: &str) -> bool {
    if arg.contains('\0') {
        return true;
    }
    let path = Path::new(arg);
    if path.is_absolute() {
        return true;
    }
    if cfg!(windows) {
        let bytes = arg.as_bytes();
        if bytes.len() >= 2 && bytes[1] == b':' {
            return true;
        }
        if arg.starts_with('\\') {
            return true;
        }
    }
    path.components()
        .any(|component| matches!(component, Component::ParentDir))
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::fs;

    fn temp_ws() -> PathBuf {
        let dir = std::env::temp_dir().join(format!("mc-paths-{}", uuid::Uuid::new_v4()));
        fs::create_dir_all(&dir).unwrap();
        dir
    }

    #[test]
    fn rejects_parent_dir_escape() {
        let ws = temp_ws();
        let outside = ws.parent().unwrap().join("outside-secret.txt");
        fs::write(&outside, "x").unwrap();
        let result = confine_to_workspace(&ws, Path::new("../outside-secret.txt"));
        let _ = fs::remove_file(&outside);
        let _ = fs::remove_dir_all(&ws);
        assert!(result.is_err());
    }

    #[test]
    fn accepts_workspace_relative_file() {
        let ws = temp_ws();
        fs::write(ws.join("ok.txt"), "hi").unwrap();
        let result = confine_to_workspace(&ws, Path::new("ok.txt"));
        let _ = fs::remove_dir_all(&ws);
        assert!(result.is_ok());
    }

    #[test]
    fn app_name_and_entry_rules() {
        assert!(is_safe_app_name("led-toggle"));
        assert!(!is_safe_app_name("../x"));
        assert!(!is_safe_app_name("a/b"));
        assert!(is_safe_relative_file("main.py"));
        assert!(!is_safe_relative_file("../main.py"));
        assert!(!is_safe_relative_file(r"C:\temp\x.py"));
    }
}
