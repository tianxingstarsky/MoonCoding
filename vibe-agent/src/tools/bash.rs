use async_trait::async_trait;
use serde_json::{json, Value};
use std::path::{Path, PathBuf};
use std::time::Instant;
use tokio::process::Command;

use super::paths::{arg_looks_like_escape, confine_to_workspace};
use super::{Tool, ToolContext, ToolResult};

const MAX_OUTPUT_BYTES: usize = 1024 * 1024; // 1 MiB
const TIMEOUT_SECS: u64 = 60;

const ALLOWLIST_HINT: &str = "Allowed examples: cargo test|check|build|clippy; cmake --build …; \
ctest; ninja; make; pytest; python|python3|py <script.py>; python -m pytest …; \
npm|pnpm|yarn test|run test|run lint|run build; go test; dotnet test|build; \
mvn/gradle test|build. Block integrity: use vibe tool action=verify (not this tool). \
Copy the `evidence_command:` line into tree evidence.command when marking completed.";

pub struct BashTool;

#[async_trait]
impl Tool for BashTool {
    fn name(&self) -> &str {
        "verify_command"
    }
    fn description(&self) -> &str {
        "Run one allowlisted build/test/lint/run program inside the workspace only \
         (no shell, no pipes, no `..`). Typical: command=python args=[\"apps/x/main.py\"]; \
         command=cargo args=[\"test\"]. On success, output includes evidence_command — \
         copy that exact value into tree evidence.command (kind is filled from the log). \
         python/cargo are resolved from PATH plus common toolchain dirs (MSYS2, ~/.cargo)."
    }
    fn parameters(&self) -> Value {
        json!({
            "type": "object",
            "properties": {
                "command": {
                    "type": "string",
                    "description": "Bare executable name: cargo, cmake, ctest, ninja, python, python3, py, pytest, …"
                },
                "args": {
                    "type": "array",
                    "items": {"type": "string"},
                    "description": "Arguments as separate strings, e.g. [\"apps/demo/main.py\"] or [\"test\", \"--workspace\"]"
                },
                "workdir": {"type": "string", "description": "Subdirectory inside workspace (default: workspace root)"},
                "timeout": {"type": "integer", "description": "Timeout in ms (default 60000)"}
            },
            "required": ["command"]
        })
    }
    async fn execute(&self, args: Value, ctx: &ToolContext) -> ToolResult {
        let command = args.get("command").and_then(|v| v.as_str()).unwrap_or("");
        let command_args: Vec<&str> = args
            .get("args")
            .and_then(Value::as_array)
            .map(|values| values.iter().filter_map(Value::as_str).collect())
            .unwrap_or_default();
        let workdir_str = args.get("workdir").and_then(|v| v.as_str());
        let timeout_ms = args
            .get("timeout")
            .and_then(|v| v.as_u64())
            .unwrap_or(TIMEOUT_SECS * 1000);

        let cwd = match workdir_str {
            Some(dir) => match confine_to_workspace(&ctx.workspace, Path::new(dir)) {
                Ok(path) => path,
                Err(reason) => {
                    return ToolResult {
                        output: format!("refused: {reason}"),
                        exit_code: 126,
                        duration_ms: 0,
                        truncated: false,
                    };
                }
            },
            None => match confine_to_workspace(&ctx.workspace, Path::new(".")) {
                Ok(path) => path,
                Err(reason) => {
                    return ToolResult {
                        output: format!("refused: {reason}"),
                        exit_code: 126,
                        duration_ms: 0,
                        truncated: false,
                    };
                }
            },
        };

        let verification_kind = match validate_command(command, &command_args, &ctx.workspace) {
            Ok(kind) => kind,
            Err(reason) => {
                return ToolResult {
                    output: format!("refused: {reason}. {ALLOWLIST_HINT}"),
                    exit_code: 126,
                    duration_ms: 0,
                    truncated: false,
                };
            }
        };
        let command_identity = json!({
            "program": command,
            "args": &command_args,
        })
        .to_string();

        let start = Instant::now();
        let program = resolve_program(command, &ctx.vibe_exe);
        let mut cmd = Command::new(&program);
        cmd.args(&command_args);
        cmd.current_dir(&cwd)
            .stdin(std::process::Stdio::null())
            .stdout(std::process::Stdio::piped())
            .stderr(std::process::Stdio::piped());
        cmd.kill_on_drop(true);
        cmd.env("VIBE_TEST", "1");
        apply_verification_path(&mut cmd, &ctx.vibe_exe);

        let child = match cmd.spawn() {
            Ok(c) => c,
            Err(e) => {
                // Retry python-family alternates when the primary name is missing.
                if let Some(alt) = first_available_alternate(command, &ctx.vibe_exe) {
                    let mut retry = Command::new(&alt);
                    retry.args(&command_args);
                    retry
                        .current_dir(&cwd)
                        .stdin(std::process::Stdio::null())
                        .stdout(std::process::Stdio::piped())
                        .stderr(std::process::Stdio::piped());
                    retry.kill_on_drop(true);
                    retry.env("VIBE_TEST", "1");
                    apply_verification_path(&mut retry, &ctx.vibe_exe);
                    match retry.spawn() {
                        Ok(c) => c,
                        Err(e2) => {
                            return ToolResult {
                                output: format!(
                                    "spawn err: {e} (also tried {}): {e2}. \
                                     Install the toolchain or ensure it is on PATH. {ALLOWLIST_HINT}",
                                    alt.display()
                                ),
                                exit_code: -1,
                                duration_ms: 0,
                                truncated: false,
                            };
                        }
                    }
                } else {
                    return ToolResult {
                        output: format!(
                            "spawn err: {e}. Program `{command}` not found on PATH \
                             (checked MSYS2/cargo common dirs). {ALLOWLIST_HINT}"
                        ),
                        exit_code: -1,
                        duration_ms: 0,
                        truncated: false,
                    };
                }
            }
        };
        let out = tokio::time::timeout(
            std::time::Duration::from_millis(timeout_ms),
            child.wait_with_output(),
        )
        .await;
        let duration_ms = start.elapsed().as_millis() as u64;

        let result = match out {
            Ok(Ok(output)) => {
                let mut stdout = output.stdout;
                let mut stderr = output.stderr;
                let mut truncated = false;
                if stdout.len() > MAX_OUTPUT_BYTES {
                    stdout.truncate(MAX_OUTPUT_BYTES);
                    truncated = true;
                }
                if stderr.len() > MAX_OUTPUT_BYTES {
                    stderr.truncate(MAX_OUTPUT_BYTES);
                    truncated = true;
                }
                let code = output.status.code().unwrap_or(-1);
                let mut text = String::new();
                text.push_str(&format!("cwd: {}\n", cwd.display()));
                text.push_str(&format!("evidence_command: {command_identity}\n"));
                text.push_str(&format!(
                    "evidence_kind: {verification_kind} (use this or omit; tree auto-fills from log)\n"
                ));
                text.push_str(&format!("exit {}\n", code));
                if !stdout.is_empty() {
                    text.push_str(&format!(
                        "--- stdout ---\n{}\n",
                        crate::encoding_util::decode_console_bytes(&stdout)
                    ));
                }
                if !stderr.is_empty() {
                    text.push_str(&format!(
                        "--- stderr ---\n{}\n",
                        crate::encoding_util::decode_console_bytes(&stderr)
                    ));
                }
                if truncated {
                    text.push_str("(output truncated at 1 MiB)\n");
                }
                if code == 0 {
                    text.push_str(
                        "OK: cite evidence_command above in tree update_node evidence.command \
                         to mark completed (human form like `python apps/x/main.py` also works).\n",
                    );
                } else if text.contains("SyntaxError") || text.contains("IndentationError") {
                    text.push_str(
                        "\nHint: map the error line to a block with vibe action=lookup path=<file> \
                         line=<N> (projection lines include `# === vibe:seq=` markers), then \
                         action=read seq=<from lookup>.\n",
                    );
                }
                ToolResult {
                    output: text,
                    exit_code: code,
                    duration_ms,
                    truncated,
                }
            }
            Ok(Err(e)) => ToolResult {
                output: format!("spawn err: {}", e),
                exit_code: -1,
                duration_ms,
                truncated: false,
            },
            Err(_) => ToolResult {
                output: format!("timeout after {}ms", timeout_ms),
                exit_code: 124,
                duration_ms,
                truncated: false,
            },
        };
        if let Ok(mut log) = ctx.command_log.write() {
            log.push(super::CommandExecution {
                command: command_identity,
                exit_code: result.exit_code,
                tool: "verify_command".to_string(),
                verification_kind: verification_kind.to_string(),
                working_directory: cwd,
                completed_at: chrono::Utc::now().to_rfc3339(),
            });
        }
        result
    }
}

fn validate_command(
    command: &str,
    args: &[&str],
    workspace: &Path,
) -> Result<&'static str, String> {
    if command.is_empty()
        || command.contains('/')
        || command.contains('\\')
        || command.chars().any(char::is_whitespace)
    {
        return Err("command must be a bare executable name".to_string());
    }
    let first = args.first().copied().unwrap_or_default();
    let cmd = command.to_ascii_lowercase();
    let kind = match cmd.as_str() {
        "cargo" => match first {
            "test" => Some("test"),
            "check" | "build" => Some("build"),
            "clippy" => Some("lint"),
            _ => None,
        },
        "cmake" if first == "--build" => Some("build"),
        "ctest" | "pytest" => Some("test"),
        "ninja" | "make" => Some("build"),
        "python" | "python3" | "py"
            if args.get(0) == Some(&"-m") && args.get(1) == Some(&"pytest") =>
        {
            Some("test")
        }
        "python" | "python3" | "py"
            if args.get(0) == Some(&"-m") && args.get(1) == Some(&"unittest") =>
        {
            Some("test")
        }
        // Micro-app / script smoke: completion-eligible as `run`.
        "python" | "python3" | "py" if args.iter().any(|a| a.ends_with(".py")) => Some("run"),
        "npm" | "pnpm" | "yarn" => {
            if first == "test" || (first == "run" && args.get(1) == Some(&"test")) {
                Some("test")
            } else if first == "run" && args.get(1) == Some(&"lint") {
                Some("lint")
            } else if first == "run"
                && args
                    .get(1)
                    .is_some_and(|name| matches!(*name, "check" | "build"))
            {
                Some("build")
            } else {
                None
            }
        }
        "go" if first == "test" => Some("test"),
        "dotnet" if first == "test" => Some("test"),
        "dotnet" if first == "build" => Some("build"),
        "mvn" | "mvnw" if args.iter().any(|arg| matches!(*arg, "test" | "verify")) => Some("test"),
        "gradle" | "gradlew" if args.iter().any(|arg| *arg == "test") => Some("test"),
        "gradle" | "gradlew" if args.iter().any(|arg| matches!(*arg, "check" | "build")) => {
            Some("build")
        }
        "git" if matches!(first, "status" | "diff" | "log" | "show") => Some("diagnostic"),
        "rustc" if first == "--version" => Some("diagnostic"),
        "python" | "python3" | "py" if first == "--version" => Some("diagnostic"),
        _ => None,
    };
    let kind = kind.ok_or_else(|| {
        "program or subcommand is not in the verification allowlist".to_string()
    })?;

    for arg in args {
        if arg.contains('\0')
            || matches!(*arg, ">" | ">>" | "<" | "|" | "||" | "&&" | ";" | "&" | "`")
        {
            return Err(
                "shell operators and source-mutating output/config flags are not accepted"
                    .to_string(),
            );
        }
        if is_blocked_flag(arg) {
            return Err(format!(
                "flag `{arg}` can redirect work outside the workspace and is not accepted"
            ));
        }
        // Path-like arguments must stay inside the workspace (or be bare flags/values).
        if looks_like_path_arg(arg) {
            if arg_looks_like_escape(arg) {
                return Err(format!(
                    "argument `{arg}` escapes the workspace (absolute path or '..')"
                ));
            }
            // Existing files must canonicalize inside workspace; missing paths still
            // reject `..` / absolute via arg_looks_like_escape above.
            if Path::new(arg).extension().is_some() || arg.contains('/') || arg.contains('\\') {
                if let Err(reason) = confine_to_workspace(workspace, Path::new(arg)) {
                    // Allow non-existent relative paths without `..` for build targets.
                    if reason.contains("path unavailable") && !arg_looks_like_escape(arg) {
                        continue;
                    }
                    return Err(reason);
                }
            }
        }
    }

    // Python scripts must exist under the workspace (prefer apps/).
    if matches!(cmd.as_str(), "python" | "python3" | "py") {
        if let Some(script) = args.iter().find(|arg| arg.ends_with(".py")) {
            confine_to_workspace(workspace, Path::new(script))?;
        }
    }

    Ok(kind)
}

fn is_blocked_flag(arg: &str) -> bool {
    let base = arg.split('=').next().unwrap_or(arg);
    matches!(
        base,
        "--out"
            | "--output"
            | "-o"
            | "--fix"
            | "--target-dir"
            | "--config"
            | "-f"
            | "--file"
            | "--makefile"
            | "-C"
            | "--directory"
            | "--manifest-path"
            | "--project"
            | "--project-directory"
            | "--prefix"
            | "--root"
            | "--workdir"
            | "--cwd"
    )
}

fn looks_like_path_arg(arg: &str) -> bool {
    if arg.starts_with('-') {
        return arg.contains('=') && arg_looks_like_escape(arg.split_once('=').unwrap().1);
    }
    arg.contains('/')
        || arg.contains('\\')
        || arg.contains("..")
        || arg.ends_with(".py")
        || arg.ends_with(".rs")
        || arg.ends_with(".toml")
        || arg.ends_with(".json")
}

fn toolchain_bin_dirs(vibe_exe: &Path) -> Vec<PathBuf> {
    let mut dirs = Vec::new();
    if let Some(parent) = vibe_exe.parent() {
        dirs.push(parent.to_path_buf());
        // build/vibe-ui or build/rust-target/... → also try sibling bins
        if let Some(grand) = parent.parent() {
            dirs.push(grand.join("bin"));
        }
    }
    let mut candidates: Vec<PathBuf> = vec![
        PathBuf::from(r"D:\msys2\ucrt64\bin"),
        PathBuf::from(r"D:\msys2\mingw64\bin"),
        PathBuf::from(r"D:\msys2\usr\bin"),
        PathBuf::from(r"C:\msys64\ucrt64\bin"),
        PathBuf::from(r"C:\msys64\mingw64\bin"),
        PathBuf::from(r"C:\msys64\usr\bin"),
        PathBuf::from("/usr/bin"),
        PathBuf::from("/usr/local/bin"),
    ];
    if let Ok(home) = std::env::var("USERPROFILE").or_else(|_| std::env::var("HOME")) {
        let home = PathBuf::from(home);
        candidates.push(home.join(".cargo").join("bin"));
        candidates.push(home.join("AppData").join("Local").join("Programs").join("Python"));
    }
    if let Ok(cargo_home) = std::env::var("CARGO_HOME") {
        candidates.push(PathBuf::from(cargo_home).join("bin"));
    }
    if let Ok(msys) = std::env::var("MSYSTEM_PREFIX") {
        candidates.push(PathBuf::from(msys).join("bin"));
    }
    for c in candidates {
        if c.is_dir() {
            dirs.push(c.clone());
            // Nested Python installs: …/Python/Python311/
            if c.file_name().is_some_and(|n| n == "Python") {
                if let Ok(entries) = std::fs::read_dir(&c) {
                    for entry in entries.flatten() {
                        let p = entry.path();
                        if p.is_dir() {
                            dirs.push(p);
                        }
                    }
                }
            }
        }
    }
    dirs
}

fn apply_verification_path(cmd: &mut Command, vibe_exe: &Path) {
    let mut paths = toolchain_bin_dirs(vibe_exe);
    if let Some(current) = std::env::var_os("PATH") {
        paths.extend(std::env::split_paths(&current));
    }
    if let Ok(joined) = std::env::join_paths(paths) {
        cmd.env("PATH", joined);
    }
}

fn program_candidates(name: &str) -> Vec<String> {
    let mut names = vec![name.to_string()];
    if cfg!(windows) && !name.ends_with(".exe") {
        names.push(format!("{name}.exe"));
    }
    names
}

fn resolve_program(name: &str, vibe_exe: &Path) -> PathBuf {
    let names = program_candidates(name);
    let mut search = toolchain_bin_dirs(vibe_exe);
    if let Some(path) = std::env::var_os("PATH") {
        search.extend(std::env::split_paths(&path));
    }
    for dir in &search {
        for n in &names {
            let candidate = dir.join(n);
            if candidate.is_file() {
                return candidate;
            }
        }
    }
    PathBuf::from(name)
}

fn first_available_alternate(name: &str, vibe_exe: &Path) -> Option<PathBuf> {
    let alts: &[&str] = match name.to_ascii_lowercase().as_str() {
        "python" => &["python3", "py"],
        "python3" => &["python", "py"],
        "py" => &["python", "python3"],
        _ => &[],
    };
    for alt in alts {
        let resolved = resolve_program(alt, vibe_exe);
        if resolved.is_file() || which_exists(alt) {
            return Some(resolved);
        }
    }
    None
}

fn which_exists(name: &str) -> bool {
    if let Some(path) = std::env::var_os("PATH") {
        for dir in std::env::split_paths(&path) {
            for n in program_candidates(name) {
                if dir.join(n).is_file() {
                    return true;
                }
            }
        }
    }
    false
}

#[cfg(test)]
mod tests {
    use super::validate_command;
    use std::fs;
    use std::path::PathBuf;

    fn temp_ws() -> PathBuf {
        let dir = std::env::temp_dir().join(format!("mc-bash-{}", uuid::Uuid::new_v4()));
        fs::create_dir_all(&dir).unwrap();
        dir
    }

    #[test]
    fn verification_policy_rejects_shell_and_source_mutation() {
        let ws = temp_ws();
        assert!(validate_command("cargo", &["test", "--workspace"], &ws).is_ok());
        assert!(validate_command("cargo test", &[], &ws).is_err());
        assert!(validate_command("powershell", &["-Command", "Remove-Item", "src"], &ws).is_err());
        assert!(validate_command("cargo", &["clippy", "--fix"], &ws).is_err());
        assert!(validate_command("git", &["status", "&&", "del", "src"], &ws).is_err());
        assert!(validate_command("make", &["-C", "/tmp"], &ws).is_err());
        assert!(validate_command("python", &["../evil.py"], &ws).is_err());
        let _ = fs::remove_dir_all(&ws);
    }

    #[test]
    fn python_script_must_live_in_workspace() {
        let ws = temp_ws();
        fs::create_dir_all(ws.join("apps").join("demo")).unwrap();
        fs::write(ws.join("apps").join("demo").join("main.py"), "print(1)\n").unwrap();
        assert_eq!(
            validate_command("python", &["apps/demo/main.py"], &ws).unwrap(),
            "run"
        );
        assert_eq!(
            validate_command("py", &["apps/demo/main.py"], &ws).unwrap(),
            "run"
        );
        let _ = fs::remove_dir_all(&ws);
    }
}
