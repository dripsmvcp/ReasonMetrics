//! Fixed, version-pinned benchmark task sets, embedded at compile time.

use serde::Deserialize;
use sha2::{Digest, Sha256};

#[derive(Debug, Clone, Deserialize)]
pub struct Task {
    pub id: String,
    #[serde(alias = "question", alias = "prompt")]
    pub problem: String,
    #[serde(alias = "ground_truth", alias = "label", alias = "target")]
    pub expected_answer: String,
}

#[derive(Debug, Clone)]
pub struct TaskSet {
    pub name: String,
    pub sha256: String,
    pub tasks: Vec<Task>,
}

/// Raw bytes of a bundled set by name. Add new sets here.
fn bundled(name: &str) -> Option<&'static str> {
    match name {
        "overthinking-v1" => Some(include_str!("../../benchsets/overthinking-v1.jsonl")),
        _ => None,
    }
}

pub fn load(name: &str) -> anyhow::Result<TaskSet> {
    let raw = bundled(name)
        .ok_or_else(|| anyhow::anyhow!("unknown task set `{name}` (bundled: overthinking-v1)"))?;

    let sha256 = format!("{:x}", Sha256::digest(raw.as_bytes()));

    let mut tasks = Vec::new();
    for (i, line) in raw.lines().enumerate() {
        let line = line.trim();
        if line.is_empty() {
            continue;
        }
        let task: Task = serde_json::from_str(line)
            .map_err(|e| anyhow::anyhow!("task set `{name}` line {}: {e}", i + 1))?;
        tasks.push(task);
    }
    if tasks.is_empty() {
        anyhow::bail!("task set `{name}` is empty");
    }

    Ok(TaskSet {
        name: name.to_string(),
        sha256,
        tasks,
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn loads_bundled_overthinking_v1() {
        let ts = load("overthinking-v1").unwrap();
        assert_eq!(ts.name, "overthinking-v1");
        assert_eq!(ts.tasks.len(), 12);
        assert_eq!(ts.tasks[0].id, "ot-001");
        assert_eq!(ts.tasks[0].expected_answer, "43");
        // sha256 is 64 lowercase hex chars and stable across calls.
        assert_eq!(ts.sha256.len(), 64);
        assert_eq!(ts.sha256, load("overthinking-v1").unwrap().sha256);
    }

    #[test]
    fn unknown_task_set_errors() {
        assert!(load("does-not-exist").is_err());
    }
}
