pub mod protocol;
pub mod sandbox;

use anyhow::{anyhow, bail, Context, Result};
use serde::{Deserialize, Serialize};
use serde_json::Value;
use std::env;
use std::path::{Component, Path, PathBuf};
use std::process::{ExitStatus, Stdio};
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, Mutex, RwLock};
use std::time::Duration;
use tokio::io::BufReader;
use tokio::process::{Child, ChildStderr, ChildStdin, ChildStdout, Command};
use tokio::sync::{broadcast, mpsc, oneshot, Mutex as AsyncMutex};
use tokio::task::JoinHandle;
use tokio::time::timeout;

use crate::apps::{AppListing, AppRuntimeKind, PythonInterpreter};
use crate::capabilities::{GpioBackend, GpioBroker, GpioRequest, MockGpioBackend};
use protocol::{
    read_app_message, read_bounded_line, write_host_message, AppToHostMessage, HostToAppMessage,
    ProtocolError, APP_PROTOCOL_VERSION, MAX_JSONL_LINE_BYTES,
};
use sandbox::{
    current_host_pid, process_alive, unix_now, ReclaimReport, RuntimeLease, RuntimeSandbox,
};

const EVENT_CHANNEL_CAPACITY: usize = 128;
const CONTROL_CHANNEL_CAPACITY: usize = 32;
const READER_JOIN_TIMEOUT: Duration = Duration::from_millis(500);
const MAX_SHUTDOWN_TIMEOUT: Duration = Duration::from_secs(10);

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "type", rename_all = "snake_case")]
pub enum AppRuntimeEvent {
    Started {
        app_id: String,
        instance_id: String,
        interpreter: String,
    },
    UiInit {
        app_id: String,
        instance_id: String,
        ui: Value,
    },
    UiPatch {
        app_id: String,
        instance_id: String,
        patch: Value,
    },
    Log {
        app_id: String,
        instance_id: String,
        level: String,
        message: String,
    },
    Error {
        app_id: String,
        instance_id: String,
        message: String,
    },
    Stopped {
        app_id: String,
        instance_id: String,
        reason: String,
        exit_code: Option<i32>,
        forced: bool,
    },
    GpioRequest {
        app_id: String,
        instance_id: String,
        request_id: String,
        alias: String,
        operation: crate::apps::GpioOperation,
    },
}

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
#[serde(tag = "state", rename_all = "snake_case")]
pub enum AppRuntimeStatus {
    #[default]
    Idle,
    Starting {
        app_id: String,
        instance_id: String,
    },
    Running {
        app_id: String,
        instance_id: String,
    },
    Stopping {
        app_id: String,
        instance_id: String,
    },
    Stopped {
        app_id: String,
        instance_id: String,
        reason: String,
        exit_code: Option<i32>,
        forced: bool,
    },
    Failed {
        app_id: String,
        instance_id: String,
        error: String,
    },
}

#[derive(Debug, Clone, Default)]
pub struct StartOptions {
    /// When true (default for UI/LLM), stop/reclaim any stuck or live instance first.
    pub replace: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AppRuntimeInspect {
    pub status: AppRuntimeStatus,
    pub supervisor_alive: bool,
    pub process_alive: Option<bool>,
    pub lease: Option<RuntimeLease>,
    pub sandbox_dir: String,
    pub last_reclaim: Option<ReclaimReport>,
}

pub struct AppRuntimeManager {
    inner: Arc<Mutex<ManagerInner>>,
    status: Arc<RwLock<AppRuntimeStatus>>,
    current_instance: Arc<RwLock<Option<String>>>,
    events: broadcast::Sender<AppRuntimeEvent>,
    gpio_backend: Arc<dyn GpioBackend>,
    sdk_dir: PathBuf,
    sandbox: Arc<Mutex<RuntimeSandbox>>,
    last_reclaim: Arc<Mutex<Option<ReclaimReport>>>,
}

struct ManagerInner {
    starting: bool,
    active: Option<ActiveApp>,
}

struct ActiveApp {
    instance_id: String,
    app_id: String,
    pid: Option<u32>,
    control: mpsc::Sender<RuntimeControl>,
    task: JoinHandle<()>,
    gpio: Arc<GpioBroker>,
}

enum RuntimeControl {
    Send {
        event: Value,
        response: oneshot::Sender<Result<(), String>>,
    },
    Stop {
        reason: String,
        response: oneshot::Sender<Result<(), String>>,
    },
}

#[derive(Clone)]
struct EventSink {
    app_id: String,
    instance_id: String,
    current_instance: Arc<RwLock<Option<String>>>,
    events: broadcast::Sender<AppRuntimeEvent>,
}

struct SpawnedPython {
    child: Child,
    stdin: ChildStdin,
    stdout: ChildStdout,
    stderr: ChildStderr,
    interpreter: String,
}

impl Default for AppRuntimeManager {
    fn default() -> Self {
        let workspace = env::temp_dir().join(format!(
            "mooncoding-app-runtime-{}",
            uuid::Uuid::new_v4()
        ));
        let _ = std::fs::create_dir_all(&workspace);
        Self::for_workspace(workspace).expect("temp runtime sandbox")
    }
}

impl AppRuntimeManager {
    pub fn for_workspace(workspace: impl Into<PathBuf>) -> Result<Self> {
        Self::with_sdk_dir(
            Arc::new(MockGpioBackend::default()),
            PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("sdk/python"),
            workspace.into(),
        )
    }

    pub fn with_sdk_dir(
        gpio_backend: Arc<dyn GpioBackend>,
        sdk_dir: PathBuf,
        workspace: PathBuf,
    ) -> Result<Self> {
        let (events, _) = broadcast::channel(EVENT_CHANNEL_CAPACITY);
        Ok(Self {
            inner: Arc::new(Mutex::new(ManagerInner {
                starting: false,
                active: None,
            })),
            status: Arc::new(RwLock::new(AppRuntimeStatus::Idle)),
            current_instance: Arc::new(RwLock::new(None)),
            events,
            gpio_backend,
            sdk_dir,
            sandbox: Arc::new(Mutex::new(RuntimeSandbox::open(&workspace)?)),
            last_reclaim: Arc::new(Mutex::new(None)),
        })
    }

    pub fn subscribe(&self) -> broadcast::Receiver<AppRuntimeEvent> {
        self.events.subscribe()
    }

    pub fn status(&self) -> AppRuntimeStatus {
        match self.status.read() {
            Ok(status) => status.clone(),
            Err(_) => AppRuntimeStatus::Failed {
                app_id: String::new(),
                instance_id: String::new(),
                error: "app runtime status lock poisoned".to_string(),
            },
        }
    }

    pub fn inspect(&self) -> AppRuntimeInspect {
        let (supervisor_alive, pid) = self
            .inner
            .lock()
            .ok()
            .and_then(|inner| {
                inner.active.as_ref().map(|active| {
                    (
                        !active.task.is_finished(),
                        active.pid,
                    )
                })
            })
            .unwrap_or((false, None));
        let lease = self
            .sandbox
            .lock()
            .ok()
            .and_then(|sandbox| sandbox.read_lease().ok().flatten());
        let pid = pid.or_else(|| lease.as_ref().and_then(|lease| lease.pid));
        let sandbox_dir = self
            .sandbox
            .lock()
            .map(|sandbox| sandbox.root().display().to_string())
            .unwrap_or_default();
        let last_reclaim = self
            .last_reclaim
            .lock()
            .ok()
            .and_then(|guard| guard.clone());
        AppRuntimeInspect {
            status: self.status(),
            supervisor_alive,
            process_alive: pid.map(process_alive),
            lease,
            sandbox_dir,
            last_reclaim,
        }
    }

    pub async fn start(&self, app: &AppListing) -> Result<String> {
        self.start_with_options(app, StartOptions { replace: true })
            .await
    }

    pub async fn start_with_options(
        &self,
        app: &AppListing,
        options: StartOptions,
    ) -> Result<String> {
        validate_native_python_app(app)?;
        let entry = validated_entry_path(&app.dir, app.manifest.python_entry())?;
        let gpio = Arc::new(GpioBroker::new(
            self.gpio_backend.clone(),
            &app.manifest.capabilities.gpio,
        )?);
        let instance_id = uuid::Uuid::new_v4().to_string();
        let app_id = app.manifest.name.clone();

        if let Some(report) = self.reclaim_sandbox_orphans()? {
            let _ = self.events.send(AppRuntimeEvent::Log {
                app_id: app_id.clone(),
                instance_id: instance_id.clone(),
                level: "warn".to_string(),
                message: format!("runtime sandbox reclaim: {}", report.reason),
            });
        }

        self.prepare_slot_for_start(options.replace).await?;

        let previous = {
            let mut inner = self
                .inner
                .lock()
                .map_err(|_| anyhow!("app runtime lock poisoned"))?;
            if inner.starting {
                bail!("an app is already starting");
            }
            if let Some(active) = inner.active.as_ref() {
                if !active.task.is_finished() {
                    bail!(
                        "an app is already running (app_id={}, instance_id={}, pid={:?}). \
                         Use replace/force_stop to take over.",
                        active.app_id,
                        active.instance_id,
                        active.pid
                    );
                }
            }
            let previous = inner.active.take();
            inner.starting = true;
            previous
        };
        if let Some(previous) = previous {
            let _ = previous.task.await;
        }

        self.replace_current_instance(Some(instance_id.clone()))?;
        self.replace_status(AppRuntimeStatus::Starting {
            app_id: app_id.clone(),
            instance_id: instance_id.clone(),
        })?;

        let spawned = spawn_python(&app.manifest, &app.dir, &entry, &self.sdk_dir);
        let mut spawned = match spawned {
            Ok(spawned) => spawned,
            Err(error) => {
                self.fail_start(&app_id, &instance_id, &error)?;
                let _ = self.events.send(AppRuntimeEvent::Error {
                    app_id: app_id.clone(),
                    instance_id: instance_id.clone(),
                    message: error.to_string(),
                });
                return Err(error);
            }
        };
        let child_pid = spawned.child.id();

        {
            let sandbox = self
                .sandbox
                .lock()
                .map_err(|_| anyhow!("runtime sandbox lock poisoned"))?;
            let now = unix_now();
            sandbox.write_lease(&RuntimeLease {
                instance_id: instance_id.clone(),
                app_id: app_id.clone(),
                pid: child_pid,
                host_pid: current_host_pid(),
                state: "starting".to_string(),
                started_at_unix: now,
                updated_at_unix: now,
            })?;
        }

        let init = HostToAppMessage::AppInit {
            protocol_version: APP_PROTOCOL_VERSION,
            instance_id: instance_id.clone(),
            app_id: app_id.clone(),
            capabilities: app.manifest.capabilities.clone(),
            limits: app.manifest.limits.clone(),
        };
        if let Err(error) = write_host_message(&mut spawned.stdin, &init).await {
            let _ = spawned.child.kill().await;
            let error = anyhow!("failed to initialize app protocol: {error}");
            self.fail_start(&app_id, &instance_id, &error)?;
            let _ = self.events.send(AppRuntimeEvent::Error {
                app_id: app_id.clone(),
                instance_id: instance_id.clone(),
                message: error.to_string(),
            });
            return Err(error);
        }

        let sink = EventSink {
            app_id: app_id.clone(),
            instance_id: instance_id.clone(),
            current_instance: self.current_instance.clone(),
            events: self.events.clone(),
        };
        sink.emit(AppRuntimeEvent::Started {
            app_id: app_id.clone(),
            instance_id: instance_id.clone(),
            interpreter: spawned.interpreter.clone(),
        });
        let stdin = Arc::new(AsyncMutex::new(spawned.stdin));
        let stopped_emitted = Arc::new(AtomicBool::new(false));
        let stdout_task = tokio::spawn(read_stdout(
            spawned.stdout,
            sink.clone(),
            gpio.clone(),
            stdin.clone(),
            stopped_emitted.clone(),
        ));
        let stderr_task = tokio::spawn(read_stderr(spawned.stderr, sink.clone()));
        let (control, control_rx) = mpsc::channel(CONTROL_CHANNEL_CAPACITY);
        let shutdown_timeout =
            Duration::from_millis(app.manifest.limits.shutdown_timeout_ms.max(1))
                .min(MAX_SHUTDOWN_TIMEOUT);
        let task = tokio::spawn(supervise_process(
            spawned.child,
            stdin,
            control_rx,
            stdout_task,
            stderr_task,
            gpio.clone(),
            sink.clone(),
            self.status.clone(),
            app.manifest.safe_shutdown,
            shutdown_timeout,
            stopped_emitted,
            SupervisorCleanup {
                instance_id: instance_id.clone(),
                inner: self.inner.clone(),
                sandbox: self.sandbox.clone(),
            },
        ));

        {
            let mut inner = self
                .inner
                .lock()
                .map_err(|_| anyhow!("app runtime lock poisoned"))?;
            inner.starting = false;
            inner.active = Some(ActiveApp {
                instance_id: instance_id.clone(),
                app_id: app_id.clone(),
                pid: child_pid,
                control,
                task,
                gpio,
            });
        }
        if let Ok(sandbox) = self.sandbox.lock() {
            let _ = sandbox.update_state(&instance_id, "running", child_pid);
        }
        self.replace_status(AppRuntimeStatus::Running {
            app_id: app_id.clone(),
            instance_id: instance_id.clone(),
        })?;
        Ok(instance_id)
    }

    pub async fn send(&self, event: Value) -> Result<()> {
        let control = {
            let inner = self
                .inner
                .lock()
                .map_err(|_| anyhow!("app runtime lock poisoned"))?;
            let active = inner
                .active
                .as_ref()
                .filter(|active| !active.task.is_finished())
                .ok_or_else(|| anyhow!("no app is running"))?;
            active.control.clone()
        };
        let (response_tx, response_rx) = oneshot::channel();
        control
            .send(RuntimeControl::Send {
                event,
                response: response_tx,
            })
            .await
            .map_err(|_| anyhow!("app runtime has stopped"))?;
        response_rx
            .await
            .map_err(|_| anyhow!("app runtime dropped the send response"))?
            .map_err(anyhow::Error::msg)
    }

    pub async fn stop(&self, reason: impl Into<String>) -> Result<()> {
        let reason = reason.into();
        let (instance_id, control, already_finished) = {
            let inner = self
                .inner
                .lock()
                .map_err(|_| anyhow!("app runtime lock poisoned"))?;
            let Some(active) = inner.active.as_ref() else {
                // No in-memory supervisor — still reclaim sandbox orphans.
                drop(inner);
                let _ = self.reclaim_sandbox_orphans()?;
                if !matches!(self.status(), AppRuntimeStatus::Idle) {
                    self.replace_status(AppRuntimeStatus::Idle)?;
                }
                return Ok(());
            };
            (
                active.instance_id.clone(),
                active.control.clone(),
                active.task.is_finished(),
            )
        };

        if !already_finished {
            let (response_tx, response_rx) = oneshot::channel();
            if control
                .send(RuntimeControl::Stop {
                    reason: reason.clone(),
                    response: response_tx,
                })
                .await
                .is_err()
            {
                return self.force_stop(reason).await;
            }
            if response_rx.await.is_err() {
                return self.force_stop(reason).await;
            }
        }

        let finished = {
            let mut inner = self
                .inner
                .lock()
                .map_err(|_| anyhow!("app runtime lock poisoned"))?;
            if inner
                .active
                .as_ref()
                .is_some_and(|active| active.instance_id == instance_id)
            {
                inner.active.take()
            } else {
                None
            }
        };
        if let Some(finished) = finished {
            let _ = finished.task.await;
            let _ = finished.gpio.safe_reset();
        }
        self.clear_current_if(&instance_id);
        if let Ok(sandbox) = self.sandbox.lock() {
            let _ = sandbox.clear_lease();
        }
        Ok(())
    }

    /// Always clears supervisor + sandbox lease. Safe for LLM/UI recovery.
    pub async fn force_stop(&self, reason: impl Into<String>) -> Result<()> {
        let reason = reason.into();
        let active = {
            let mut inner = self
                .inner
                .lock()
                .map_err(|_| anyhow!("app runtime lock poisoned"))?;
            inner.starting = false;
            inner.active.take()
        };

        if let Some(active) = active {
            let _ = active
                .control
                .send(RuntimeControl::Stop {
                    reason: reason.clone(),
                    response: oneshot::channel().0,
                })
                .await;
            active.task.abort();
            let _ = active.task.await;
            let _ = active.gpio.safe_reset();
            self.clear_current_if(&active.instance_id);
            let _ = self.events.send(AppRuntimeEvent::Stopped {
                app_id: active.app_id,
                instance_id: active.instance_id,
                reason: format!("force_stop: {reason}"),
                exit_code: None,
                forced: true,
            });
        }

        let killed = {
            let sandbox = self
                .sandbox
                .lock()
                .map_err(|_| anyhow!("runtime sandbox lock poisoned"))?;
            sandbox.force_kill_lease_process()?
        };
        if let Some(pid) = killed {
            let _ = self.events.send(AppRuntimeEvent::Log {
                app_id: String::new(),
                instance_id: String::new(),
                level: "warn".to_string(),
                message: format!("force_stop killed orphan pid {pid}"),
            });
        }

        self.replace_status(AppRuntimeStatus::Idle)?;
        *self
            .current_instance
            .write()
            .map_err(|_| anyhow!("app runtime instance lock poisoned"))? = None;
        Ok(())
    }

    async fn prepare_slot_for_start(&self, replace: bool) -> Result<()> {
        let busy = {
            let inner = self
                .inner
                .lock()
                .map_err(|_| anyhow!("app runtime lock poisoned"))?;
            if inner.starting {
                bail!("an app is already starting");
            }
            inner
                .active
                .as_ref()
                .is_some_and(|active| !active.task.is_finished())
        };
        if busy {
            if replace {
                self.force_stop("replaced by new start").await?;
            } else {
                let inspect = self.inspect();
                bail!(
                    "an app is already running (supervisor_alive={}, process_alive={:?}, lease={:?})",
                    inspect.supervisor_alive,
                    inspect.process_alive,
                    inspect.lease.as_ref().map(|lease| {
                        format!("{}:{}:{:?}", lease.app_id, lease.instance_id, lease.pid)
                    })
                );
            }
        }

        // Reclaim finished supervisors left in `active`.
        let finished = {
            let mut inner = self
                .inner
                .lock()
                .map_err(|_| anyhow!("app runtime lock poisoned"))?;
            if inner
                .active
                .as_ref()
                .is_some_and(|active| active.task.is_finished())
            {
                inner.active.take()
            } else {
                None
            }
        };
        if let Some(finished) = finished {
            let _ = finished.task.await;
        }
        Ok(())
    }

    fn reclaim_sandbox_orphans(&self) -> Result<Option<ReclaimReport>> {
        let report = {
            let sandbox = self
                .sandbox
                .lock()
                .map_err(|_| anyhow!("runtime sandbox lock poisoned"))?;
            sandbox.reclaim_orphans()?
        };
        if let Some(report) = &report {
            if let Ok(mut last) = self.last_reclaim.lock() {
                *last = Some(report.clone());
            }
        }
        Ok(report)
    }

    fn fail_start(&self, app_id: &str, instance_id: &str, error: &anyhow::Error) -> Result<()> {
        if let Ok(mut inner) = self.inner.lock() {
            inner.starting = false;
        }
        if let Ok(sandbox) = self.sandbox.lock() {
            let _ = sandbox.clear_lease();
        }
        self.replace_status(AppRuntimeStatus::Failed {
            app_id: app_id.to_string(),
            instance_id: instance_id.to_string(),
            error: error.to_string(),
        })?;
        self.clear_current_if(instance_id);
        Ok(())
    }

    fn replace_status(&self, next: AppRuntimeStatus) -> Result<()> {
        *self
            .status
            .write()
            .map_err(|_| anyhow!("app runtime status lock poisoned"))? = next;
        Ok(())
    }

    fn replace_current_instance(&self, instance_id: Option<String>) -> Result<()> {
        *self
            .current_instance
            .write()
            .map_err(|_| anyhow!("app runtime instance lock poisoned"))? = instance_id;
        Ok(())
    }

    fn clear_current_if(&self, instance_id: &str) {
        if let Ok(mut current) = self.current_instance.write() {
            if current.as_deref() == Some(instance_id) {
                *current = None;
            }
        }
    }
}

struct SupervisorCleanup {
    instance_id: String,
    inner: Arc<Mutex<ManagerInner>>,
    sandbox: Arc<Mutex<RuntimeSandbox>>,
}

impl SupervisorCleanup {
    fn run(self) {
        if let Ok(mut inner) = self.inner.lock() {
            if inner
                .active
                .as_ref()
                .is_some_and(|active| active.instance_id == self.instance_id)
            {
                inner.active = None;
            }
            inner.starting = false;
        }
        if let Ok(sandbox) = self.sandbox.lock() {
            if let Ok(Some(lease)) = sandbox.read_lease() {
                if lease.instance_id == self.instance_id {
                    let _ = sandbox.clear_lease();
                }
            }
        }
    }
}

impl Drop for AppRuntimeManager {
    fn drop(&mut self) {
        if let Ok(mut current) = self.current_instance.write() {
            *current = None;
        }
        if let Ok(mut inner) = self.inner.lock() {
            if let Some(active) = inner.active.take() {
                active.task.abort();
                let _ = active.gpio.safe_reset();
            }
        }
        if let Ok(sandbox) = self.sandbox.lock() {
            let _ = sandbox.force_kill_lease_process();
        }
    }
}

impl EventSink {
    fn is_current(&self) -> bool {
        self.current_instance
            .read()
            .map(|current| current.as_deref() == Some(self.instance_id.as_str()))
            .unwrap_or(false)
    }

    fn emit(&self, event: AppRuntimeEvent) {
        if self.is_current() {
            let _ = self.events.send(event);
        }
    }
}

fn validate_native_python_app(app: &AppListing) -> Result<()> {
    if app.manifest.app_type != "python" {
        bail!(
            "native runtime only starts Python apps, got {}",
            app.manifest.app_type
        );
    }
    if let Some(runtime) = &app.manifest.runtime {
        if runtime.kind != AppRuntimeKind::Python {
            bail!("unsupported app runtime kind");
        }
    }
    if app.manifest.limits.max_message_bytes == 0
        || app.manifest.limits.max_message_bytes > MAX_JSONL_LINE_BYTES
    {
        bail!(
            "max_message_bytes must be between 1 and {}",
            MAX_JSONL_LINE_BYTES
        );
    }
    Ok(())
}

fn validated_entry_path(app_dir: &Path, entry: &str) -> Result<PathBuf> {
    let relative = Path::new(entry);
    if entry.trim().is_empty()
        || relative.is_absolute()
        || relative
            .components()
            .any(|component| !matches!(component, Component::Normal(_) | Component::CurDir))
    {
        bail!("Python entry must be a relative path inside the app directory");
    }
    let canonical_dir = app_dir
        .canonicalize()
        .with_context(|| format!("cannot resolve app directory {}", app_dir.display()))?;
    let canonical_entry = app_dir
        .join(relative)
        .canonicalize()
        .with_context(|| format!("cannot resolve Python entry {entry}"))?;
    if !canonical_entry.starts_with(&canonical_dir) || !canonical_entry.is_file() {
        bail!("Python entry must be a file inside the app directory");
    }
    Ok(canonical_entry)
}

fn spawn_python(
    manifest: &crate::apps::AppManifest,
    app_dir: &Path,
    entry: &Path,
    sdk_dir: &Path,
) -> Result<SpawnedPython> {
    let interpreter = manifest
        .runtime
        .as_ref()
        .map(|runtime| runtime.interpreter)
        .unwrap_or_default();
    let candidates = python_command_candidates(interpreter);
    let mut not_found = Vec::new();
    for candidate in &candidates {
        let mut command = Command::new(candidate);
        command
            .arg("-u")
            .arg("-B")
            .arg("-s")
            .arg(entry)
            .current_dir(app_dir)
            .stdin(Stdio::piped())
            .stdout(Stdio::piped())
            .stderr(Stdio::piped())
            .kill_on_drop(true);
        configure_safe_environment(&mut command, sdk_dir);

        match command.spawn() {
            Ok(mut child) => {
                let stdin = child
                    .stdin
                    .take()
                    .ok_or_else(|| anyhow!("Python stdin pipe was not created"))?;
                let stdout = child
                    .stdout
                    .take()
                    .ok_or_else(|| anyhow!("Python stdout pipe was not created"))?;
                let stderr = child
                    .stderr
                    .take()
                    .ok_or_else(|| anyhow!("Python stderr pipe was not created"))?;
                return Ok(SpawnedPython {
                    child,
                    stdin,
                    stdout,
                    stderr,
                    interpreter: candidate.display().to_string(),
                });
            }
            Err(error) if error.kind() == std::io::ErrorKind::NotFound => {
                not_found.push(candidate.display().to_string());
            }
            Err(error) => {
                return Err(anyhow!(
                    "failed to start {}: {error}",
                    candidate.display()
                ));
            }
        }
    }
    bail!(
        "Python interpreter not found (tried {})",
        not_found.join(", ")
    )
}

fn python_command_candidates(interpreter: PythonInterpreter) -> Vec<PathBuf> {
    let names: &[&str] = match interpreter {
        PythonInterpreter::Auto if cfg!(windows) => &["python", "python3"],
        PythonInterpreter::Auto => &["python3", "python"],
        PythonInterpreter::Python => &["python"],
        PythonInterpreter::Python3 => &["python3"],
    };

    let mut ordered = Vec::new();
    let mut push_unique = |path: PathBuf| {
        if path.as_os_str().is_empty() || !path.is_file() {
            return;
        }
        if ordered.iter().any(|existing| existing == &path) {
            return;
        }
        ordered.push(path);
    };

    // Prefer real installs over the Windows Store alias stub.
    if cfg!(windows) {
        for name in names {
            if let Ok(path) = which_in_path(name) {
                let lowered = path.to_string_lossy().to_ascii_lowercase();
                if !lowered.contains("\\windowsapps\\") {
                    push_unique(path);
                }
            }
        }
        if let Some(local) = env::var_os("LOCALAPPDATA") {
            let local = PathBuf::from(local);
            for version in ["Python313", "Python312", "Python311", "Python310"] {
                for name in names {
                    push_unique(
                        local
                            .join("Programs")
                            .join("Python")
                            .join(version)
                            .join(format!("{name}.exe")),
                    );
                }
            }
        }
        if let Some(home) = env::var_os("USERPROFILE") {
            let home = PathBuf::from(home);
            for name in names {
                push_unique(home.join("anaconda3").join(format!("{name}.exe")));
                push_unique(home.join("miniconda3").join(format!("{name}.exe")));
            }
        }
    }

    for name in names {
        let bare = PathBuf::from(name);
        if ordered.iter().all(|existing| existing != &bare) {
            ordered.push(bare);
        }
    }
    ordered
}

fn which_in_path(name: &str) -> Result<PathBuf, ()> {
    let path = env::var_os("PATH").ok_or(())?;
    for dir in env::split_paths(&path) {
        let candidate = dir.join(name);
        if candidate.is_file() {
            return Ok(candidate);
        }
        let with_exe = dir.join(format!("{name}.exe"));
        if with_exe.is_file() {
            return Ok(with_exe);
        }
    }
    Err(())
}

fn configure_safe_environment(command: &mut Command, sdk_dir: &Path) {
    const PASSTHROUGH: &[&str] = &[
        "PATH",
        "SYSTEMROOT",
        "WINDIR",
        "TEMP",
        "TMP",
        "HOME",
        "LANG",
        "LC_ALL",
    ];
    command.env_clear();
    for key in PASSTHROUGH {
        if let Some(value) = env::var_os(key) {
            command.env(key, value);
        }
    }
    command
        .env("PYTHONUNBUFFERED", "1")
        .env("PYTHONDONTWRITEBYTECODE", "1")
        .env("PYTHONNOUSERSITE", "1")
        .env("PYTHONPATH", sdk_dir)
        .env("MOONCODING_APP_PROTOCOL", APP_PROTOCOL_VERSION.to_string());
}

async fn read_stdout(
    stdout: ChildStdout,
    sink: EventSink,
    gpio: Arc<GpioBroker>,
    stdin: Arc<AsyncMutex<ChildStdin>>,
    stopped_emitted: Arc<AtomicBool>,
) {
    let mut reader = BufReader::new(stdout);
    loop {
        let message = match read_app_message(&mut reader).await {
            Ok(Some(message)) => message,
            Ok(None) => return,
            Err(error) => {
                sink.emit(AppRuntimeEvent::Error {
                    app_id: sink.app_id.clone(),
                    instance_id: sink.instance_id.clone(),
                    message: error.to_string(),
                });
                if matches!(error, ProtocolError::Io(_)) {
                    return;
                }
                continue;
            }
        };
        if !sink.is_current() {
            continue;
        }

        match message {
            AppToHostMessage::AppReady { protocol_version } => {
                if protocol_version != APP_PROTOCOL_VERSION {
                    sink.emit(AppRuntimeEvent::Error {
                        app_id: sink.app_id.clone(),
                        instance_id: sink.instance_id.clone(),
                        message: format!("unsupported app protocol version {protocol_version}"),
                    });
                }
            }
            AppToHostMessage::UiInit { ui } => {
                sink.emit(AppRuntimeEvent::UiInit {
                    app_id: sink.app_id.clone(),
                    instance_id: sink.instance_id.clone(),
                    ui,
                });
            }
            AppToHostMessage::UiPatch { patch } => {
                sink.emit(AppRuntimeEvent::UiPatch {
                    app_id: sink.app_id.clone(),
                    instance_id: sink.instance_id.clone(),
                    patch,
                });
            }
            AppToHostMessage::Log { level, message } => {
                sink.emit(AppRuntimeEvent::Log {
                    app_id: sink.app_id.clone(),
                    instance_id: sink.instance_id.clone(),
                    level,
                    message,
                });
            }
            AppToHostMessage::AppError { message } => {
                sink.emit(AppRuntimeEvent::Error {
                    app_id: sink.app_id.clone(),
                    instance_id: sink.instance_id.clone(),
                    message,
                });
            }
            AppToHostMessage::AppStopped { reason } => {
                if !stopped_emitted.swap(true, Ordering::SeqCst) {
                    sink.emit(AppRuntimeEvent::Stopped {
                        app_id: sink.app_id.clone(),
                        instance_id: sink.instance_id.clone(),
                        reason: reason.unwrap_or_else(|| "app acknowledged stop".to_string()),
                        exit_code: None,
                        forced: false,
                    });
                }
            }
            AppToHostMessage::GpioRequest {
                request_id,
                alias,
                operation,
                mode,
                value,
            } => {
                sink.emit(AppRuntimeEvent::GpioRequest {
                    app_id: sink.app_id.clone(),
                    instance_id: sink.instance_id.clone(),
                    request_id: request_id.clone(),
                    alias: alias.clone(),
                    operation,
                });
                let request = GpioRequest {
                    alias,
                    operation,
                    mode,
                    value,
                };
                let response = match gpio.execute(&request) {
                    Ok(value) => HostToAppMessage::GpioResult {
                        request_id,
                        ok: true,
                        value,
                        error: None,
                    },
                    Err(error) => HostToAppMessage::GpioResult {
                        request_id,
                        ok: false,
                        value: None,
                        error: Some(error.to_string()),
                    },
                };
                let write_result = {
                    let mut writer = stdin.lock().await;
                    write_host_message(&mut *writer, &response).await
                };
                if let Err(error) = write_result {
                    sink.emit(AppRuntimeEvent::Error {
                        app_id: sink.app_id.clone(),
                        instance_id: sink.instance_id.clone(),
                        message: format!("failed to send GPIO response: {error}"),
                    });
                    return;
                }
            }
        }
    }
}

async fn read_stderr(stderr: ChildStderr, sink: EventSink) {
    let mut reader = BufReader::new(stderr);
    loop {
        match read_bounded_line(&mut reader, MAX_JSONL_LINE_BYTES).await {
            Ok(Some(line)) => {
                if !line.is_empty() {
                    sink.emit(AppRuntimeEvent::Log {
                        app_id: sink.app_id.clone(),
                        instance_id: sink.instance_id.clone(),
                        level: "stderr".to_string(),
                        message: crate::encoding_util::decode_console_bytes(&line),
                    });
                }
            }
            Ok(None) => return,
            Err(error) => {
                sink.emit(AppRuntimeEvent::Error {
                    app_id: sink.app_id.clone(),
                    instance_id: sink.instance_id.clone(),
                    message: format!("app stderr error: {error}"),
                });
                if matches!(error, ProtocolError::Io(_)) {
                    return;
                }
            }
        }
    }
}

#[allow(clippy::too_many_arguments)]
async fn supervise_process(
    mut child: Child,
    stdin: Arc<AsyncMutex<ChildStdin>>,
    mut controls: mpsc::Receiver<RuntimeControl>,
    mut stdout_task: JoinHandle<()>,
    mut stderr_task: JoinHandle<()>,
    gpio: Arc<GpioBroker>,
    sink: EventSink,
    status: Arc<RwLock<AppRuntimeStatus>>,
    safe_shutdown: bool,
    shutdown_timeout: Duration,
    stopped_emitted: Arc<AtomicBool>,
    cleanup: SupervisorCleanup,
) {
    let mut requested_reason = None;
    let mut forced = false;
    let mut stop_response = None;

    let wait_result: Result<ExitStatus, String> = loop {
        tokio::select! {
            result = child.wait() => {
                break result.map_err(|error| error.to_string());
            }
            control = controls.recv() => {
                match control {
                    Some(RuntimeControl::Send { event, response }) => {
                        let result = {
                            let mut writer = stdin.lock().await;
                            write_host_message(
                                &mut *writer,
                                &HostToAppMessage::UiEvent { event },
                            )
                            .await
                            .map_err(|error| error.to_string())
                        };
                        let _ = response.send(result);
                    }
                    Some(RuntimeControl::Stop { reason, response }) => {
                        set_status_if_current(
                            &status,
                            &sink,
                            AppRuntimeStatus::Stopping {
                                app_id: sink.app_id.clone(),
                                instance_id: sink.instance_id.clone(),
                            },
                        );
                        requested_reason = Some(reason.clone());
                        stop_response = Some(response);
                        let result = terminate_child(
                            &mut child,
                            &stdin,
                            safe_shutdown,
                            shutdown_timeout,
                            reason,
                        )
                        .await;
                        forced = result.1;
                        break result.0;
                    }
                    None => {
                        requested_reason =
                            Some("runtime control channel closed".to_string());
                        let result = terminate_child(
                            &mut child,
                            &stdin,
                            false,
                            Duration::ZERO,
                            "runtime control channel closed".to_string(),
                        )
                        .await;
                        forced = true;
                        break result.0;
                    }
                }
            }
        }
    };

    finish_reader(&mut stdout_task).await;
    finish_reader(&mut stderr_task).await;
    if let Err(error) = gpio.safe_reset() {
        sink.emit(AppRuntimeEvent::Error {
            app_id: sink.app_id.clone(),
            instance_id: sink.instance_id.clone(),
            message: error.to_string(),
        });
    }

    let (exit_code, final_error) = match &wait_result {
        Ok(exit_status) => (exit_status.code(), None),
        Err(error) => (None, Some(error.clone())),
    };
    let reason = requested_reason.unwrap_or_else(|| match &wait_result {
        Ok(exit_status) => format!("process exited with {exit_status}"),
        Err(error) => format!("process wait failed: {error}"),
    });

    if let Some(error) = final_error {
        set_status_if_current(
            &status,
            &sink,
            AppRuntimeStatus::Failed {
                app_id: sink.app_id.clone(),
                instance_id: sink.instance_id.clone(),
                error,
            },
        );
    } else {
        set_status_if_current(
            &status,
            &sink,
            AppRuntimeStatus::Stopped {
                app_id: sink.app_id.clone(),
                instance_id: sink.instance_id.clone(),
                reason: reason.clone(),
                exit_code,
                forced,
            },
        );
    }
    if !stopped_emitted.swap(true, Ordering::SeqCst) {
        sink.emit(AppRuntimeEvent::Stopped {
            app_id: sink.app_id.clone(),
            instance_id: sink.instance_id.clone(),
            reason,
            exit_code,
            forced,
        });
    }
    if let Some(response) = stop_response {
        let response_value = wait_result.map(|_| ());
        let _ = response.send(response_value);
    }
    cleanup.run();
}

async fn terminate_child(
    child: &mut Child,
    stdin: &Arc<AsyncMutex<ChildStdin>>,
    safe_shutdown: bool,
    shutdown_timeout: Duration,
    reason: String,
) -> (Result<ExitStatus, String>, bool) {
    if safe_shutdown {
        let write_result = {
            let mut writer = stdin.lock().await;
            write_host_message(&mut *writer, &HostToAppMessage::AppStop { reason }).await
        };
        if write_result.is_ok() {
            match timeout(shutdown_timeout, child.wait()).await {
                Ok(result) => return (result.map_err(|error| error.to_string()), false),
                Err(_) => {}
            }
        }
    }

    let kill_result = child.start_kill().map_err(|error| error.to_string());
    if let Err(error) = kill_result {
        return (Err(error), true);
    }
    (child.wait().await.map_err(|error| error.to_string()), true)
}

async fn finish_reader(task: &mut JoinHandle<()>) {
    if timeout(READER_JOIN_TIMEOUT, &mut *task).await.is_err() {
        task.abort();
        let _ = task.await;
    }
}

fn set_status_if_current(
    status: &Arc<RwLock<AppRuntimeStatus>>,
    sink: &EventSink,
    next: AppRuntimeStatus,
) {
    if sink.is_current() {
        if let Ok(mut current) = status.write() {
            *current = next;
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::apps::AppManifest;
    use serde_json::json;
    use std::fs;

    async fn available_python() -> Option<PythonInterpreter> {
        for (name, interpreter) in [
            ("python", PythonInterpreter::Python),
            ("python3", PythonInterpreter::Python3),
        ] {
            let status = Command::new(name)
                .arg("--version")
                .stdin(Stdio::null())
                .stdout(Stdio::null())
                .stderr(Stdio::null())
                .status()
                .await;
            if status.is_ok_and(|status| status.success()) {
                return Some(interpreter);
            }
        }
        None
    }

    async fn receive_until(
        receiver: &mut broadcast::Receiver<AppRuntimeEvent>,
        predicate: impl Fn(&AppRuntimeEvent) -> bool,
    ) -> Result<AppRuntimeEvent> {
        timeout(Duration::from_secs(5), async {
            loop {
                let event = receiver
                    .recv()
                    .await
                    .map_err(|error| anyhow!("event receive failed: {error}"))?;
                if predicate(&event) {
                    return Ok(event);
                }
            }
        })
        .await
        .map_err(|_| anyhow!("timed out waiting for app event"))?
    }

    #[tokio::test]
    async fn python_runtime_start_send_stop_lifecycle() -> Result<()> {
        let Some(interpreter) = available_python().await else {
            eprintln!("skipping runtime lifecycle test: python/python3 unavailable");
            return Ok(());
        };
        let root =
            env::temp_dir().join(format!("mooncoding-runtime-test-{}", uuid::Uuid::new_v4()));
        let app_dir = root.join("apps").join("lifecycle");
        fs::create_dir_all(&app_dir)?;
        fs::write(
            app_dir.join("main.py"),
            r#"from mooncoding_app import App

app = App()
app.ui_init({"type": "screen", "id": "root"})

def on_event(event):
    app.log(event["message"])

app.run(on_event)
"#,
        )?;
        let mut manifest: AppManifest = serde_json::from_value(json!({
            "schema_version": 2,
            "type": "python",
            "title": "Lifecycle",
            "description": "runtime test",
            "runtime": {
                "kind": "python",
                "entry": "main.py",
                "interpreter": match interpreter {
                    PythonInterpreter::Python => "python",
                    _ => "python3",
                }
            },
            "ui": {"kind": "native_json", "entry": "ui.json"}
        }))?;
        manifest.name = "lifecycle".to_string();
        let listing = AppListing {
            manifest,
            dir: app_dir.clone(),
            entry_path: Some(app_dir.join("main.py")),
            entry_exists: true,
        };
        let manager = AppRuntimeManager::for_workspace(&root)?;
        let mut events = manager.subscribe();

        let expected_instance_id = manager.start(&listing).await?;
        let started = receive_until(&mut events, |event| {
            matches!(event, AppRuntimeEvent::Started { .. })
        })
        .await?;
        assert!(matches!(
            started,
            AppRuntimeEvent::Started {
                ref instance_id,
                ..
            } if instance_id == &expected_instance_id
        ));
        receive_until(&mut events, |event| {
            matches!(event, AppRuntimeEvent::UiInit { .. })
        })
        .await?;

        manager.send(json!({"message": "button pressed"})).await?;
        let log = receive_until(&mut events, |event| {
            matches!(
                event,
                AppRuntimeEvent::Log { message, .. } if message == "button pressed"
            )
        })
        .await?;
        assert!(matches!(log, AppRuntimeEvent::Log { .. }));

        manager.stop("test complete").await?;
        assert!(matches!(manager.status(), AppRuntimeStatus::Stopped { .. }));
        fs::remove_dir_all(root)?;
        Ok(())
    }

    #[tokio::test]
    async fn replace_start_clears_busy_slot() -> Result<()> {
        let Some(interpreter) = available_python().await else {
            eprintln!("skipping replace_start test: python unavailable");
            return Ok(());
        };
        let root =
            env::temp_dir().join(format!("mooncoding-runtime-replace-{}", uuid::Uuid::new_v4()));
        let app_dir = root.join("apps").join("busy");
        fs::create_dir_all(&app_dir)?;
        fs::write(
            app_dir.join("main.py"),
            r#"from mooncoding_app import App
app = App()
app.ui_init({"type": "screen", "id": "root"})
def on_event(event):
    pass
app.run(on_event)
"#,
        )?;
        let mut manifest: AppManifest = serde_json::from_value(json!({
            "schema_version": 2,
            "type": "python",
            "title": "Busy",
            "description": "replace test",
            "runtime": {
                "kind": "python",
                "entry": "main.py",
                "interpreter": match interpreter {
                    PythonInterpreter::Python => "python",
                    _ => "python3",
                }
            },
            "ui": {"kind": "native_json", "entry": "ui.json"}
        }))?;
        manifest.name = "busy".to_string();
        let listing = AppListing {
            manifest,
            dir: app_dir.clone(),
            entry_path: Some(app_dir.join("main.py")),
            entry_exists: true,
        };
        let manager = AppRuntimeManager::for_workspace(&root)?;
        let first = manager.start(&listing).await?;
        let second = manager
            .start_with_options(&listing, StartOptions { replace: true })
            .await?;
        assert_ne!(first, second);
        assert!(manager.inspect().supervisor_alive);
        manager.force_stop("test cleanup").await?;
        assert!(!manager.inspect().supervisor_alive);
        fs::remove_dir_all(&root)?;
        Ok(())
    }
}
