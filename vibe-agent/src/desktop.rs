use anyhow::Result;
use serde_json::{json, Value};
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Arc;
use tokio::sync::broadcast;

use crate::app_runtime::{
    AppRuntimeEvent, AppRuntimeInspect, AppRuntimeManager, AppRuntimeStatus, StartOptions,
};
use crate::apps::{self, AppListing};
use crate::config::Config;
use crate::session::{Session, SessionStore, SqliteStore};
use crate::stream::AgentEvent;
use crate::tools::{default_registry, ToolRegistry};
use crate::tree::{NewTreeNode, NodePatch, TreeActor, TreeField, TreeManager};

/// UI-independent application facade.
///
/// Qt talks to this facade through the C ABI. Keeping session and tree operations
/// here makes the same behavior directly testable from Rust without a GUI runtime.
pub struct DesktopCore {
    config: Arc<Config>,
    tools: Arc<ToolRegistry>,
    store: Arc<dyn SessionStore>,
    session_id: String,
    interrupted: AtomicBool,
    app_runtime: Arc<AppRuntimeManager>,
}

impl DesktopCore {
    pub fn open(config: Config, session_id: String) -> Result<Self> {
        let store = SqliteStore::new(&config.session_dir.join("sessions.db"))?;
        Ok(Self::new(
            config,
            session_id,
            Arc::new(default_registry()),
            Arc::new(store),
        )?)
    }

    pub fn new(
        config: Config,
        session_id: String,
        tools: Arc<ToolRegistry>,
        store: Arc<dyn SessionStore>,
    ) -> Result<Self> {
        let app_runtime = Arc::new(AppRuntimeManager::for_workspace(&config.workspace)?);
        Ok(Self {
            config: Arc::new(config),
            tools,
            store,
            session_id,
            interrupted: AtomicBool::new(false),
            app_runtime,
        })
    }

    pub fn app_runtime(&self) -> Arc<AppRuntimeManager> {
        self.app_runtime.clone()
    }

    pub fn config(&self) -> &Config {
        &self.config
    }

    pub fn session_id(&self) -> &str {
        &self.session_id
    }

    pub fn interrupt(&self) {
        self.interrupted.store(true, Ordering::SeqCst);
    }

    pub async fn send(&self, input: &str, on_event: &mut dyn FnMut(AgentEvent)) -> Result<()> {
        self.interrupted.store(false, Ordering::SeqCst);
        crate::agent::run_agent_with_interrupt(
            &self.config,
            &self.tools,
            self.store.as_ref(),
            input,
            &self.session_id,
            &self.interrupted,
            Some(self.app_runtime.clone()),
            on_event,
        )
        .await
    }

    pub async fn tree_json(&self) -> Result<String> {
        let session = self.load_or_create_session().await?;
        TreeManager::new(session.project_tree.unwrap_or_default())?.to_json()
    }

    pub async fn sessions_json(&self) -> Result<String> {
        let mut summaries = Vec::new();
        for id in self.store.list().await? {
            let Some(session) = self.store.load(&id).await? else {
                continue;
            };
            let title = session
                .metadata
                .get("title")
                .cloned()
                .or_else(|| {
                    session
                        .messages
                        .iter()
                        .find(|message| message.role == "user")
                        .and_then(|message| message.content.as_deref())
                        .map(session_title)
                })
                .unwrap_or_else(|| "New conversation".to_string());
            summaries.push(json!({
                "id": session.id,
                "title": title,
                "updated_at": session.updated_at,
                "steps": session.step,
                "tokens_in": session.tokens_in,
                "tokens_out": session.tokens_out,
                "current": session.id == self.session_id,
            }));
        }
        Ok(serde_json::to_string(&summaries)?)
    }

    pub async fn session_json(&self, session_id: &str) -> Result<String> {
        let session = if session_id == self.session_id {
            self.load_or_create_session().await?
        } else {
            self.store
                .load(session_id)
                .await?
                .ok_or_else(|| anyhow::anyhow!("session not found: {session_id}"))?
        };
        // Export a UI transcript that keeps tool calls/results (DB already stores them;
        // the old filter dropped role=tool and empty-content assistant tool turns).
        let mut tool_names = std::collections::HashMap::<String, String>::new();
        for message in &session.messages {
            if crate::prompt::is_runtime_context_message(message) {
                continue;
            }
            if message.role != "assistant" {
                continue;
            }
            if let Some(calls) = message.tool_calls.as_ref() {
                for call in calls {
                    tool_names.insert(call.id.clone(), call.function.name.clone());
                }
            }
        }

        let messages = session
            .messages
            .iter()
            .filter(|message| !crate::prompt::is_runtime_context_message(message))
            .filter_map(|message| match message.role.as_str() {
                "user" => {
                    let content = message.content.as_deref()?.trim();
                    if content.is_empty() {
                        return None;
                    }
                    Some(json!({"role": "user", "content": content}))
                }
                "assistant" => {
                    let content = message.content.clone().unwrap_or_default();
                    let has_tools = message
                        .tool_calls
                        .as_ref()
                        .map(|calls| !calls.is_empty())
                        .unwrap_or(false);
                    if content.trim().is_empty() && !has_tools {
                        return None;
                    }
                    let mut obj = json!({
                        "role": "assistant",
                        "content": content,
                    });
                    if has_tools {
                        if let Ok(value) = serde_json::to_value(&message.tool_calls) {
                            obj["tool_calls"] = value;
                        }
                    }
                    Some(obj)
                }
                "tool" => {
                    let id = message.tool_call_id.clone().unwrap_or_default();
                    let name = tool_names
                        .get(&id)
                        .cloned()
                        .unwrap_or_else(|| "tool".to_string());
                    Some(json!({
                        "role": "tool",
                        "content": message.content.clone().unwrap_or_default(),
                        "tool_call_id": id,
                        "name": name,
                    }))
                }
                _ => None,
            })
            .collect::<Vec<Value>>();
        Ok(serde_json::to_string(&json!({
            "id": session.id,
            "messages": messages,
            "tree": session.project_tree,
            "tree_version": session.tree_version,
            "steps": session.step,
            "tokens_in": session.tokens_in,
            "tokens_out": session.tokens_out,
            "updated_at": session.updated_at,
        }))?)
    }

    pub async fn add_human_node(
        &self,
        input: NewTreeNode,
        expected_version: u64,
    ) -> Result<String> {
        let mut session = self.load_or_create_session().await?;
        let mut manager = TreeManager::new(session.project_tree.take().unwrap_or_default())?;
        let id = manager.add_node(input, TreeActor::Human, expected_version)?;
        self.save_tree(&mut session, manager, expected_version)
            .await?;
        Ok(id)
    }

    pub async fn update_human_node(
        &self,
        node_id: &str,
        patch: NodePatch,
        expected_version: u64,
    ) -> Result<()> {
        let mut session = self.load_or_create_session().await?;
        let mut manager = TreeManager::new(session.project_tree.take().unwrap_or_default())?;
        manager.update_node(node_id, patch, TreeActor::Human, expected_version)?;
        self.save_tree(&mut session, manager, expected_version)
            .await
    }

    pub async fn delete_human_node(
        &self,
        node_id: &str,
        expected_version: u64,
    ) -> Result<Vec<String>> {
        let mut session = self.load_or_create_session().await?;
        let mut manager = TreeManager::new(session.project_tree.take().unwrap_or_default())?;
        let deleted = manager.delete_node(node_id, TreeActor::Human, expected_version)?;
        self.save_tree(&mut session, manager, expected_version)
            .await?;
        Ok(deleted)
    }

    pub async fn release_human_fields(
        &self,
        node_id: &str,
        fields: &[TreeField],
        expected_version: u64,
    ) -> Result<()> {
        let mut session = self.load_or_create_session().await?;
        let mut manager = TreeManager::new(session.project_tree.take().unwrap_or_default())?;
        manager.release_fields(node_id, fields, expected_version)?;
        self.save_tree(&mut session, manager, expected_version)
            .await
    }

    pub async fn review_node_prompt(&self, node_id: &str) -> Result<String> {
        let session = self.load_or_create_session().await?;
        let manager = TreeManager::new(session.project_tree.unwrap_or_default())?;
        Ok(format!(
            "Perform a strict review of this project-tree node. Re-open associated files, \
             verify implementation and tests, and update only AI-owned tree fields. Do not \
             accept previous completion claims without evidence.\n\n{}",
            manager.review_context(node_id)?
        ))
    }

    pub async fn review_all_prompt(&self) -> Result<String> {
        let session = self.load_or_create_session().await?;
        let manager = TreeManager::new(session.project_tree.unwrap_or_default())?;
        Ok(format!(
            "Perform a strict review of the complete project tree. Re-open relevant files, \
             check dependencies and test evidence, identify missing branches, and update only \
             AI-owned fields.\n\n{}",
            manager.full_review_context()
        ))
    }

    /// List all apps in the workspace.
    pub fn list_apps(&self) -> Vec<AppListing> {
        apps::list_apps(&self.config.workspace)
    }

    /// Get a single app by name.
    pub fn get_app(&self, name: &str) -> Option<AppListing> {
        apps::get_app(&self.config.workspace, name)
    }

    /// Read the entry file content of an app.
    pub fn read_app_entry(&self, name: &str) -> Result<String> {
        apps::read_app_entry(&self.config.workspace, name).map_err(|e| anyhow::anyhow!(e))
    }

    /// Subscribe before starting an app so no lifecycle events are missed.
    pub fn subscribe_app_events(&self) -> broadcast::Receiver<AppRuntimeEvent> {
        self.app_runtime.subscribe()
    }

    /// Start a Python micro-app directly, without entering the agent/chat loop.
    /// Replaces any stuck/live instance so UI never gets trapped on a ghost runtime.
    pub async fn start_app(&self, name: &str) -> Result<String> {
        self.start_app_with_options(name, StartOptions { replace: true })
            .await
    }

    pub async fn start_app_with_options(
        &self,
        name: &str,
        options: StartOptions,
    ) -> Result<String> {
        let app = self
            .get_app(name)
            .ok_or_else(|| anyhow::anyhow!("app not found: {name}"))?;
        self.app_runtime.start_with_options(&app, options).await
    }

    pub async fn send_app_message(&self, message: Value) -> Result<()> {
        self.app_runtime.send(message).await
    }

    pub async fn stop_app(&self) -> Result<()> {
        self.app_runtime.stop("user requested stop").await
    }

    pub async fn force_stop_app(&self) -> Result<()> {
        self.app_runtime.force_stop("user requested force stop").await
    }

    pub fn app_status(&self) -> AppRuntimeStatus {
        self.app_runtime.status()
    }

    pub fn app_inspect(&self) -> AppRuntimeInspect {
        self.app_runtime.inspect()
    }

    async fn load_or_create_session(&self) -> Result<Session> {
        if let Some(session) = self.store.load(&self.session_id).await? {
            return Ok(session);
        }
        Ok(Session::new(
            self.session_id.clone(),
            self.config.provider.model.clone(),
            self.config.provider.base_url.clone(),
        ))
    }

    async fn save_tree(
        &self,
        session: &mut Session,
        manager: TreeManager,
        expected_version: u64,
    ) -> Result<()> {
        session.tree_version = manager.version();
        session.project_tree = Some(manager.into_tree());
        if !self.store.save_tree_cas(session, expected_version).await? {
            anyhow::bail!("tree version stale while saving: expected {expected_version}");
        }
        Ok(())
    }
}

fn session_title(content: &str) -> String {
    let compact = content.split_whitespace().collect::<Vec<_>>().join(" ");
    let mut title = compact.chars().take(48).collect::<String>();
    if compact.chars().count() > 48 {
        title.push('…');
    }
    if title.is_empty() {
        "New conversation".to_string()
    } else {
        title
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::config::{AgentToml, ApiSource, ProviderConfig};
    use crate::tree::{TreeNodeKind, TreeStatus};
    use async_trait::async_trait;
    use std::collections::HashMap;
    use std::path::PathBuf;
    use std::sync::Mutex;

    #[derive(Default)]
    struct MemoryStore {
        sessions: Mutex<HashMap<String, Session>>,
    }

    #[async_trait]
    impl SessionStore for MemoryStore {
        async fn load(&self, id: &str) -> Result<Option<Session>> {
            Ok(self
                .sessions
                .lock()
                .map_err(|_| anyhow::anyhow!("memory store lock poisoned"))?
                .get(id)
                .cloned())
        }

        async fn save(&self, session: &Session) -> Result<()> {
            let mut sessions = self
                .sessions
                .lock()
                .map_err(|_| anyhow::anyhow!("memory store lock poisoned"))?;
            let mut updated = session.clone();
            if let Some(existing) = sessions.get(&session.id) {
                updated.project_tree = existing.project_tree.clone();
                updated.tree_version = existing.tree_version;
            }
            sessions.insert(session.id.clone(), updated);
            Ok(())
        }

        async fn save_tree_cas(&self, session: &Session, expected_version: u64) -> Result<bool> {
            let mut sessions = self
                .sessions
                .lock()
                .map_err(|_| anyhow::anyhow!("memory store lock poisoned"))?;
            let current = sessions
                .get(&session.id)
                .map(|current| current.tree_version)
                .unwrap_or_default();
            if current != expected_version {
                return Ok(false);
            }
            sessions.insert(session.id.clone(), session.clone());
            Ok(true)
        }

        async fn list(&self) -> Result<Vec<String>> {
            Ok(self
                .sessions
                .lock()
                .map_err(|_| anyhow::anyhow!("memory store lock poisoned"))?
                .keys()
                .cloned()
                .collect())
        }

        async fn latest(&self) -> Result<Option<String>> {
            Ok(self
                .sessions
                .lock()
                .map_err(|_| anyhow::anyhow!("memory store lock poisoned"))?
                .keys()
                .next()
                .cloned())
        }
    }

    fn config() -> Config {
        Config {
            language: "zh".to_string(),
            api_source: ApiSource::Custom,
            managed_api: None,
            provider: ProviderConfig {
                base_url: "https://example.invalid".to_string(),
                model: "test-model".to_string(),
                api_key: String::new(),
                max_tokens: 1024,
                temperature: 0.0,
            },
            agent: AgentToml::default(),
            workspace: PathBuf::from("."),
            vibe_exe: PathBuf::from("vibe"),
            session_dir: PathBuf::from("."),
            deployment_target: crate::config::DeploymentTarget::Desktop,
        }
    }

    fn node(id: &str, parent_id: Option<&str>, title: &str) -> NewTreeNode {
        NewTreeNode {
            id: Some(id.to_string()),
            parent_id: parent_id.map(str::to_string),
            title: title.to_string(),
            description: "description".to_string(),
            kind: TreeNodeKind::Task,
            status: TreeStatus::Pending,
            priority: 80,
            human_note: None,
            ai_note: None,
            target_files: vec!["src/main.rs".to_string()],
            evidence: Vec::new(),
            delegate_status_to_ai: true,
        }
    }

    #[tokio::test]
    async fn human_tree_changes_persist_between_core_instances() -> Result<()> {
        let store = Arc::new(MemoryStore::default());
        let tools = Arc::new(ToolRegistry::new());
        let first = DesktopCore::new(
            config(),
            "session".to_string(),
            tools.clone(),
            store.clone(),
        )?;
        first
            .add_human_node(node("root", None, "Project"), 0)
            .await?;
        first
            .add_human_node(node("test", Some("root"), "Verify"), 1)
            .await?;

        let second = DesktopCore::new(config(), "session".to_string(), tools, store)?;
        let tree: crate::tree::ProjectTree = serde_json::from_str(&second.tree_json().await?)?;
        assert_eq!(tree.version, 2);
        assert_eq!(tree.nodes.len(), 2);
        assert_eq!(tree.nodes[0].created_by, TreeActor::Human);
        Ok(())
    }

    #[tokio::test]
    async fn review_prompt_includes_human_authority_and_files() -> Result<()> {
        let core = DesktopCore::new(
            config(),
            "review".to_string(),
            Arc::new(ToolRegistry::new()),
            Arc::new(MemoryStore::default()),
        )?;
        core.add_human_node(node("test", None, "Verify Linux"), 0)
            .await?;
        let prompt = core.review_node_prompt("test").await?;

        assert!(prompt.contains("Human-owned fields are authoritative"));
        assert!(prompt.contains("src/main.rs"));
        assert!(prompt.contains("Verify Linux"));
        Ok(())
    }

    #[tokio::test]
    async fn session_json_exports_tool_calls_for_ui() -> Result<()> {
        use crate::provider::{FunctionCall, Message, ToolCall};

        let store = Arc::new(MemoryStore::default());
        let tools = Arc::new(ToolRegistry::new());
        let core = DesktopCore::new(
            config(),
            "tools-ui".to_string(),
            tools,
            store.clone(),
        )?;

        let mut session = Session::new(
            "tools-ui".to_string(),
            "test-model".to_string(),
            "custom".to_string(),
        );
        session.messages = vec![
            Message {
                role: "user".to_string(),
                content: Some("list files".to_string()),
                tool_calls: None,
                tool_call_id: None,
            },
            Message {
                role: "assistant".to_string(),
                content: Some(String::new()),
                tool_calls: Some(vec![ToolCall {
                    id: "call_1".to_string(),
                    r#type: "function".to_string(),
                    function: FunctionCall {
                        name: "bash".to_string(),
                        arguments: r#"{"command":"ls"}"#.to_string(),
                    },
                }]),
                tool_call_id: None,
            },
            Message {
                role: "tool".to_string(),
                content: Some("a.rs\nb.rs".to_string()),
                tool_calls: None,
                tool_call_id: Some("call_1".to_string()),
            },
            Message {
                role: "assistant".to_string(),
                content: Some("Here are the files.".to_string()),
                tool_calls: None,
                tool_call_id: None,
            },
        ];
        store.save(&session).await?;

        let json: Value = serde_json::from_str(&core.session_json("tools-ui").await?)?;
        let messages = json["messages"].as_array().expect("messages array");
        assert_eq!(messages.len(), 4);
        assert_eq!(messages[0]["role"], "user");
        assert_eq!(messages[1]["role"], "assistant");
        assert_eq!(messages[1]["tool_calls"].as_array().unwrap().len(), 1);
        assert_eq!(messages[2]["role"], "tool");
        assert_eq!(messages[2]["name"], "bash");
        assert_eq!(messages[2]["tool_call_id"], "call_1");
        assert_eq!(messages[3]["content"], "Here are the files.");
        Ok(())
    }
}
