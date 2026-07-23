use serde::{Deserialize, Serialize};
use std::path::{Path, PathBuf};

pub const APP_SCHEMA_V1: u32 = 1;
pub const APP_SCHEMA_V2: u32 = 2;
pub const DEFAULT_PYTHON_ENTRY: &str = "main.py";
pub const DEFAULT_UI_ENTRY: &str = "ui.json";

/// In-app application manifest loaded from `apps/<name>/app.json`.
///
/// Web apps are self-contained HTML/CSS/JS pages rendered inside a
/// Qt WebEngine view (or a QTextBrowser fallback). CLI apps expose a
/// command that the agent executes through the existing bash tool.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AppManifest {
    /// Manifest schema. Missing means the original v1 format.
    #[serde(default = "default_schema_version")]
    pub schema_version: u32,
    /// Directory name under `apps/` (also the app identity).
    /// Filled from the directory name when loading, not required in JSON.
    #[serde(default)]
    pub name: String,
    /// `"web"` or `"cli"`.
    #[serde(rename = "type")]
    pub app_type: String,
    /// Human-readable title.
    pub title: String,
    /// Short description shown in the app launcher.
    pub description: String,
    /// Semantic version.
    #[serde(default = "default_version")]
    pub version: String,
    /// For web apps: relative path to the HTML entry point.
    #[serde(default)]
    pub entry: Option<String>,
    /// For cli apps: the command string or relative script path.
    #[serde(default)]
    pub command: Option<String>,
    /// Optional relative icon path (not yet rendered).
    #[serde(default)]
    pub icon: Option<String>,
    /// Who created the app.
    #[serde(default = "default_author")]
    pub author: String,
    /// Free-form tags.
    #[serde(default)]
    pub tags: Vec<String>,
    /// Direct process runtime for native v2 apps. Absent on v1 packages.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub runtime: Option<AppRuntimeConfig>,
    /// Declarative native UI document. Absent on legacy web/CLI apps.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub ui: Option<AppUiConfig>,
    /// Explicitly granted broker capabilities.
    #[serde(default)]
    pub capabilities: AppCapabilities,
    /// Resource and protocol envelope. OS memory enforcement is a later phase.
    #[serde(default)]
    pub limits: AppLimits,
    /// Optional project-tree node that owns this app package.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub tree_node_id: Option<String>,
    /// Give the app a grace period to acknowledge `app.stop` before killing it.
    #[serde(default = "default_safe_shutdown")]
    pub safe_shutdown: bool,
}

impl AppManifest {
    /// Resolve the Python entry while retaining the v1 top-level `entry` field.
    pub fn python_entry(&self) -> &str {
        self.runtime
            .as_ref()
            .map(|runtime| runtime.entry.as_str())
            .filter(|entry| !entry.trim().is_empty())
            .or_else(|| {
                self.entry
                    .as_deref()
                    .filter(|entry| !entry.trim().is_empty())
            })
            .unwrap_or(DEFAULT_PYTHON_ENTRY)
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AppRuntimeConfig {
    #[serde(default)]
    pub kind: AppRuntimeKind,
    #[serde(default = "default_python_entry")]
    pub entry: String,
    #[serde(default)]
    pub interpreter: PythonInterpreter,
}

impl Default for AppRuntimeConfig {
    fn default() -> Self {
        Self {
            kind: AppRuntimeKind::default(),
            entry: default_python_entry(),
            interpreter: PythonInterpreter::default(),
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, Default)]
#[serde(rename_all = "snake_case")]
pub enum AppRuntimeKind {
    #[default]
    Python,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, Default)]
pub enum PythonInterpreter {
    #[default]
    #[serde(rename = "auto")]
    Auto,
    #[serde(rename = "python")]
    Python,
    #[serde(rename = "python3")]
    Python3,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AppUiConfig {
    #[serde(default)]
    pub kind: AppUiKind,
    #[serde(default = "default_ui_entry")]
    pub entry: String,
}

impl Default for AppUiConfig {
    fn default() -> Self {
        Self {
            kind: AppUiKind::default(),
            entry: default_ui_entry(),
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, Default)]
#[serde(rename_all = "snake_case")]
pub enum AppUiKind {
    #[default]
    NativeJson,
}

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct AppCapabilities {
    #[serde(default)]
    pub gpio: Vec<GpioCapability>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct GpioCapability {
    pub alias: String,
    #[serde(default)]
    pub operations: Vec<GpioOperation>,
    #[serde(default, alias = "safe_state")]
    pub safe_value: Option<bool>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum GpioOperation {
    Configure,
    Read,
    Write,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum GpioMode {
    Input,
    Output,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AppLimits {
    /// Declared memory budget for future OS-level enforcement.
    #[serde(default = "default_memory_mb")]
    pub memory_mb: u64,
    #[serde(default = "default_max_message_bytes")]
    pub max_message_bytes: usize,
    #[serde(default = "default_shutdown_timeout_ms")]
    pub shutdown_timeout_ms: u64,
    #[serde(default = "default_max_ui_nodes")]
    pub max_ui_nodes: usize,
    #[serde(default = "default_max_ui_depth")]
    pub max_ui_depth: usize,
}

impl Default for AppLimits {
    fn default() -> Self {
        Self {
            memory_mb: default_memory_mb(),
            max_message_bytes: default_max_message_bytes(),
            shutdown_timeout_ms: default_shutdown_timeout_ms(),
            max_ui_nodes: default_max_ui_nodes(),
            max_ui_depth: default_max_ui_depth(),
        }
    }
}

fn default_schema_version() -> u32 {
    APP_SCHEMA_V1
}

fn default_python_entry() -> String {
    DEFAULT_PYTHON_ENTRY.to_string()
}

fn default_ui_entry() -> String {
    DEFAULT_UI_ENTRY.to_string()
}

fn default_memory_mb() -> u64 {
    32
}

fn default_max_message_bytes() -> usize {
    64 * 1024
}

fn default_shutdown_timeout_ms() -> u64 {
    2_000
}

fn default_max_ui_nodes() -> usize {
    128
}

fn default_max_ui_depth() -> usize {
    8
}

fn default_safe_shutdown() -> bool {
    true
}

fn default_version() -> String {
    "0.1.0".to_string()
}

fn default_author() -> String {
    "AI".to_string()
}

/// Discovered app with its absolute directory path.
#[derive(Debug, Clone, Serialize)]
pub struct AppListing {
    pub manifest: AppManifest,
    /// Absolute path to the app directory.
    pub dir: PathBuf,
    /// Absolute path to the entry file (web) or command script (cli), if resolvable.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub entry_path: Option<PathBuf>,
    /// Whether the entry file actually exists on disk.
    pub entry_exists: bool,
}

/// Return the canonical `apps/` directory inside the workspace.
pub fn apps_dir(workspace: &Path) -> PathBuf {
    workspace.join("apps")
}

/// List every valid app under `workspace/apps/`.
pub fn list_apps(workspace: &Path) -> Vec<AppListing> {
    let root = apps_dir(workspace);
    let mut apps = Vec::new();

    let Ok(entries) = std::fs::read_dir(&root) else {
        return apps;
    };

    for entry in entries.flatten() {
        let dir = entry.path();
        if !dir.is_dir() {
            continue;
        }
        let name = entry.file_name().to_string_lossy().to_string();
        if !crate::tools::paths::is_safe_app_name(&name) {
            continue;
        }
        let manifest_path = dir.join("app.json");
        let Ok(text) = std::fs::read_to_string(&manifest_path) else {
            continue;
        };
        let Ok(mut manifest) = serde_json::from_str::<AppManifest>(&text) else {
            continue;
        };
        manifest.name = name;

        let (entry_path, entry_exists) = if manifest.app_type == "web" {
            let rel = manifest
                .entry
                .as_deref()
                .unwrap_or("index.html");
            if !crate::tools::paths::is_safe_relative_file(rel) {
                (None, false)
            } else {
                let p = dir.join(rel);
                let exists = p.is_file();
                (Some(p), exists)
            }
        } else if manifest.app_type == "cli" {
            match manifest.command.as_deref() {
                Some(c) if crate::tools::paths::is_safe_relative_file(c) => {
                    let p = dir.join(c);
                    let exists = p.is_file();
                    (Some(p), exists)
                }
                _ => (None, false),
            }
        } else if manifest.app_type == "python" {
            let rel = manifest.python_entry();
            if !crate::tools::paths::is_safe_relative_file(rel) {
                (None, false)
            } else {
                let p = dir.join(rel);
                let exists = p.is_file();
                (Some(p), exists)
            }
        } else {
            (None, false)
        };

        apps.push(AppListing {
            manifest,
            dir,
            entry_path,
            entry_exists,
        });
    }

    apps.sort_by(|a, b| a.manifest.title.cmp(&b.manifest.title));
    apps
}

/// Get a single app by name. Returns `None` if the manifest is missing or invalid.
pub fn get_app(workspace: &Path, name: &str) -> Option<AppListing> {
    if !crate::tools::paths::is_safe_app_name(name) {
        return None;
    }
    let dir = match crate::tools::paths::confine_app_dir(workspace, name) {
        Ok(dir) => dir,
        Err(_) => return None,
    };
    if !dir.is_dir() {
        return None;
    }
    let manifest_path = dir.join("app.json");
    let text = std::fs::read_to_string(&manifest_path).ok()?;
    let mut manifest = serde_json::from_str::<AppManifest>(&text).ok()?;
    manifest.name = name.to_string();

    let (entry_path, entry_exists) = if manifest.app_type == "web" {
        let p = manifest
            .entry
            .as_deref()
            .map(|e| dir.join(e))
            .unwrap_or_else(|| dir.join("index.html"));
        let exists = p.is_file();
        (Some(p), exists)
    } else if manifest.app_type == "cli" {
        let p = manifest.command.as_deref().map(|c| dir.join(c));
        let exists = p.as_ref().map_or(false, |p| p.is_file());
        (p, exists)
    } else if manifest.app_type == "python" {
        let p = dir.join(manifest.python_entry());
        let exists = p.is_file();
        (Some(p), exists)
    } else {
        (None, false)
    };

    Some(AppListing {
        manifest,
        dir,
        entry_path,
        entry_exists,
    })
}

/// Read the full text of an app's entry file (web HTML or cli/python script).
pub fn read_app_entry(workspace: &Path, name: &str) -> Result<String, String> {
    if !crate::tools::paths::is_safe_app_name(name) {
        return Err("invalid app name".to_string());
    }
    let dir = crate::tools::paths::confine_app_dir(workspace, name)?;
    if !dir.is_dir() {
        return Err(format!("app directory not found: {}", dir.display()));
    }
    let manifest_path = crate::tools::paths::confine_app_entry(&dir, "app.json")?;
    let text = std::fs::read_to_string(&manifest_path)
        .map_err(|e| format!("cannot read app.json: {e}"))?;
    let manifest =
        serde_json::from_str::<AppManifest>(&text).map_err(|e| format!("invalid app.json: {e}"))?;

    let entry_rel = if manifest.app_type == "web" {
        manifest.entry.unwrap_or_else(|| "index.html".to_string())
    } else if manifest.app_type == "cli" {
        manifest
            .command
            .ok_or_else(|| "cli app has no command field".to_string())?
    } else if manifest.app_type == "python" {
        manifest.python_entry().to_string()
    } else {
        return Err(format!("unknown app type: {}", manifest.app_type));
    };

    let entry_path = crate::tools::paths::confine_app_entry(&dir, &entry_rel)?;
    std::fs::read_to_string(&entry_path)
        .map_err(|e| format!("cannot read entry file {}: {e}", entry_path.display()))
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::fs;

    fn setup_test_workspace() -> PathBuf {
        let dir =
            std::env::temp_dir().join(format!("mooncoding-apps-test-{}", uuid::Uuid::new_v4()));
        fs::create_dir_all(&dir).unwrap();
        dir
    }

    fn create_test_app(workspace: &Path, name: &str, app_json: &str, entry_content: &str) {
        let app_dir = workspace.join("apps").join(name);
        fs::create_dir_all(&app_dir).unwrap();
        fs::write(app_dir.join("app.json"), app_json).unwrap();
        fs::write(app_dir.join("index.html"), entry_content).unwrap();
    }

    #[test]
    fn list_apps_returns_empty_when_no_apps_dir() {
        let ws = setup_test_workspace();
        let apps = list_apps(&ws);
        assert!(apps.is_empty());
    }

    #[test]
    fn list_apps_finds_web_app() {
        let ws = setup_test_workspace();
        create_test_app(
            &ws,
            "hello",
            r#"{"type":"web","title":"Hello App","description":"A test app"}"#,
            "<html></html>",
        );

        let apps = list_apps(&ws);
        assert_eq!(apps.len(), 1);
        assert_eq!(apps[0].manifest.name, "hello");
        assert_eq!(apps[0].manifest.app_type, "web");
        assert!(apps[0].entry_exists);
    }

    #[test]
    fn list_apps_finds_cli_app() {
        let ws = setup_test_workspace();
        let app_dir = ws.join("apps").join("runner");
        fs::create_dir_all(&app_dir).unwrap();
        fs::write(
            app_dir.join("app.json"),
            r#"{"type":"cli","title":"Runner","description":"CLI app","command":"run.sh"}"#,
        )
        .unwrap();
        fs::write(app_dir.join("run.sh"), "#!/bin/sh\necho ok").unwrap();

        let apps = list_apps(&ws);
        assert_eq!(apps.len(), 1);
        assert_eq!(apps[0].manifest.app_type, "cli");
        assert!(apps[0].entry_exists);
    }

    #[test]
    fn get_app_returns_none_for_missing() {
        let ws = setup_test_workspace();
        assert!(get_app(&ws, "nope").is_none());
    }

    #[test]
    fn get_app_blocks_path_traversal() {
        let ws = setup_test_workspace();
        assert!(get_app(&ws, "../secret").is_none());
        assert!(get_app(&ws, "a\\..\\b").is_none());
    }

    #[test]
    fn get_app_returns_correct_web_entry() {
        let ws = setup_test_workspace();
        create_test_app(
            &ws,
            "demo",
            r#"{"type":"web","title":"Demo","description":"desc","entry":"main.html"}"#,
            "<html>demo</html>",
        );
        let app_dir = ws.join("apps").join("demo");
        fs::write(app_dir.join("main.html"), "<html>custom entry</html>").unwrap();

        let app = get_app(&ws, "demo").unwrap();
        assert_eq!(
            app.entry_path
                .unwrap()
                .file_name()
                .unwrap()
                .to_str()
                .unwrap(),
            "main.html"
        );
        assert!(app.entry_exists);
    }

    #[test]
    fn read_app_entry_returns_content() {
        let ws = setup_test_workspace();
        create_test_app(
            &ws,
            "reader",
            r#"{"type":"web","title":"Reader","description":"desc"}"#,
            "<html><body>Hello World</body></html>",
        );

        let content = read_app_entry(&ws, "reader").unwrap();
        assert!(content.contains("Hello World"));
    }

    #[test]
    fn sanitize_invalid_manifest() {
        let ws = setup_test_workspace();
        let app_dir = ws.join("apps").join("bad");
        fs::create_dir_all(&app_dir).unwrap();
        fs::write(app_dir.join("app.json"), "not json").unwrap();

        // Should not panic, just not find the app.
        assert!(get_app(&ws, "bad").is_none());
    }

    #[test]
    fn v1_manifest_keeps_legacy_defaults() {
        let manifest: AppManifest = serde_json::from_str(
            r#"{
                "type": "python",
                "title": "Legacy",
                "description": "A v1 Python app"
            }"#,
        )
        .expect("v1 manifest should parse");

        assert_eq!(manifest.schema_version, APP_SCHEMA_V1);
        assert_eq!(manifest.python_entry(), DEFAULT_PYTHON_ENTRY);
        assert!(manifest.runtime.is_none());
        assert!(manifest.ui.is_none());
        assert!(manifest.capabilities.gpio.is_empty());
        assert_eq!(manifest.limits.max_message_bytes, 64 * 1024);
        assert!(manifest.safe_shutdown);
    }

    #[test]
    fn v2_manifest_parses_native_runtime_policy() {
        let manifest: AppManifest = serde_json::from_str(
            r#"{
                "schema_version": 2,
                "type": "python",
                "title": "Traffic Lights",
                "description": "Native GPIO lesson",
                "runtime": {
                    "kind": "python",
                    "entry": "lesson.py",
                    "interpreter": "python3"
                },
                "ui": {
                    "kind": "native_json",
                    "entry": "ui.json"
                },
                "capabilities": {
                    "gpio": [{
                        "alias": "red_led",
                        "operations": ["configure", "write"],
                        "safe_state": false
                    }]
                },
                "limits": {
                    "memory_mb": 24,
                    "shutdown_timeout_ms": 750
                },
                "tree_node_id": "lesson-node",
                "safe_shutdown": true
            }"#,
        )
        .expect("v2 manifest should parse");

        assert_eq!(manifest.schema_version, APP_SCHEMA_V2);
        assert_eq!(manifest.python_entry(), "lesson.py");
        assert_eq!(
            manifest.runtime.expect("runtime").interpreter,
            PythonInterpreter::Python3
        );
        assert_eq!(manifest.ui.expect("ui").kind, AppUiKind::NativeJson);
        assert_eq!(
            manifest.capabilities.gpio[0].operations,
            vec![GpioOperation::Configure, GpioOperation::Write]
        );
        assert_eq!(manifest.capabilities.gpio[0].safe_value, Some(false));
        assert_eq!(manifest.limits.memory_mb, 24);
        assert_eq!(manifest.limits.max_message_bytes, 64 * 1024);
        assert_eq!(manifest.tree_node_id.as_deref(), Some("lesson-node"));
    }
}
