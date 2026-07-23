use anyhow::{anyhow, bail, Result};
use chrono::Utc;
use serde::{Deserialize, Serialize};
use std::collections::{BTreeMap, BTreeSet, HashSet};
use std::path::{Component, Path};

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum TreeActor {
    Ai,
    Human,
    System,
}

impl Default for TreeActor {
    fn default() -> Self {
        Self::System
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum TreeStatus {
    Pending,
    InProgress,
    Completed,
    Failed,
    NeedsReview,
    Blocked,
    Rejected,
    Cancelled,
}

impl Default for TreeStatus {
    fn default() -> Self {
        Self::Pending
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum TreeNodeKind {
    Project,
    Feature,
    Module,
    Task,
    Test,
    Decision,
    Alternative,
    Research,
    Milestone,
}

impl Default for TreeNodeKind {
    fn default() -> Self {
        Self::Task
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum TreeField {
    Parent,
    Title,
    Description,
    Kind,
    Status,
    Priority,
    HumanNote,
    AiNote,
    TargetFiles,
    Evidence,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct VerificationEvidence {
    pub kind: String,
    pub summary: String,
    #[serde(default)]
    pub command: Option<String>,
    #[serde(default)]
    pub tool: Option<String>,
    #[serde(default)]
    pub working_directory: Option<String>,
    pub success: bool,
    #[serde(default)]
    pub recorded_by: TreeActor,
    #[serde(default)]
    pub recorded_at: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TreeNode {
    pub id: String,
    pub parent_id: Option<String>,
    pub title: String,
    #[serde(default)]
    pub description: String,
    #[serde(default)]
    pub kind: TreeNodeKind,
    #[serde(default)]
    pub status: TreeStatus,
    #[serde(default = "default_priority")]
    pub priority: u8,
    #[serde(default)]
    pub created_by: TreeActor,
    #[serde(default)]
    pub last_modified_by: TreeActor,
    #[serde(default)]
    pub human_locked_fields: BTreeSet<TreeField>,
    #[serde(default)]
    pub human_note: Option<String>,
    #[serde(default)]
    pub ai_note: Option<String>,
    #[serde(default)]
    pub target_files: Vec<String>,
    #[serde(default)]
    pub evidence: Vec<VerificationEvidence>,
    #[serde(default)]
    pub revision: u64,
    #[serde(default)]
    pub created_at: String,
    #[serde(default)]
    pub updated_at: String,
}

fn default_priority() -> u8 {
    50
}

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct ProjectTree {
    pub version: u64,
    #[serde(default)]
    pub nodes: Vec<TreeNode>,
}

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct NodePatch {
    #[serde(default, deserialize_with = "present_option::deserialize")]
    pub parent_id: Option<Option<String>>,
    pub title: Option<String>,
    pub description: Option<String>,
    pub kind: Option<TreeNodeKind>,
    pub status: Option<TreeStatus>,
    pub priority: Option<u8>,
    #[serde(default, deserialize_with = "present_option::deserialize")]
    pub human_note: Option<Option<String>>,
    #[serde(default, deserialize_with = "present_option::deserialize")]
    pub ai_note: Option<Option<String>>,
    pub target_files: Option<Vec<String>>,
    pub evidence: Option<Vec<VerificationEvidence>>,
}

mod present_option {
    use serde::{Deserialize, Deserializer};

    pub fn deserialize<'de, D, T>(deserializer: D) -> Result<Option<Option<T>>, D::Error>
    where
        D: Deserializer<'de>,
        T: Deserialize<'de>,
    {
        Ok(Some(Option::<T>::deserialize(deserializer)?))
    }
}

impl NodePatch {
    fn touched_fields(&self) -> BTreeSet<TreeField> {
        let mut fields = BTreeSet::new();
        if self.parent_id.is_some() {
            fields.insert(TreeField::Parent);
        }
        if self.title.is_some() {
            fields.insert(TreeField::Title);
        }
        if self.description.is_some() {
            fields.insert(TreeField::Description);
        }
        if self.kind.is_some() {
            fields.insert(TreeField::Kind);
        }
        if self.status.is_some() {
            fields.insert(TreeField::Status);
        }
        if self.priority.is_some() {
            fields.insert(TreeField::Priority);
        }
        if self.human_note.is_some() {
            fields.insert(TreeField::HumanNote);
        }
        if self.ai_note.is_some() {
            fields.insert(TreeField::AiNote);
        }
        if self.target_files.is_some() {
            fields.insert(TreeField::TargetFiles);
        }
        if self.evidence.is_some() {
            fields.insert(TreeField::Evidence);
        }
        fields
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct NewTreeNode {
    pub id: Option<String>,
    pub parent_id: Option<String>,
    pub title: String,
    #[serde(default)]
    pub description: String,
    #[serde(default)]
    pub kind: TreeNodeKind,
    #[serde(default)]
    pub status: TreeStatus,
    #[serde(default = "default_priority")]
    pub priority: u8,
    #[serde(default)]
    pub human_note: Option<String>,
    #[serde(default)]
    pub ai_note: Option<String>,
    #[serde(default)]
    pub target_files: Vec<String>,
    #[serde(default)]
    pub evidence: Vec<VerificationEvidence>,
    #[serde(default)]
    pub delegate_status_to_ai: bool,
}

#[derive(Debug, Clone)]
pub struct TreeManager {
    tree: ProjectTree,
}

impl TreeManager {
    pub fn new(tree: ProjectTree) -> Result<Self> {
        // Legacy field locks are retired: clear on load so AI/human can both update status.
        let mut tree = tree;
        for node in &mut tree.nodes {
            node.human_locked_fields.clear();
        }
        let manager = Self { tree };
        manager.validate()?;
        Ok(manager)
    }

    pub fn empty() -> Self {
        Self {
            tree: ProjectTree::default(),
        }
    }

    pub fn tree(&self) -> &ProjectTree {
        &self.tree
    }

    pub fn into_tree(self) -> ProjectTree {
        self.tree
    }

    pub fn version(&self) -> u64 {
        self.tree.version
    }

    pub fn is_empty(&self) -> bool {
        self.tree.nodes.is_empty()
    }

    pub fn get(&self, id: &str) -> Option<&TreeNode> {
        self.tree.nodes.iter().find(|node| node.id == id)
    }

    pub fn add_node(
        &mut self,
        input: NewTreeNode,
        actor: TreeActor,
        expected_version: u64,
    ) -> Result<String> {
        self.ensure_version(expected_version)?;
        if input.title.trim().is_empty() {
            bail!("tree node title cannot be empty");
        }
        if input.priority > 100 {
            bail!("tree node priority must be between 0 and 100");
        }
        validate_target_files(&input.target_files)?;
        let evidence = normalize_evidence(input.evidence.clone(), actor);
        if actor == TreeActor::Ai
            && input.status == TreeStatus::Completed
            && !evidence.last().is_some_and(|item| item.success)
        {
            bail!("AI cannot create a completed node without successful verification evidence");
        }
        if let Some(parent_id) = input.parent_id.as_deref() {
            if self.get(parent_id).is_none() {
                bail!("parent node not found: {parent_id}");
            }
        }

        let id = input.id.unwrap_or_else(|| uuid::Uuid::new_v4().to_string());
        if self.get(&id).is_some() {
            bail!("tree node already exists: {id}");
        }

        let now = Utc::now().to_rfc3339();
        self.tree.nodes.push(TreeNode {
            id: id.clone(),
            parent_id: input.parent_id,
            title: input.title,
            description: input.description,
            kind: input.kind,
            status: input.status,
            priority: input.priority,
            created_by: actor,
            last_modified_by: actor,
            human_locked_fields: BTreeSet::new(),
            human_note: input.human_note,
            ai_note: input.ai_note,
            target_files: normalize_files(input.target_files),
            evidence,
            revision: 1,
            created_at: now.clone(),
            updated_at: now,
        });
        self.bump_version();
        Ok(id)
    }

    pub fn update_node(
        &mut self,
        id: &str,
        patch: NodePatch,
        actor: TreeActor,
        expected_version: u64,
    ) -> Result<()> {
        self.ensure_version(expected_version)?;
        let touched = patch.touched_fields();
        if touched.is_empty() {
            bail!("tree node patch is empty");
        }

        let index = self
            .tree
            .nodes
            .iter()
            .position(|node| node.id == id)
            .ok_or_else(|| anyhow!("tree node not found: {id}"))?;

        if let Some(parent_id) = patch.parent_id.as_ref().and_then(|value| value.as_deref()) {
            if parent_id == id {
                bail!("tree node cannot be its own parent");
            }
            if self.get(parent_id).is_none() {
                bail!("parent node not found: {parent_id}");
            }
            if self.descendant_ids(id).contains(parent_id) {
                bail!("moving node would create a cycle");
            }
        }
        if patch
            .title
            .as_ref()
            .is_some_and(|title| title.trim().is_empty())
        {
            bail!("tree node title cannot be empty");
        }
        if patch.priority.is_some_and(|priority| priority > 100) {
            bail!("tree node priority must be between 0 and 100");
        }
        if let Some(files) = patch.target_files.as_ref() {
            validate_target_files(files)?;
        }
        let normalized_evidence = patch
            .evidence
            .as_ref()
            .map(|evidence| normalize_evidence(evidence.clone(), actor));
        if actor == TreeActor::Ai && patch.status == Some(TreeStatus::Completed) {
            let evidence = normalized_evidence
                .as_ref()
                .unwrap_or(&self.tree.nodes[index].evidence);
            if !evidence.last().is_some_and(|item| item.success) {
                bail!("AI cannot complete a node without successful verification evidence");
            }
            let incomplete_children: Vec<&str> = self
                .tree
                .nodes
                .iter()
                .filter(|node| node.parent_id.as_deref() == Some(id))
                .filter(|node| {
                    !matches!(
                        node.status,
                        TreeStatus::Completed | TreeStatus::Cancelled | TreeStatus::Rejected
                    )
                })
                .map(|node| node.id.as_str())
                .collect();
            if !incomplete_children.is_empty() {
                bail!(
                    "AI cannot complete a parent with unfinished children: {}",
                    incomplete_children.join(", ")
                );
            }
        }

        let node = &mut self.tree.nodes[index];
        if let Some(value) = patch.parent_id {
            node.parent_id = value;
        }
        if let Some(value) = patch.title {
            node.title = value;
        }
        if let Some(value) = patch.description {
            node.description = value;
        }
        if let Some(value) = patch.kind {
            node.kind = value;
        }
        if let Some(value) = patch.status {
            node.status = value;
        }
        if let Some(value) = patch.priority {
            node.priority = value;
        }
        if let Some(value) = patch.human_note {
            node.human_note = value;
        }
        if let Some(value) = patch.ai_note {
            node.ai_note = value;
        }
        if let Some(value) = patch.target_files {
            node.target_files = normalize_files(value);
        }
        if patch.evidence.is_some() {
            node.evidence = normalized_evidence.unwrap_or_default();
        }
        node.human_locked_fields.clear();
        node.last_modified_by = actor;
        node.revision = node.revision.saturating_add(1);
        node.updated_at = Utc::now().to_rfc3339();
        self.bump_version();
        Ok(())
    }

    /// Legacy no-op: field locks are retired. Clears any leftover locks for ABI compat.
    pub fn release_fields(
        &mut self,
        id: &str,
        _fields: &[TreeField],
        expected_version: u64,
    ) -> Result<()> {
        self.ensure_version(expected_version)?;
        let node = self
            .tree
            .nodes
            .iter_mut()
            .find(|node| node.id == id)
            .ok_or_else(|| anyhow!("tree node not found: {id}"))?;
        if !node.human_locked_fields.is_empty() {
            node.human_locked_fields.clear();
            node.revision = node.revision.saturating_add(1);
            node.updated_at = Utc::now().to_rfc3339();
            self.bump_version();
        }
        Ok(())
    }

    pub fn delete_node(
        &mut self,
        id: &str,
        actor: TreeActor,
        expected_version: u64,
    ) -> Result<Vec<String>> {
        self.ensure_version(expected_version)?;
        if self.get(id).is_none() {
            bail!("tree node not found: {id}");
        }
        let mut removed = self.descendant_ids(id);
        removed.insert(id.to_string());
        if actor == TreeActor::Ai
            && self.tree.nodes.iter().any(|candidate| {
                removed.contains(&candidate.id) && candidate.created_by == TreeActor::Human
            })
        {
            bail!("AI cannot delete a branch containing human-created nodes");
        }
        self.tree.nodes.retain(|node| !removed.contains(&node.id));
        self.bump_version();
        Ok(removed.into_iter().collect())
    }

    pub fn review_context(&self, id: &str) -> Result<String> {
        let root = self
            .get(id)
            .ok_or_else(|| anyhow!("tree node not found: {id}"))?;
        let mut output = String::from(
            "Review this project-tree branch. Prefer preserving recent human notes when still relevant.\n",
        );
        self.render_node(root, 0, true, &mut output);
        Ok(output)
    }

    pub fn full_review_context(&self) -> String {
        let mut output = String::from(
            "Review the complete project tree. Verify completion claims, tests, dependencies, and human corrections.\n",
        );
        for root in self
            .tree
            .nodes
            .iter()
            .filter(|node| node.parent_id.is_none())
        {
            self.render_node(root, 0, true, &mut output);
        }
        output
    }

    pub fn prompt_summary(&self) -> String {
        if self.tree.nodes.is_empty() {
            return "Project tree is empty. Create a basic project tree before multi-step implementation."
                .to_string();
        }
        if self.tree.nodes.len() > 80 {
            return self.compact_prompt_summary();
        }
        let mut output = format!(
            "Tree version: {}. Human-owned fields are authoritative and must not be overwritten.\n",
            self.tree.version
        );
        for root in self
            .tree
            .nodes
            .iter()
            .filter(|node| node.parent_id.is_none())
        {
            self.render_node(root, 0, false, &mut output);
        }
        output
    }

    pub fn authorize_file_edit(&self, path: &str) -> Result<&TreeNode> {
        validate_target_files(&[path.to_string()])?;
        let normalized = normalize_file(path);
        if normalized.is_empty() {
            bail!("edited file path cannot be empty");
        }
        if self.tree.nodes.is_empty() {
            bail!("project tree is empty; create the project structure before editing code");
        }
        self.tree
            .nodes
            .iter()
            .find(|node| {
                node.status == TreeStatus::InProgress
                    && node
                        .target_files
                        .iter()
                        .any(|target| normalize_file(target) == normalized)
            })
            .ok_or_else(|| {
                anyhow!("file edit is not authorized by an in_progress tree node: {path}")
            })
    }

    pub fn to_json(&self) -> Result<String> {
        Ok(serde_json::to_string(&self.tree)?)
    }

    pub fn validate(&self) -> Result<()> {
        let mut ids = HashSet::new();
        for node in &self.tree.nodes {
            if node.id.is_empty() || !ids.insert(node.id.as_str()) {
                bail!("tree contains an empty or duplicate node id");
            }
            if node.title.trim().is_empty() {
                bail!("tree node title cannot be empty: {}", node.id);
            }
            if node.priority > 100 {
                bail!("tree node priority must be between 0 and 100: {}", node.id);
            }
            validate_target_files(&node.target_files)?;
            if node.revision > self.tree.version && self.tree.version > 0 {
                bail!(
                    "tree node revision exceeds tree version: {} > {}",
                    node.revision,
                    self.tree.version
                );
            }
            for evidence in &node.evidence {
                if evidence.kind.trim().is_empty() || evidence.summary.trim().is_empty() {
                    bail!(
                        "tree node {} contains invalid verification evidence",
                        node.id
                    );
                }
            }
            if node.status == TreeStatus::Completed
                && node.last_modified_by == TreeActor::Ai
                && !node.evidence.last().is_some_and(|evidence| {
                    evidence.success
                        && is_completion_evidence_kind(
                            evidence.kind.trim().to_ascii_lowercase().as_str(),
                        )
                        && evidence
                            .command
                            .as_deref()
                            .is_some_and(|command| !command.trim().is_empty())
                        && evidence
                            .tool
                            .as_deref()
                            .is_some_and(|tool| !tool.trim().is_empty())
                        && evidence
                            .working_directory
                            .as_deref()
                            .is_some_and(|cwd| !cwd.trim().is_empty())
                })
            {
                bail!(
                    "AI-completed tree node lacks successful command evidence: {}",
                    node.id
                );
            }
        }
        for node in &self.tree.nodes {
            if let Some(parent_id) = node.parent_id.as_deref() {
                if !ids.contains(parent_id) {
                    bail!("tree node {} has missing parent {}", node.id, parent_id);
                }
            }
            let mut ancestors = HashSet::new();
            let mut current = node.parent_id.as_deref();
            while let Some(parent_id) = current {
                if !ancestors.insert(parent_id) {
                    bail!("tree contains a cycle at node {}", node.id);
                }
                current = self
                    .get(parent_id)
                    .and_then(|parent| parent.parent_id.as_deref());
            }
        }
        Ok(())
    }

    fn ensure_version(&self, expected_version: u64) -> Result<()> {
        if self.tree.version != expected_version {
            bail!(
                "tree version stale: expected {}, current {}",
                expected_version,
                self.tree.version
            );
        }
        Ok(())
    }

    fn bump_version(&mut self) {
        self.tree.version = self.tree.version.saturating_add(1);
    }

    fn compact_prompt_summary(&self) -> String {
        let mut visible = BTreeSet::new();
        for node in &self.tree.nodes {
            if node.parent_id.is_none()
                || matches!(
                    node.status,
                    TreeStatus::InProgress
                        | TreeStatus::Failed
                        | TreeStatus::NeedsReview
                        | TreeStatus::Blocked
                        | TreeStatus::Rejected
                )
            {
                visible.insert(node.id.clone());
                let mut parent_id = node.parent_id.as_deref();
                while let Some(id) = parent_id {
                    if !visible.insert(id.to_string()) {
                        break;
                    }
                    parent_id = self.get(id).and_then(|parent| parent.parent_id.as_deref());
                }
            }
        }

        let mut status_counts = BTreeMap::<&'static str, usize>::new();
        for node in &self.tree.nodes {
            *status_counts.entry(status_name(node.status)).or_default() += 1;
        }
        let counts = status_counts
            .into_iter()
            .map(|(status, count)| format!("{status}={count}"))
            .collect::<Vec<_>>()
            .join(", ");
        let mut output = format!(
            "Tree version: {}. Large tree: showing {} of {} relevant nodes ({counts}). \
             Use tree.get or review_context before changing an omitted branch.\n",
            self.tree.version,
            visible.len(),
            self.tree.nodes.len()
        );
        for node in self
            .tree
            .nodes
            .iter()
            .filter(|node| visible.contains(&node.id))
        {
            output.push_str(&format!(
                "- [{}] {} (id={}, parent={}, kind={}, owner={}, files={})\n",
                status_name(node.status),
                node.title,
                node.id,
                node.parent_id.as_deref().unwrap_or("root"),
                kind_name(node.kind),
                actor_name(node.last_modified_by),
                node.target_files.join(", ")
            ));
        }
        output
    }

    fn descendant_ids(&self, id: &str) -> BTreeSet<String> {
        let mut result = BTreeSet::new();
        let mut frontier = vec![id.to_string()];
        while let Some(parent_id) = frontier.pop() {
            for child in self
                .tree
                .nodes
                .iter()
                .filter(|node| node.parent_id.as_deref() == Some(parent_id.as_str()))
            {
                if result.insert(child.id.clone()) {
                    frontier.push(child.id.clone());
                }
            }
        }
        result
    }

    fn render_node(&self, node: &TreeNode, depth: usize, detailed: bool, output: &mut String) {
        let indent = "  ".repeat(depth);
        output.push_str(&format!(
            "{}- [{}] {} (id={}, kind={}, priority={}, owner={})\n",
            indent,
            status_name(node.status),
            node.title,
            node.id,
            kind_name(node.kind),
            node.priority,
            actor_name(node.last_modified_by)
        ));
        if detailed {
            if !node.description.is_empty() {
                output.push_str(&format!("{}  description: {}\n", indent, node.description));
            }
            if let Some(note) = node.human_note.as_deref() {
                output.push_str(&format!("{}  HUMAN NOTE: {}\n", indent, note));
            }
            if let Some(note) = node.ai_note.as_deref() {
                output.push_str(&format!("{}  AI NOTE: {}\n", indent, note));
            }
            if !node.target_files.is_empty() {
                output.push_str(&format!(
                    "{}  files: {}\n",
                    indent,
                    node.target_files.join(", ")
                ));
            }
            for evidence in &node.evidence {
                output.push_str(&format!(
                    "{}  evidence: [{}] {}{}\n",
                    indent,
                    if evidence.success { "passed" } else { "failed" },
                    evidence.summary,
                    evidence
                        .command
                        .as_deref()
                        .map(|command| format!(" (command: {command})"))
                        .unwrap_or_default()
                ));
            }
        }
        for child in self
            .tree
            .nodes
            .iter()
            .filter(|child| child.parent_id.as_deref() == Some(node.id.as_str()))
        {
            self.render_node(child, depth + 1, detailed, output);
        }
    }
}

fn normalize_files(files: Vec<String>) -> Vec<String> {
    let mut unique = BTreeMap::new();
    for file in files {
        let normalized = normalize_file(&file);
        if !normalized.is_empty() {
            unique.insert(normalized.clone(), normalized);
        }
    }
    unique.into_values().collect()
}

fn normalize_file(file: &str) -> String {
    let portable = file.trim().replace('\\', "/");
    let normalized = Path::new(&portable)
        .components()
        .filter_map(|component| match component {
            Component::Normal(value) => Some(value.to_string_lossy().to_string()),
            Component::CurDir => None,
            _ => None,
        })
        .collect::<Vec<_>>()
        .join("/");
    #[cfg(windows)]
    {
        normalized.to_lowercase()
    }
    #[cfg(not(windows))]
    {
        normalized
    }
}

fn validate_target_files(files: &[String]) -> Result<()> {
    for file in files {
        let trimmed = file.trim();
        let bytes = trimmed.as_bytes();
        let has_windows_prefix =
            (bytes.len() >= 2 && bytes[0].is_ascii_alphabetic() && bytes[1] == b':')
                || trimmed.starts_with("\\\\");
        let path = Path::new(trimmed);
        if trimmed.is_empty()
            || path.is_absolute()
            || has_windows_prefix
            || !path
                .components()
                .any(|component| matches!(component, Component::Normal(_)))
            || !path
                .components()
                .all(|component| matches!(component, Component::Normal(_) | Component::CurDir))
        {
            bail!("target file must be workspace-relative and cannot contain '..': {file}");
        }
    }
    Ok(())
}

fn normalize_evidence(
    evidence: Vec<VerificationEvidence>,
    actor: TreeActor,
) -> Vec<VerificationEvidence> {
    evidence
        .into_iter()
        .filter(|item| {
            !item.kind.trim().is_empty()
                && !item.summary.trim().is_empty()
                && (actor != TreeActor::Ai
                    || !item.success
                    || (is_completion_evidence_kind(
                        item.kind.trim().to_ascii_lowercase().as_str(),
                    ) && item
                        .command
                        .as_deref()
                        .is_some_and(|command| !command.trim().is_empty())
                        && item
                            .tool
                            .as_deref()
                            .is_some_and(|tool| !tool.trim().is_empty())
                        && item
                            .working_directory
                            .as_deref()
                            .is_some_and(|cwd| !cwd.trim().is_empty())))
        })
        .map(|mut item| {
            item.kind = item.kind.trim().to_lowercase();
            item.summary = item.summary.trim().to_string();
            item.command = item
                .command
                .map(|command| command.trim().to_string())
                .filter(|command| !command.is_empty());
            item.recorded_by = actor;
            if item.recorded_at.is_empty() {
                item.recorded_at = Utc::now().to_rfc3339();
            }
            item
        })
        .collect()
}

fn is_completion_evidence_kind(kind: &str) -> bool {
    // `run` covers allowlisted python/py script checks for micro-apps.
    matches!(kind, "test" | "build" | "lint" | "integrity" | "run")
}

fn actor_name(actor: TreeActor) -> &'static str {
    match actor {
        TreeActor::Ai => "ai",
        TreeActor::Human => "human",
        TreeActor::System => "system",
    }
}

fn status_name(status: TreeStatus) -> &'static str {
    match status {
        TreeStatus::Pending => "pending",
        TreeStatus::InProgress => "in_progress",
        TreeStatus::Completed => "completed",
        TreeStatus::Failed => "failed",
        TreeStatus::NeedsReview => "needs_review",
        TreeStatus::Blocked => "blocked",
        TreeStatus::Rejected => "rejected",
        TreeStatus::Cancelled => "cancelled",
    }
}

fn kind_name(kind: TreeNodeKind) -> &'static str {
    match kind {
        TreeNodeKind::Project => "project",
        TreeNodeKind::Feature => "feature",
        TreeNodeKind::Module => "module",
        TreeNodeKind::Task => "task",
        TreeNodeKind::Test => "test",
        TreeNodeKind::Decision => "decision",
        TreeNodeKind::Alternative => "alternative",
        TreeNodeKind::Research => "research",
        TreeNodeKind::Milestone => "milestone",
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn node(id: &str, parent_id: Option<&str>, title: &str) -> NewTreeNode {
        NewTreeNode {
            id: Some(id.to_string()),
            parent_id: parent_id.map(str::to_string),
            title: title.to_string(),
            description: String::new(),
            kind: TreeNodeKind::Task,
            status: TreeStatus::Pending,
            priority: 50,
            human_note: None,
            ai_note: None,
            target_files: Vec::new(),
            evidence: Vec::new(),
            delegate_status_to_ai: false,
        }
    }

    #[test]
    fn builds_and_validates_nested_tree() -> Result<()> {
        let mut manager = TreeManager::empty();
        manager.add_node(node("root", None, "Project"), TreeActor::Ai, 0)?;
        manager.add_node(node("child", Some("root"), "Feature"), TreeActor::Ai, 1)?;

        assert_eq!(manager.version(), 2);
        assert_eq!(
            manager
                .get("child")
                .and_then(|item| item.parent_id.as_deref()),
            Some("root")
        );
        manager.validate()
    }

    #[test]
    fn rejects_stale_tree_mutation() -> Result<()> {
        let mut manager = TreeManager::empty();
        manager.add_node(node("root", None, "Project"), TreeActor::Ai, 0)?;

        let error = manager
            .add_node(node("late", None, "Late"), TreeActor::Ai, 0)
            .expect_err("stale version must be rejected");
        assert!(error.to_string().contains("tree version stale"));
        Ok(())
    }

    #[test]
    fn human_and_ai_can_both_update_status() -> Result<()> {
        let mut manager = TreeManager::empty();
        manager.add_node(node("task", None, "Original"), TreeActor::Ai, 0)?;
        manager.update_node(
            "task",
            NodePatch {
                status: Some(TreeStatus::NeedsReview),
                human_note: Some(Some("Tests did not pass".to_string())),
                ..NodePatch::default()
            },
            TreeActor::Human,
            1,
        )?;

        manager.update_node(
            "task",
            NodePatch {
                status: Some(TreeStatus::InProgress),
                ai_note: Some(Some("Will rerun tests".to_string())),
                ..NodePatch::default()
            },
            TreeActor::Ai,
            2,
        )?;
        assert!(manager
            .get("task")
            .is_some_and(|item| item.human_locked_fields.is_empty()));
        assert_eq!(
            manager.get("task").map(|item| item.status),
            Some(TreeStatus::InProgress)
        );
        Ok(())
    }

    #[test]
    fn ai_completion_requires_successful_evidence() -> Result<()> {
        let mut manager = TreeManager::empty();
        manager.add_node(node("task", None, "Implement"), TreeActor::Ai, 0)?;
        let unsupported = manager
            .update_node(
                "task",
                NodePatch {
                    status: Some(TreeStatus::Completed),
                    ..NodePatch::default()
                },
                TreeActor::Ai,
                1,
            )
            .expect_err("completion without evidence must fail");
        assert!(unsupported.to_string().contains("verification evidence"));

        manager.update_node(
            "task",
            NodePatch {
                status: Some(TreeStatus::Completed),
                evidence: Some(vec![VerificationEvidence {
                    kind: "test".to_string(),
                    summary: "Tree unit tests passed".to_string(),
                    command: Some("cargo test tree".to_string()),
                    tool: Some("verify_command".to_string()),
                    working_directory: Some(".".to_string()),
                    success: true,
                    recorded_by: TreeActor::System,
                    recorded_at: String::new(),
                }]),
                ..NodePatch::default()
            },
            TreeActor::Ai,
            1,
        )?;
        assert_eq!(
            manager.get("task").map(|item| item.status),
            Some(TreeStatus::Completed)
        );
        assert_eq!(
            manager
                .get("task")
                .and_then(|item| item.evidence.first())
                .map(|item| item.recorded_by),
            Some(TreeActor::Ai)
        );
        Ok(())
    }

    #[test]
    fn prevents_cycles_and_cascades_deletion() -> Result<()> {
        let mut manager = TreeManager::empty();
        manager.add_node(node("root", None, "Project"), TreeActor::Ai, 0)?;
        manager.add_node(node("child", Some("root"), "Feature"), TreeActor::Ai, 1)?;
        manager.add_node(node("leaf", Some("child"), "Test"), TreeActor::Ai, 2)?;

        let cycle_error = manager
            .update_node(
                "root",
                NodePatch {
                    parent_id: Some(Some("leaf".to_string())),
                    ..NodePatch::default()
                },
                TreeActor::Ai,
                3,
            )
            .expect_err("cycle must be rejected");
        assert!(cycle_error.to_string().contains("cycle"));

        let removed = manager.delete_node("child", TreeActor::Ai, 3)?;
        assert_eq!(removed, vec!["child".to_string(), "leaf".to_string()]);
        assert!(manager.get("root").is_some());
        assert!(manager.get("child").is_none());
        Ok(())
    }

    #[test]
    fn review_context_preserves_human_corrections_and_files() -> Result<()> {
        let mut manager = TreeManager::empty();
        let mut input = node("test", None, "Run integration tests");
        input.target_files = vec![
            "src\\main.rs".to_string(),
            "SRC/main.rs".to_string(),
            "tests/tree.rs".to_string(),
        ];
        manager.add_node(input, TreeActor::Ai, 0)?;
        manager.update_node(
            "test",
            NodePatch {
                status: Some(TreeStatus::Failed),
                human_note: Some(Some("Observed crash on Linux".to_string())),
                ..NodePatch::default()
            },
            TreeActor::Human,
            1,
        )?;

        let context = manager.review_context("test")?;
        assert!(context.contains("HUMAN NOTE: Observed crash on Linux"));
        assert!(context.contains("src/main.rs"));
        assert!(context.contains("tests/tree.rs"));
        #[cfg(not(windows))]
        assert!(context.contains("SRC/main.rs"));
        assert!(context.contains("owner=human"));
        Ok(())
    }

    #[test]
    fn file_edits_require_an_active_associated_node() -> Result<()> {
        let mut manager = TreeManager::empty();
        let mut input = node("code", None, "Implement tree");
        input.target_files = vec!["src/tree.rs".to_string()];
        manager.add_node(input, TreeActor::Ai, 0)?;

        assert!(manager.authorize_file_edit("src/tree.rs").is_err());
        manager.update_node(
            "code",
            NodePatch {
                status: Some(TreeStatus::InProgress),
                ..NodePatch::default()
            },
            TreeActor::Ai,
            1,
        )?;
        assert_eq!(manager.authorize_file_edit("src\\tree.rs")?.id, "code");
        assert!(manager.authorize_file_edit("src/other.rs").is_err());
        Ok(())
    }

    #[test]
    fn serializes_round_trip_and_accepts_legacy_nodes() -> Result<()> {
        let legacy = r#"{
            "version": 1,
            "nodes": [{
                "id": "legacy",
                "parent_id": null,
                "title": "Legacy task",
                "kind": "task",
                "status": "pending",
                "priority": 50,
                "owner": "ai",
                "human_note": null,
                "target_files": []
            }]
        }"#;
        let tree: ProjectTree = serde_json::from_str(legacy)?;
        let manager = TreeManager::new(tree)?;
        let encoded = manager.to_json()?;
        let decoded: ProjectTree = serde_json::from_str(&encoded)?;

        assert_eq!(decoded.nodes.len(), 1);
        assert_eq!(decoded.nodes[0].created_by, TreeActor::System);
        Ok(())
    }

    #[test]
    fn large_tree_prompt_keeps_active_and_human_nodes_compact() -> Result<()> {
        let mut manager = TreeManager::empty();
        manager.add_node(node("root", None, "Project"), TreeActor::Ai, 0)?;
        for index in 0..81 {
            manager.add_node(
                node(
                    &format!("task-{index}"),
                    Some("root"),
                    &format!("Task {index}"),
                ),
                TreeActor::Ai,
                manager.version(),
            )?;
        }
        manager.update_node(
            "task-80",
            NodePatch {
                status: Some(TreeStatus::InProgress),
                ..NodePatch::default()
            },
            TreeActor::Ai,
            manager.version(),
        )?;

        let summary = manager.prompt_summary();
        assert!(summary.contains("Large tree"));
        assert!(summary.contains("task-80"));
        assert!(!summary.contains("task-40"));
        Ok(())
    }
}
