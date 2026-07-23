use anyhow::{bail, Context, Result};
use chrono::Utc;
use serde::{Deserialize, Serialize};
use std::cmp::Ordering;
use std::fs::{self, OpenOptions};
use std::io::Write;
use std::path::{Path, PathBuf};
use std::sync::Mutex;

const EMBEDDING_DIMS: usize = 256;
const MAX_SOURCE_BYTES: u64 = 512 * 1024;
const MAX_KNOWLEDGE_FILES: usize = 200;
const CHUNK_CHARS: usize = 1_600;
const CHUNK_OVERLAP: usize = 200;
const MAX_MEMORY_CHARS: usize = 8_000;
static MEMORY_WRITE_LOCK: Mutex<()> = Mutex::new(());

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct KnowledgeEntry {
    pub id: String,
    pub title: String,
    pub content: String,
    pub source: String,
    #[serde(default)]
    pub tags: Vec<String>,
    pub created_at: String,
    pub created_by: String,
}

#[derive(Debug, Clone)]
pub struct KnowledgeHit {
    pub entry: KnowledgeEntry,
    pub score: f32,
}

#[derive(Debug, Clone)]
struct IndexedEntry {
    entry: KnowledgeEntry,
    embedding: Vec<f32>,
}

#[derive(Debug, Clone)]
pub struct KnowledgeBase {
    workspace: PathBuf,
    entries: Vec<IndexedEntry>,
}

impl KnowledgeBase {
    pub fn empty(workspace: PathBuf) -> Self {
        Self {
            workspace,
            entries: Vec::new(),
        }
    }

    pub fn load(workspace: &Path) -> Result<Self> {
        let workspace = workspace
            .canonicalize()
            .with_context(|| format!("invalid workspace: {}", workspace.display()))?;
        let mut base = Self {
            workspace,
            entries: Vec::new(),
        };
        base.load_markdown_knowledge()?;
        base.load_agent_memory()?;
        Ok(base)
    }

    pub fn search(&self, query: &str, limit: usize) -> Vec<KnowledgeHit> {
        if query.trim().is_empty() || limit == 0 {
            return Vec::new();
        }
        let query_embedding = embed(query);
        let mut hits: Vec<KnowledgeHit> = self
            .entries
            .iter()
            .filter_map(|indexed| {
                let score = dot(&query_embedding, &indexed.embedding);
                (score >= 0.08).then(|| KnowledgeHit {
                    entry: indexed.entry.clone(),
                    score,
                })
            })
            .collect();
        hits.sort_by(|left, right| {
            right
                .score
                .partial_cmp(&left.score)
                .unwrap_or(Ordering::Equal)
                .then_with(|| left.entry.id.cmp(&right.entry.id))
        });
        hits.truncate(limit);
        hits
    }

    pub fn prompt_guidance(&self, query: &str, limit: usize) -> String {
        let hits = self.search(query, limit);
        if hits.is_empty() {
            return String::new();
        }
        let mut output = String::from(
            "Retrieved project-specific engineering knowledge. Treat it as guidance, \
             not as proof that current code is correct.\n",
        );
        for hit in hits {
            output.push_str(&format!(
                "\n### {} (source: {}, relevance: {:.3})\n{}\n",
                hit.entry.title, hit.entry.source, hit.score, hit.entry.content
            ));
        }
        output
    }

    pub fn remember(
        &mut self,
        title: &str,
        content: &str,
        tags: Vec<String>,
        created_by: &str,
    ) -> Result<String> {
        let title = title.trim();
        let content = content.trim();
        if title.is_empty() || content.is_empty() {
            bail!("knowledge title and content are required");
        }
        if content.chars().count() > MAX_MEMORY_CHARS {
            bail!("knowledge content exceeds {MAX_MEMORY_CHARS} characters");
        }
        if self
            .entries
            .iter()
            .any(|entry| entry.entry.title == title && entry.entry.content == content)
        {
            bail!("identical knowledge already exists");
        }

        let id = uuid::Uuid::new_v4().to_string();
        let entry = KnowledgeEntry {
            id: id.clone(),
            title: title.to_string(),
            content: content.to_string(),
            source: "agent-memory".to_string(),
            tags: normalize_tags(tags),
            created_at: Utc::now().to_rfc3339(),
            created_by: created_by.to_string(),
        };
        let memory_path = self.memory_path();
        if let Some(parent) = memory_path.parent() {
            fs::create_dir_all(parent)?;
        }
        let _guard = MEMORY_WRITE_LOCK
            .lock()
            .map_err(|_| anyhow::anyhow!("knowledge memory write lock poisoned"))?;
        let mut line = serde_json::to_vec(&entry)?;
        line.push(b'\n');
        let mut file = OpenOptions::new()
            .create(true)
            .append(true)
            .open(&memory_path)?;
        file.write_all(&line)?;
        file.flush()?;
        self.push(entry);
        Ok(id)
    }

    pub fn len(&self) -> usize {
        self.entries.len()
    }

    fn load_markdown_knowledge(&mut self) -> Result<()> {
        let root = self.workspace.join(".mooncoding").join("knowledge");
        if !root.is_dir() {
            return Ok(());
        }
        let mut loaded = 0usize;
        for item in walkdir::WalkDir::new(&root)
            .max_depth(5)
            .follow_links(false)
            .into_iter()
        {
            if loaded >= MAX_KNOWLEDGE_FILES {
                break;
            }
            let entry = match item {
                Ok(entry) => entry,
                Err(_) => continue,
            };
            if !entry.file_type().is_file() || entry.path() == self.memory_path() {
                continue;
            }
            let extension = entry
                .path()
                .extension()
                .and_then(|value| value.to_str())
                .map(str::to_ascii_lowercase);
            if !matches!(extension.as_deref(), Some("md" | "txt")) {
                continue;
            }
            let metadata = match entry.metadata() {
                Ok(metadata) => metadata,
                Err(_) => continue,
            };
            if metadata.len() > MAX_SOURCE_BYTES {
                continue;
            }
            let content = match fs::read_to_string(entry.path()) {
                Ok(content) => content,
                Err(_) => continue,
            };
            let source = entry
                .path()
                .strip_prefix(&self.workspace)
                .unwrap_or(entry.path())
                .to_string_lossy()
                .replace('\\', "/");
            for (index, chunk) in chunk_text(&content).into_iter().enumerate() {
                let title = first_heading(&chunk)
                    .unwrap_or_else(|| format!("{} · chunk {}", source, index + 1));
                self.push(KnowledgeEntry {
                    id: format!("file:{source}:{index}"),
                    title,
                    content: chunk,
                    source: source.clone(),
                    tags: Vec::new(),
                    created_at: String::new(),
                    created_by: "human".to_string(),
                });
            }
            loaded += 1;
        }
        Ok(())
    }

    fn load_agent_memory(&mut self) -> Result<()> {
        let path = self.memory_path();
        if !path.is_file() {
            return Ok(());
        }
        let _guard = MEMORY_WRITE_LOCK
            .lock()
            .map_err(|_| anyhow::anyhow!("knowledge memory write lock poisoned"))?;
        let content = fs::read_to_string(&path)?;
        let lines: Vec<&str> = content.lines().collect();
        for (line_number, line) in lines.iter().copied().enumerate() {
            if line.trim().is_empty() {
                continue;
            }
            let entry: KnowledgeEntry = match serde_json::from_str(line) {
                Ok(entry) => entry,
                Err(_) if line_number + 1 == lines.len() && !content.ends_with('\n') => {
                    let quarantine = path.with_extension(format!(
                        "jsonl.corrupt-{}",
                        Utc::now().format("%Y%m%dT%H%M%S%fZ")
                    ));
                    fs::write(&quarantine, line).with_context(|| {
                        format!(
                            "quarantine malformed trailing knowledge record to {}",
                            quarantine.display()
                        )
                    })?;
                    break;
                }
                Err(error) => {
                    return Err(error).with_context(|| {
                        format!(
                            "invalid knowledge memory at {}:{}",
                            path.display(),
                            line_number + 1
                        )
                    });
                }
            };
            self.push(entry);
        }
        Ok(())
    }

    fn push(&mut self, entry: KnowledgeEntry) {
        let embedding = embed(&format!(
            "{}\n{}\n{}",
            entry.title,
            entry.tags.join(" "),
            entry.content
        ));
        self.entries.push(IndexedEntry { entry, embedding });
    }

    fn memory_path(&self) -> PathBuf {
        self.workspace
            .join(".mooncoding")
            .join("knowledge")
            .join("agent-memory.jsonl")
    }
}

fn chunk_text(content: &str) -> Vec<String> {
    let characters: Vec<char> = content.chars().collect();
    if characters.is_empty() {
        return Vec::new();
    }
    let mut chunks = Vec::new();
    let mut start = 0usize;
    while start < characters.len() {
        let end = (start + CHUNK_CHARS).min(characters.len());
        let chunk: String = characters[start..end].iter().collect();
        if !chunk.trim().is_empty() {
            chunks.push(chunk);
        }
        if end == characters.len() {
            break;
        }
        start = end.saturating_sub(CHUNK_OVERLAP);
    }
    chunks
}

fn first_heading(content: &str) -> Option<String> {
    content
        .lines()
        .find_map(|line| line.trim().strip_prefix('#').map(str::trim))
        .filter(|heading| !heading.is_empty())
        .map(str::to_string)
}

fn normalize_tags(tags: Vec<String>) -> Vec<String> {
    let mut tags: Vec<String> = tags
        .into_iter()
        .map(|tag| tag.trim().to_ascii_lowercase())
        .filter(|tag| !tag.is_empty())
        .collect();
    tags.sort();
    tags.dedup();
    tags
}

fn embed(text: &str) -> Vec<f32> {
    let normalized = text.to_lowercase();
    let mut vector = vec![0.0f32; EMBEDDING_DIMS];
    for token in normalized
        .split(|character: char| !character.is_alphanumeric() && character != '_')
        .filter(|token| token.len() >= 2)
    {
        add_feature(&mut vector, token.as_bytes(), 1.0);
    }
    let characters: Vec<char> = normalized.chars().collect();
    for gram in characters.windows(3) {
        let value: String = gram.iter().collect();
        add_feature(&mut vector, value.as_bytes(), 0.35);
    }
    let norm = vector.iter().map(|value| value * value).sum::<f32>().sqrt();
    if norm > 0.0 {
        for value in &mut vector {
            *value /= norm;
        }
    }
    vector
}

fn add_feature(vector: &mut [f32], feature: &[u8], weight: f32) {
    let hash = fnv1a(feature);
    let index = (hash as usize) % vector.len();
    let sign = if hash & (1 << 63) == 0 { 1.0 } else { -1.0 };
    vector[index] += weight * sign;
}

fn fnv1a(value: &[u8]) -> u64 {
    let mut hash = 0xcbf29ce484222325u64;
    for byte in value {
        hash ^= u64::from(*byte);
        hash = hash.wrapping_mul(0x100000001b3);
    }
    hash
}

fn dot(left: &[f32], right: &[f32]) -> f32 {
    left.iter()
        .zip(right)
        .map(|(left, right)| left * right)
        .sum()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn retrieval_prefers_semantically_related_engineering_knowledge() {
        let mut base = KnowledgeBase {
            workspace: PathBuf::from("."),
            entries: Vec::new(),
        };
        base.push(KnowledgeEntry {
            id: "rust".to_string(),
            title: "Rust cancellation".to_string(),
            content: "Use tokio select and cancellation tokens for asynchronous tasks.".to_string(),
            source: "test".to_string(),
            tags: vec!["rust".to_string()],
            created_at: String::new(),
            created_by: "human".to_string(),
        });
        base.push(KnowledgeEntry {
            id: "css".to_string(),
            title: "CSS colors".to_string(),
            content: "Use balanced foreground and background color palettes.".to_string(),
            source: "test".to_string(),
            tags: vec!["design".to_string()],
            created_at: String::new(),
            created_by: "human".to_string(),
        });

        let hits = base.search("cancel an async tokio Rust task", 1);
        assert_eq!(hits.len(), 1);
        assert_eq!(hits[0].entry.id, "rust");
    }

    #[test]
    fn chunking_is_bounded_and_overlapping() {
        let content = "a".repeat(CHUNK_CHARS + 50);
        let chunks = chunk_text(&content);
        assert_eq!(chunks.len(), 2);
        assert_eq!(chunks[0].chars().count(), CHUNK_CHARS);
        assert!(chunks[1].chars().count() > 50);
    }

    #[test]
    fn remembered_knowledge_persists_and_reloads() -> Result<()> {
        let workspace =
            std::env::temp_dir().join(format!("mooncoding-knowledge-{}", uuid::Uuid::new_v4()));
        fs::create_dir_all(&workspace)?;
        let mut base = KnowledgeBase::load(&workspace)?;
        base.remember(
            "Qt callback threading",
            "Queue Rust callback events onto the Qt object thread.",
            vec!["qt".to_string(), "ffi".to_string()],
            "ai",
        )?;
        let reloaded = KnowledgeBase::load(&workspace)?;
        let hits = reloaded.search("Qt Rust FFI callback thread", 3);
        assert!(hits
            .iter()
            .any(|hit| hit.entry.title == "Qt callback threading"));
        fs::remove_dir_all(workspace)?;
        Ok(())
    }
}
