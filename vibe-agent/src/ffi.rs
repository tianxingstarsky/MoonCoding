use anyhow::{anyhow, bail, Result};
use serde::Deserialize;
use serde_json::{json, Value};
use std::ffi::{c_char, c_void, CStr, CString};
use std::panic::{catch_unwind, AssertUnwindSafe};
use std::path::PathBuf;
use std::ptr;
use std::sync::{Arc, Mutex, OnceLock};
use std::thread::JoinHandle;

use crate::config::{ApiSource, Config, ManagedApiConfig};
use crate::desktop::DesktopCore;
use crate::stream::AgentEvent;
use crate::tree::{NewTreeNode, NodePatch, TreeField};

pub type EventCallback = extern "C" fn(event_json: *const c_char, user_data: *mut c_void);

static LAST_ERROR: OnceLock<Mutex<String>> = OnceLock::new();

#[derive(Debug, Default, Deserialize)]
struct InitOptions {
    workspace: PathBuf,
    session_id: Option<String>,
    language: Option<String>,
    api_source: Option<String>,
    base_url: Option<String>,
    model: Option<String>,
    api_key: Option<String>,
    max_tokens: Option<u64>,
    temperature: Option<f64>,
    max_steps: Option<u64>,
    prune_after: Option<usize>,
    prune_keep: Option<usize>,
    vibe_exe: Option<PathBuf>,
    managed_endpoint: Option<String>,
    managed_auth_token: Option<String>,
    managed_project_id: Option<String>,
}

#[derive(Debug, Deserialize)]
struct AddNodeRequest {
    expected_version: u64,
    node: NewTreeNode,
}

#[derive(Debug, Deserialize)]
struct UpdateNodeRequest {
    expected_version: u64,
    node_id: String,
    patch: NodePatch,
}

#[derive(Debug, Deserialize)]
struct DeleteNodeRequest {
    expected_version: u64,
    node_id: String,
}

#[derive(Debug, Deserialize)]
struct ReleaseFieldsRequest {
    expected_version: u64,
    node_id: String,
    fields: Vec<TreeField>,
}

pub struct VibeHandle {
    core: Arc<DesktopCore>,
    callback: Option<EventCallback>,
    user_data: usize,
    worker: Mutex<Option<JoinHandle<()>>>,
    /// Long-lived runtime that owns micro-app supervisor tasks.
    runtime: tokio::runtime::Runtime,
}

#[no_mangle]
pub extern "C" fn vibe_api_version() -> u32 {
    3
}

#[no_mangle]
pub extern "C" fn vibe_init(
    options_json: *const c_char,
    callback: Option<EventCallback>,
    user_data: *mut c_void,
) -> *mut VibeHandle {
    match catch_unwind(AssertUnwindSafe(|| {
        create_handle(options_json, callback, user_data)
    })) {
        Ok(Ok(handle)) => Box::into_raw(Box::new(handle)),
        Ok(Err(error)) => {
            set_last_error(error.to_string());
            ptr::null_mut()
        }
        Err(payload) => {
            set_last_error(panic_message(payload));
            ptr::null_mut()
        }
    }
}

#[no_mangle]
pub extern "C" fn vibe_send(handle: *mut VibeHandle, input: *const c_char) -> i32 {
    ffi_status(|| {
        let prompt = string_arg_owned(input)?;
        with_handle(handle, |handle| start_send(handle, prompt))
    })
}

#[no_mangle]
pub extern "C" fn vibe_interrupt(handle: *mut VibeHandle) -> i32 {
    ffi_status(|| {
        with_handle(handle, |handle| {
            handle.core.interrupt();
            Ok(())
        })
    })
}

#[no_mangle]
pub extern "C" fn vibe_tree_get_json(handle: *mut VibeHandle) -> *mut c_char {
    ffi_json_result(|| {
        with_handle(handle, |handle| {
            let tree = block_on(handle.core.tree_json())?;
            Ok(serde_json::from_str(&tree)?)
        })
    })
}

#[no_mangle]
pub extern "C" fn vibe_sessions_get_json(handle: *mut VibeHandle) -> *mut c_char {
    ffi_json_result(|| {
        with_handle(handle, |handle| {
            Ok(serde_json::from_str(&block_on(
                handle.core.sessions_json(),
            )?)?)
        })
    })
}

#[no_mangle]
pub extern "C" fn vibe_session_get_json(
    handle: *mut VibeHandle,
    session_id: *const c_char,
) -> *mut c_char {
    ffi_json_result(|| {
        let session_id = string_arg_owned(session_id)?;
        with_handle(handle, |handle| {
            Ok(serde_json::from_str(&block_on(
                handle.core.session_json(&session_id),
            )?)?)
        })
    })
}

#[no_mangle]
pub extern "C" fn vibe_tree_add_node(
    handle: *mut VibeHandle,
    request_json: *const c_char,
) -> *mut c_char {
    ffi_json_result(|| {
        let request: AddNodeRequest = parse_json_arg(request_json)?;
        with_idle_handle(handle, |handle| {
            let core = handle.core.clone();
            block_on(async move {
                let id = core
                    .add_human_node(request.node, request.expected_version)
                    .await?;
                let tree: Value = serde_json::from_str(&core.tree_json().await?)?;
                Ok(json!({"created_id": id, "tree": tree}))
            })
        })
    })
}

#[no_mangle]
pub extern "C" fn vibe_tree_update_node(
    handle: *mut VibeHandle,
    request_json: *const c_char,
) -> *mut c_char {
    ffi_json_result(|| {
        let request: UpdateNodeRequest = parse_json_arg(request_json)?;
        with_idle_handle(handle, |handle| {
            let core = handle.core.clone();
            block_on(async move {
                core.update_human_node(&request.node_id, request.patch, request.expected_version)
                    .await?;
                Ok(serde_json::from_str(&core.tree_json().await?)?)
            })
        })
    })
}

#[no_mangle]
pub extern "C" fn vibe_tree_delete_node(
    handle: *mut VibeHandle,
    request_json: *const c_char,
) -> *mut c_char {
    ffi_json_result(|| {
        let request: DeleteNodeRequest = parse_json_arg(request_json)?;
        with_idle_handle(handle, |handle| {
            let core = handle.core.clone();
            block_on(async move {
                let deleted_ids = core
                    .delete_human_node(&request.node_id, request.expected_version)
                    .await?;
                let tree: Value = serde_json::from_str(&core.tree_json().await?)?;
                Ok(json!({"deleted_ids": deleted_ids, "tree": tree}))
            })
        })
    })
}

#[no_mangle]
pub extern "C" fn vibe_tree_release_fields(
    handle: *mut VibeHandle,
    request_json: *const c_char,
) -> *mut c_char {
    ffi_json_result(|| {
        let request: ReleaseFieldsRequest = parse_json_arg(request_json)?;
        with_idle_handle(handle, |handle| {
            let core = handle.core.clone();
            block_on(async move {
                core.release_human_fields(
                    &request.node_id,
                    &request.fields,
                    request.expected_version,
                )
                .await?;
                Ok(serde_json::from_str(&core.tree_json().await?)?)
            })
        })
    })
}

#[no_mangle]
pub extern "C" fn vibe_tree_review_node(handle: *mut VibeHandle, node_id: *const c_char) -> i32 {
    ffi_status(|| review_node(handle, node_id))
}

#[no_mangle]
pub extern "C" fn vibe_tree_review_all(handle: *mut VibeHandle) -> i32 {
    ffi_status(|| review_all(handle))
}

#[no_mangle]
pub extern "C" fn vibe_apps_list_json(handle: *mut VibeHandle) -> *mut c_char {
    ffi_json_result(|| {
        with_handle(handle, |handle| {
            let apps = handle.core.list_apps();
            Ok(serde_json::to_value(&apps)?)
        })
    })
}

#[no_mangle]
pub extern "C" fn vibe_apps_get_json(handle: *mut VibeHandle, name: *const c_char) -> *mut c_char {
    ffi_json_result(|| {
        let name = string_arg_owned(name)?;
        with_handle(handle, |handle| {
            let app = handle.core.get_app(&name);
            Ok(serde_json::to_value(&app)?)
        })
    })
}

#[no_mangle]
pub extern "C" fn vibe_apps_read_entry(
    handle: *mut VibeHandle,
    name: *const c_char,
) -> *mut c_char {
    ffi_json_result(|| {
        let name = string_arg_owned(name)?;
        with_handle(handle, |handle| {
            let content = handle.core.read_app_entry(&name)?;
            Ok(json!({"content": content}))
        })
    })
}

#[no_mangle]
pub extern "C" fn vibe_apps_start(handle: *mut VibeHandle, name: *const c_char) -> i32 {
    ffi_status(|| {
        let name = string_arg_owned(name)?;
        with_handle(handle, |handle| {
            let core = handle.core.clone();
            let callback = handle.callback;
            let user_data = handle.user_data;
            handle.runtime.spawn(async move {
                if let Err(error) = core.start_app(&name).await {
                    emit_event(
                        callback,
                        user_data,
                        &AgentEvent::Error(format!("app start failed: {error}")),
                    );
                }
            });
            Ok(())
        })
    })
}

#[no_mangle]
pub extern "C" fn vibe_apps_send(handle: *mut VibeHandle, event_json: *const c_char) -> i32 {
    ffi_status(|| {
        let event: Value = parse_json_arg(event_json)?;
        with_handle(handle, |handle| {
            let core = handle.core.clone();
            handle.runtime.block_on(async move { core.send_app_message(event).await })
        })
    })
}

#[no_mangle]
pub extern "C" fn vibe_apps_stop(handle: *mut VibeHandle) -> i32 {
    ffi_status(|| {
        with_handle(handle, |handle| {
            let core = handle.core.clone();
            // force_stop always clears ghost supervisors / orphan PIDs.
            handle
                .runtime
                .block_on(async move { core.force_stop_app().await })
        })
    })
}

#[no_mangle]
pub extern "C" fn vibe_apps_status_json(handle: *mut VibeHandle) -> *mut c_char {
    ffi_json_result(|| {
        with_handle(handle, |handle| Ok(serde_json::to_value(handle.core.app_inspect())?))
    })
}

#[no_mangle]
pub extern "C" fn vibe_last_error() -> *mut c_char {
    catch_unwind(AssertUnwindSafe(|| {
        let message = LAST_ERROR
            .get_or_init(|| Mutex::new(String::new()))
            .lock()
            .map(|value| value.clone())
            .unwrap_or_else(|_| "last-error lock poisoned".to_string());
        owned_c_string(message)
    }))
    .unwrap_or(ptr::null_mut())
}

/// Release strings returned by this library.
///
/// Passing any pointer not returned by a `vibe_*` string function is invalid.
#[no_mangle]
pub unsafe extern "C" fn vibe_string_free(value: *mut c_char) {
    let _ = catch_unwind(AssertUnwindSafe(|| {
        if !value.is_null() {
            // SAFETY: The caller follows the ownership contract documented above.
            drop(unsafe { CString::from_raw(value) });
        }
    }));
}

/// Stop active work and release an opaque handle returned by `vibe_init`.
#[no_mangle]
pub unsafe extern "C" fn vibe_destroy(handle: *mut VibeHandle) {
    let result = catch_unwind(AssertUnwindSafe(|| {
        if handle.is_null() {
            return;
        }
        // SAFETY: The caller transfers the unique pointer returned by `vibe_init`.
        let mut boxed = unsafe { Box::from_raw(handle) };
        boxed.core.interrupt();
        let core = boxed.core.clone();
        let _ = boxed.runtime.block_on(async move {
            let _ = core.stop_app().await;
        });
        if let Ok(worker_slot) = boxed.worker.get_mut() {
            if let Some(worker) = worker_slot.take() {
                if worker.thread().id() != std::thread::current().id() {
                    let _ = worker.join();
                }
            }
        }
    }));
    if let Err(payload) = result {
        set_last_error(panic_message(payload));
    }
}

fn create_handle(
    options_json: *const c_char,
    callback: Option<EventCallback>,
    user_data: *mut c_void,
) -> Result<VibeHandle> {
    let options: InitOptions = parse_json_arg(options_json)?;
    if options.workspace.as_os_str().is_empty() {
        bail!("workspace is required");
    }
    if !options.workspace.is_dir() {
        bail!(
            "workspace directory does not exist: {}",
            options.workspace.display()
        );
    }

    let mut config = Config::load(&options.workspace)?;
    if let Some(ref lang) = options.language {
        if !lang.is_empty() {
            config.language = lang.clone();
        }
    }
    if let Some(value) = options.base_url {
        config.provider.base_url = value;
    }
    if let Some(value) = options.model {
        config.provider.model = value;
    }
    if let Some(value) = options.api_key {
        config.provider.api_key = value;
    }
    if let Some(ref source) = options.api_source {
        if source == "managed" {
            config.api_source = ApiSource::Managed;
            let managed = ManagedApiConfig {
                endpoint: options.managed_endpoint.unwrap_or_default(),
                auth_token: options.managed_auth_token.unwrap_or_default(),
                project_id: options.managed_project_id,
            };
            config.managed_api = Some(managed);
        }
    }
    if let Some(value) = options.max_tokens {
        config.provider.max_tokens = value;
    }
    if let Some(value) = options.temperature {
        config.provider.temperature = value;
    }
    if let Some(value) = options.max_steps {
        config.agent.max_steps = Some(value);
    }
    if let Some(value) = options.prune_after {
        config.agent.prune_after = Some(value);
    }
    if let Some(value) = options.prune_keep {
        config.agent.prune_keep = Some(value);
    }
    if let Some(value) = options.vibe_exe {
        config.vibe_exe = value;
    }
    if config.vibe_exe.is_absolute() && !config.vibe_exe.is_file() {
        bail!(
            "vibe protocol executable does not exist: {}",
            config.vibe_exe.display()
        );
    }
    let session_id = options
        .session_id
        .filter(|value| !value.trim().is_empty())
        .unwrap_or_else(|| uuid::Uuid::new_v4().to_string());
    let core = Arc::new(DesktopCore::open(config, session_id)?);
    let runtime = tokio::runtime::Builder::new_multi_thread()
        .worker_threads(2)
        .enable_all()
        .thread_name("mooncoding-rt")
        .build()?;

    let callback_for_apps = callback;
    let user_data_for_apps = user_data as usize;
    let mut app_events = core.subscribe_app_events();
    runtime.spawn(async move {
        loop {
            match app_events.recv().await {
                Ok(event) => {
                    emit_event(
                        callback_for_apps,
                        user_data_for_apps,
                        &AgentEvent::AppRuntime(event),
                    );
                }
                Err(tokio::sync::broadcast::error::RecvError::Lagged(_)) => continue,
                Err(tokio::sync::broadcast::error::RecvError::Closed) => break,
            }
        }
    });

    Ok(VibeHandle {
        core,
        callback,
        user_data: user_data as usize,
        worker: Mutex::new(None),
        runtime,
    })
}

fn start_send(handle: &VibeHandle, input: String) -> Result<()> {
    let prompt = input.trim().to_string();
    if prompt.is_empty() {
        bail!("input cannot be empty");
    }
    let mut worker_slot = handle
        .worker
        .lock()
        .map_err(|_| anyhow!("agent worker lock poisoned"))?;
    if worker_slot
        .as_ref()
        .is_some_and(|worker| !worker.is_finished())
    {
        bail!("agent is already running");
    }
    if let Some(worker) = worker_slot.take() {
        let _ = worker.join();
    }

    let core = handle.core.clone();
    let callback = handle.callback;
    let user_data = handle.user_data;
    *worker_slot = Some(std::thread::spawn(move || {
        let runtime = tokio::runtime::Builder::new_multi_thread()
            .enable_all()
            .build();
        match runtime {
            Ok(runtime) => {
                let result = runtime.block_on(async {
                    core.send(&prompt, &mut |event| {
                        emit_event(callback, user_data, &event);
                    })
                    .await
                });
                if let Err(error) = result {
                    emit_event(callback, user_data, &AgentEvent::Error(error.to_string()));
                }
            }
            Err(error) => emit_event(
                callback,
                user_data,
                &AgentEvent::Error(format!("failed to start async runtime: {error}")),
            ),
        }
    }));
    Ok(())
}

fn review_node(handle: *mut VibeHandle, node_id: *const c_char) -> Result<()> {
    let node_id = string_arg_owned(node_id)?;
    with_handle(handle, |handle| {
        let prompt = block_on(handle.core.review_node_prompt(&node_id))?;
        start_send(handle, prompt)
    })
}

fn review_all(handle: *mut VibeHandle) -> Result<()> {
    with_handle(handle, |handle| {
        let prompt = block_on(handle.core.review_all_prompt())?;
        start_send(handle, prompt)
    })
}

fn with_idle_handle<T>(
    raw: *mut VibeHandle,
    operation: impl FnOnce(&VibeHandle) -> Result<T>,
) -> Result<T> {
    with_handle(raw, |handle| {
        let worker_slot = handle
            .worker
            .lock()
            .map_err(|_| anyhow!("agent worker lock poisoned"))?;
        if worker_slot
            .as_ref()
            .is_some_and(|worker| !worker.is_finished())
        {
            bail!("tree cannot be edited while agent is running");
        }
        operation(handle)
    })
}

fn emit_event(callback: Option<EventCallback>, user_data: usize, event: &AgentEvent) {
    let Some(callback) = callback else {
        return;
    };
    let json = serde_json::to_string(event)
        .unwrap_or_else(|error| json!({"Error": error.to_string()}).to_string());
    // Never drop events that contain NUL; sanitize like owned_c_string.
    let sanitized = json.replace('\0', "\\u0000");
    if let Ok(value) = CString::new(sanitized) {
        callback(value.as_ptr(), user_data as *mut c_void);
    }
}

fn ffi_json_result(operation: impl FnOnce() -> Result<Value>) -> *mut c_char {
    let response = match catch_unwind(AssertUnwindSafe(operation)) {
        Ok(Ok(data)) => json!({"ok": true, "data": data}),
        Ok(Err(error)) => {
            set_last_error(error.to_string());
            json!({"ok": false, "error": error.to_string()})
        }
        Err(payload) => {
            let error = panic_message(payload);
            set_last_error(error.clone());
            json!({"ok": false, "error": error})
        }
    };
    owned_c_string(response.to_string())
}

fn block_on<T>(future: impl std::future::Future<Output = Result<T>>) -> Result<T> {
    let runtime = tokio::runtime::Builder::new_current_thread()
        .enable_all()
        .build()?;
    runtime.block_on(future)
}

fn parse_json_arg<T: for<'de> Deserialize<'de>>(value: *const c_char) -> Result<T> {
    Ok(serde_json::from_str(&string_arg_owned(value)?)?)
}

fn string_arg_owned(value: *const c_char) -> Result<String> {
    if value.is_null() {
        bail!("string argument is null");
    }
    // SAFETY: Public C functions require a valid, NUL-terminated pointer for call duration.
    let c_value = unsafe { CStr::from_ptr(value) };
    Ok(c_value.to_str()?.to_string())
}

fn with_handle<T>(
    handle: *mut VibeHandle,
    operation: impl FnOnce(&VibeHandle) -> Result<T>,
) -> Result<T> {
    if handle.is_null() {
        bail!("handle is null");
    }
    // SAFETY: Public C functions require a live pointer returned by `vibe_init`.
    operation(unsafe { &*handle })
}

fn ffi_status(operation: impl FnOnce() -> Result<()>) -> i32 {
    match catch_unwind(AssertUnwindSafe(operation)) {
        Ok(Ok(())) => 0,
        Ok(Err(error)) => {
            set_last_error(error.to_string());
            -1
        }
        Err(payload) => {
            set_last_error(panic_message(payload));
            -1
        }
    }
}

fn panic_message(payload: Box<dyn std::any::Any + Send>) -> String {
    if let Some(message) = payload.downcast_ref::<&str>() {
        format!("Rust panic: {message}")
    } else if let Some(message) = payload.downcast_ref::<String>() {
        format!("Rust panic: {message}")
    } else {
        "Rust panic in FFI call".to_string()
    }
}

fn owned_c_string(value: String) -> *mut c_char {
    let sanitized = value.replace('\0', "\\u0000");
    match CString::new(sanitized) {
        Ok(value) => value.into_raw(),
        Err(_) => ptr::null_mut(),
    }
}

fn set_last_error(message: String) {
    if let Ok(mut value) = LAST_ERROR.get_or_init(|| Mutex::new(String::new())).lock() {
        *value = message;
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn take_json(value: *mut c_char) -> Result<Value> {
        Ok(serde_json::from_str(&take_string(value)?)?)
    }

    fn take_string(value: *mut c_char) -> Result<String> {
        if value.is_null() {
            bail!("FFI returned a null string");
        }
        // SAFETY: Test owns the string returned by the FFI function until it is freed.
        let encoded = unsafe { CStr::from_ptr(value) }.to_str()?.to_string();
        // SAFETY: Pointer came from this library and has not been freed.
        unsafe { vibe_string_free(value) };
        Ok(encoded)
    }

    #[test]
    fn tree_round_trip_through_c_abi() -> Result<()> {
        let workspace =
            std::env::temp_dir().join(format!("mooncoding-ffi-{}", uuid::Uuid::new_v4()));
        std::fs::create_dir_all(&workspace)?;
        let options = CString::new(
            json!({
                "workspace": workspace,
                "session_id": "ffi-test"
            })
            .to_string(),
        )?;
        let handle = vibe_init(options.as_ptr(), None, ptr::null_mut());
        if handle.is_null() {
            let error = take_string(vibe_last_error())
                .unwrap_or_else(|_| "unknown initialization error".to_string());
            bail!("vibe_init failed: {error}");
        }

        let add_request = CString::new(
            json!({
                "expected_version": 0,
                "node": {
                    "id": "root",
                    "parent_id": null,
                    "title": "Project",
                    "description": "Human goal",
                    "kind": "project",
                    "status": "pending",
                    "priority": 90,
                    "target_files": []
                }
            })
            .to_string(),
        )?;
        let add_response = take_json(vibe_tree_add_node(handle, add_request.as_ptr()))?;
        assert_eq!(add_response["ok"], true);

        let tree_response = take_json(vibe_tree_get_json(handle))?;
        assert_eq!(tree_response["data"]["version"], 1);
        assert_eq!(tree_response["data"]["nodes"][0]["id"], "root");
        assert_eq!(tree_response["data"]["nodes"][0]["created_by"], "human");

        // SAFETY: Handle is live and uniquely owned by this test.
        unsafe { vibe_destroy(handle) };
        std::fs::remove_dir_all(workspace)?;
        Ok(())
    }
}
