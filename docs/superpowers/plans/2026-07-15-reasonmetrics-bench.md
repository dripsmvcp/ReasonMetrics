# `reasonmetrics bench` Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Add a feature-gated `reasonmetrics bench` subcommand that runs a fixed, content-hashed task set against any OpenAI-compatible endpoint, scores each returned trace with the existing engine, and writes a commit-friendly result JSON plus a leaderboard.

**Architecture:** Six units under a new `crates/reasonmetrics-cli/src/bench/` module: task-set loader, `Model` trait + `ureq` HTTP impl + mock, runner (rayon), scorer+correctness adapter (reuses `reasonmetrics-core`), aggregator, and result writer. All new dependencies (`ureq`, `sha2`) sit behind an opt-in `bench` cargo feature so the default curation binary stays lean.

**Tech Stack:** Rust, clap, rayon (existing), ureq + sha2 (new, feature-gated), serde_json, reasonmetrics-core.

## Global Constraints

- Rust edition 2021, `rust-version` 1.80 (workspace floor) — copied from `Cargo.toml`.
- New deps `ureq` and `sha2` are **feature-gated** under `bench`; a default `cargo build` MUST compile with neither and without the subcommand.
- No AI-attribution trailers in any commit (`Co-Authored-By`/"Generated with" are forbidden).
- API keys are read only from an env var named by `--api-key-env`; never a flag value, never written to any output.
- `endpoint_host` in output stores host only — never a full URL.
- Reuse `reasonmetrics-core` for scoring and answer-matching; do not duplicate scoring logic.
- Every task ends green (`cargo test --features bench` for bench code; `cargo build` with default features stays green throughout).

---

### Task 1: Expose `answers_match` for reuse

**Files:**
- Modify: `crates/reasonmetrics-core/src/scorers/accuracy_efficiency.rs:28-42`
- Test: same file (`#[cfg(test)] mod tests`)

**Interfaces:**
- Consumes: nothing.
- Produces: `reasonmetrics_core::scorers::accuracy_efficiency::answers_match(answer: &str, expected: &str) -> bool` and `normalize_answer(s: &str) -> String`, both `pub`.

- [ ] **Step 1: Write the failing test**

Add to the `tests` module in `accuracy_efficiency.rs`:

```rust
#[test]
fn answers_match_is_public_and_normalizes() {
    // Reachable as a public item, and applies numeric + punctuation/casing rules.
    assert!(super::answers_match("43.", "43"));
    assert!(super::answers_match("4.0", "4"));
    assert!(!super::answers_match("44", "43"));
    assert_eq!(super::normalize_answer(" X = 2. "), "x = 2");
}
```

- [ ] **Step 2: Run test to verify it fails**

Run: `cargo test -p reasonmetrics-core answers_match_is_public_and_normalizes`
Expected: FAIL to compile — `answers_match`/`normalize_answer` are private (E0603).

- [ ] **Step 3: Make the two functions public**

Change the two signatures (leave bodies unchanged):

```rust
pub fn normalize_answer(s: &str) -> String {
    s.trim().trim_end_matches(['.', ',']).trim().to_lowercase()
}

pub fn answers_match(answer: &str, expected: &str) -> bool {
    let a = normalize_answer(answer);
    let b = normalize_answer(expected);
    if a == b {
        return true;
    }
    if let (Ok(x), Ok(y)) = (a.parse::<f64>(), b.parse::<f64>()) {
        return (x - y).abs() < 1e-9;
    }
    false
}
```

- [ ] **Step 4: Run tests to verify they pass**

Run: `cargo test -p reasonmetrics-core`
Expected: PASS (new test + all existing).

- [ ] **Step 5: Commit**

```bash
git add crates/reasonmetrics-core/src/scorers/accuracy_efficiency.rs
git commit -m "refactor(core): make answers_match/normalize_answer public for reuse"
```

---

### Task 2: Bench feature + module scaffold + subcommand wiring

**Files:**
- Modify: `crates/reasonmetrics-cli/Cargo.toml`
- Create: `crates/reasonmetrics-cli/src/bench/mod.rs`
- Modify: `crates/reasonmetrics-cli/src/main.rs` (mod decl, `Commands` variant, match arm)

**Interfaces:**
- Consumes: `reasonmetrics_core::config::ScoringConfig`.
- Produces: `bench::BenchArgs` (public fields below), `bench::LeaderboardFormat`, and `bench::run(args: BenchArgs, scoring: &ScoringConfig) -> anyhow::Result<()>` (a stub in this task).

- [ ] **Step 1: Add the feature and gated deps to `Cargo.toml`**

Add a `[features]` section and the two optional deps:

```toml
[features]
default = []
bench = ["dep:ureq", "dep:sha2"]

[dependencies]
# ... existing deps unchanged ...
ureq = { version = "2", features = ["json"], optional = true }
sha2 = { version = "0.10", optional = true }
```

- [ ] **Step 2: Create the module with the arg types and a stub `run`**

Create `crates/reasonmetrics-cli/src/bench/mod.rs`:

```rust
//! `reasonmetrics bench` — run a fixed task set against an OpenAI-compatible
//! endpoint and score the returned traces. Feature-gated (`bench`).

use std::path::PathBuf;
use std::str::FromStr;

use reasonmetrics_core::config::ScoringConfig;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum LeaderboardFormat {
    Table,
    Md,
    Html,
    Json,
}

impl FromStr for LeaderboardFormat {
    type Err = String;
    fn from_str(s: &str) -> Result<Self, Self::Err> {
        match s.to_ascii_lowercase().as_str() {
            "table" => Ok(Self::Table),
            "md" | "markdown" => Ok(Self::Md),
            "html" => Ok(Self::Html),
            "json" => Ok(Self::Json),
            other => Err(format!("unknown format `{other}` (use table|md|html|json)")),
        }
    }
}

#[derive(Debug, Clone)]
pub struct BenchArgs {
    pub endpoint: String,
    pub model: String,
    pub task_set: String,
    pub temperature: f32,
    pub max_tokens: usize,
    pub concurrency: usize,
    pub cost_per_mtok: Option<f32>,
    pub api_key_env: Option<String>,
    pub out: Option<PathBuf>,
    pub format: LeaderboardFormat,
    pub retries: usize,
}

pub fn run(_args: BenchArgs, _scoring: &ScoringConfig) -> anyhow::Result<()> {
    anyhow::bail!("reasonmetrics bench: not yet implemented")
}
```

- [ ] **Step 3: Wire the module and subcommand in `main.rs`**

After the existing `mod pipeline;` lines near the top add:

```rust
#[cfg(feature = "bench")]
mod bench;
```

Add a gated variant to `enum Commands` (after the `Models` variant):

```rust
    /// Benchmark a model's reasoning over a fixed task set (feature: bench)
    #[cfg(feature = "bench")]
    Bench {
        #[arg(long)]
        endpoint: String,
        #[arg(long)]
        model: String,
        #[arg(long, default_value = "overthinking-v1")]
        task_set: String,
        #[arg(long, default_value_t = 0.0)]
        temperature: f32,
        #[arg(long, default_value_t = 8192)]
        max_tokens: usize,
        #[arg(long, default_value_t = 8)]
        concurrency: usize,
        #[arg(long)]
        cost_per_mtok: Option<f32>,
        #[arg(long)]
        api_key_env: Option<String>,
        #[arg(long)]
        out: Option<std::path::PathBuf>,
        #[arg(long, default_value = "table")]
        format: String,
        #[arg(long, default_value_t = 2)]
        retries: usize,
    },
```

Add a gated match arm in `main()` (after the `Commands::Models => cmd_models(),` arm):

```rust
        #[cfg(feature = "bench")]
        Commands::Bench {
            endpoint,
            model,
            task_set,
            temperature,
            max_tokens,
            concurrency,
            cost_per_mtok,
            api_key_env,
            out,
            format,
            retries,
        } => {
            let config = Config::load(&cli.config)?;
            let format = format
                .parse::<bench::LeaderboardFormat>()
                .map_err(|e| anyhow::anyhow!(e))?;
            let args = bench::BenchArgs {
                endpoint,
                model,
                task_set,
                temperature,
                max_tokens,
                concurrency,
                cost_per_mtok,
                api_key_env,
                out,
                format,
                retries,
            };
            bench::run(args, &config.scoring)?
        }
```

- [ ] **Step 4: Verify both feature configurations compile**

Run: `cargo build -p reasonmetrics-cli`
Expected: PASS, no `ureq`/`sha2` compiled (default features).

Run: `cargo build -p reasonmetrics-cli --features bench`
Expected: PASS.

Run: `cargo run -p reasonmetrics-cli --features bench -- bench --help`
Expected: usage text listing `--endpoint`, `--model`, `--task-set`, etc.

- [ ] **Step 5: Commit**

```bash
git add crates/reasonmetrics-cli/Cargo.toml crates/reasonmetrics-cli/src/bench/mod.rs crates/reasonmetrics-cli/src/main.rs
git commit -m "feat(cli): scaffold feature-gated bench subcommand"
```

---

### Task 3: Bundled task set + loader

**Files:**
- Create: `crates/reasonmetrics-cli/benchsets/overthinking-v1.jsonl`
- Create: `crates/reasonmetrics-cli/src/bench/taskset.rs`
- Modify: `crates/reasonmetrics-cli/src/bench/mod.rs` (add `pub mod taskset;`)

**Interfaces:**
- Consumes: nothing.
- Produces: `taskset::Task { id: String, problem: String, expected_answer: String }`, `taskset::TaskSet { name: String, sha256: String, tasks: Vec<Task> }`, and `taskset::load(name: &str) -> anyhow::Result<TaskSet>`.

- [ ] **Step 1: Create the bundled task set (original, hand-authored, no license encumbrance)**

Create `crates/reasonmetrics-cli/benchsets/overthinking-v1.jsonl` with exactly these lines:

```jsonl
{"id":"ot-001","problem":"What is 17 + 26?","expected_answer":"43"}
{"id":"ot-002","problem":"What is 8 multiplied by 7?","expected_answer":"56"}
{"id":"ot-003","problem":"What is 144 divided by 12?","expected_answer":"12"}
{"id":"ot-004","problem":"What is 100 minus 37?","expected_answer":"63"}
{"id":"ot-005","problem":"What is the sum of the first 5 positive integers?","expected_answer":"15"}
{"id":"ot-006","problem":"A train travels 60 km in 1.5 hours. What is its average speed in km/h?","expected_answer":"40"}
{"id":"ot-007","problem":"What is 15% of 200?","expected_answer":"30"}
{"id":"ot-008","problem":"What is the smallest prime number greater than 13?","expected_answer":"17"}
{"id":"ot-009","problem":"What is the area of a rectangle 6 units wide and 9 units tall?","expected_answer":"54"}
{"id":"ot-010","problem":"What is 2 to the power of 10?","expected_answer":"1024"}
{"id":"ot-011","problem":"How many minutes are there in 3.5 hours?","expected_answer":"210"}
{"id":"ot-012","problem":"What is 3/4 plus 1/4?","expected_answer":"1"}
```

- [ ] **Step 2: Write the failing test**

Create `crates/reasonmetrics-cli/src/bench/taskset.rs`:

```rust
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
```

Register the module — add to `crates/reasonmetrics-cli/src/bench/mod.rs` (top, after the doc comment):

```rust
pub mod taskset;
```

- [ ] **Step 3: Run test to verify it fails**

Run: `cargo test -p reasonmetrics-cli --features bench taskset`
Expected: FAIL to compile — `load`, `Task`, `TaskSet` undefined.

- [ ] **Step 4: Implement the loader**

Prepend to `crates/reasonmetrics-cli/src/bench/taskset.rs` (above the `tests` module):

```rust
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
```

- [ ] **Step 5: Run tests to verify they pass**

Run: `cargo test -p reasonmetrics-cli --features bench taskset`
Expected: PASS.

- [ ] **Step 6: Commit**

```bash
git add crates/reasonmetrics-cli/benchsets/overthinking-v1.jsonl crates/reasonmetrics-cli/src/bench/taskset.rs crates/reasonmetrics-cli/src/bench/mod.rs
git commit -m "feat(bench): bundled overthinking-v1 task set + loader"
```

---

### Task 4: `Model` trait, `Completion`, and `MockModel`

**Files:**
- Create: `crates/reasonmetrics-cli/src/bench/model.rs`
- Modify: `crates/reasonmetrics-cli/src/bench/mod.rs` (add `pub mod model;`)

**Interfaces:**
- Consumes: nothing.
- Produces: `model::Completion { text: String, completion_tokens: Option<usize> }` (derives `Clone`), `model::Model` trait with `fn complete(&self, prompt: &str) -> anyhow::Result<Completion>` (bound `: Sync`), and `model::MockModel` mapping prompt → `Completion`.

- [ ] **Step 1: Write the failing test**

Create `crates/reasonmetrics-cli/src/bench/model.rs`:

```rust
#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn mock_returns_canned_completion() {
        let mock = MockModel::new(vec![(
            "What is 2+2?".to_string(),
            Completion { text: "<think>2+2=4</think> 4".into(), completion_tokens: Some(5) },
        )]);
        let c = mock.complete("What is 2+2?").unwrap();
        assert_eq!(c.completion_tokens, Some(5));
        assert!(c.text.contains("4"));
    }

    #[test]
    fn mock_errors_on_unknown_prompt() {
        let mock = MockModel::new(vec![]);
        assert!(mock.complete("unknown").is_err());
    }
}
```

Register the module — add to `crates/reasonmetrics-cli/src/bench/mod.rs`:

```rust
pub mod model;
```

- [ ] **Step 2: Run test to verify it fails**

Run: `cargo test -p reasonmetrics-cli --features bench model`
Expected: FAIL to compile — `Completion`, `Model`, `MockModel` undefined.

- [ ] **Step 3: Implement the trait, `Completion`, and `MockModel`**

Prepend to `crates/reasonmetrics-cli/src/bench/model.rs`:

```rust
//! The model backend abstraction: a `Model` yields a `Completion` for a prompt.

use std::collections::HashMap;

#[derive(Debug, Clone)]
pub struct Completion {
    pub text: String,
    /// Completion-token count from the endpoint's `usage`, when provided.
    pub completion_tokens: Option<usize>,
}

pub trait Model: Sync {
    fn complete(&self, prompt: &str) -> anyhow::Result<Completion>;
}

/// Test/offline model: returns a canned completion keyed by exact prompt.
pub struct MockModel {
    responses: HashMap<String, Completion>,
}

impl MockModel {
    pub fn new(pairs: Vec<(String, Completion)>) -> Self {
        Self {
            responses: pairs.into_iter().collect(),
        }
    }
}

impl Model for MockModel {
    fn complete(&self, prompt: &str) -> anyhow::Result<Completion> {
        self.responses
            .get(prompt)
            .cloned()
            .ok_or_else(|| anyhow::anyhow!("MockModel: no canned response for prompt {prompt:?}"))
    }
}
```

- [ ] **Step 4: Run tests to verify they pass**

Run: `cargo test -p reasonmetrics-cli --features bench model`
Expected: PASS.

- [ ] **Step 5: Commit**

```bash
git add crates/reasonmetrics-cli/src/bench/model.rs crates/reasonmetrics-cli/src/bench/mod.rs
git commit -m "feat(bench): Model trait, Completion, and MockModel"
```

---

### Task 5: `HttpModel` — OpenAI-compatible endpoint client

**Files:**
- Modify: `crates/reasonmetrics-cli/src/bench/model.rs`
- Modify: `crates/reasonmetrics-cli/Cargo.toml` (`[dev-dependencies]`: `tiny_http`)

**Interfaces:**
- Consumes: `Model`, `Completion` (Task 4).
- Produces: `model::HttpModel::new(endpoint: &str, model: &str, api_key: Option<String>, temperature: f32, max_tokens: usize) -> HttpModel` implementing `Model`; `model::host_of(url: &str) -> String`.

- [ ] **Step 1: Write the failing test (against an in-process stub endpoint)**

Add `tiny_http` to `crates/reasonmetrics-cli/Cargo.toml` under `[dev-dependencies]`:

```toml
tiny_http = "0.12"
```

Add to the `tests` module in `model.rs`:

```rust
#[test]
fn host_of_extracts_host_only() {
    assert_eq!(host_of("http://localhost:8000/v1"), "localhost:8000");
    assert_eq!(host_of("https://api.example.com/v1/"), "api.example.com");
    assert_eq!(host_of("localhost:11434"), "localhost:11434");
}

#[test]
fn http_model_parses_openai_response() {
    // Stub endpoint returns one OpenAI-shaped completion.
    let server = tiny_http::Server::http("127.0.0.1:0").unwrap();
    let addr = server.server_addr().to_ip().unwrap();
    let base = format!("http://{}:{}/v1", addr.ip(), addr.port());

    let handle = std::thread::spawn(move || {
        let req = server.recv().unwrap();
        let body = r#"{"choices":[{"message":{"content":"<think>1+1=2</think> 2"}}],
                       "usage":{"completion_tokens":7}}"#;
        let resp = tiny_http::Response::from_string(body)
            .with_header("Content-Type: application/json".parse::<tiny_http::Header>().unwrap());
        req.respond(resp).unwrap();
    });

    let m = HttpModel::new(&base, "stub-model", None, 0.0, 128);
    let c = m.complete("What is 1+1?").unwrap();
    handle.join().unwrap();

    assert_eq!(c.text, "<think>1+1=2</think> 2");
    assert_eq!(c.completion_tokens, Some(7));
}
```

- [ ] **Step 2: Run test to verify it fails**

Run: `cargo test -p reasonmetrics-cli --features bench http_model_parses_openai_response`
Expected: FAIL to compile — `HttpModel`, `host_of` undefined.

- [ ] **Step 3: Implement `HttpModel` and `host_of`**

Add to the top of `model.rs` (after the existing `use` line):

```rust
use std::time::Duration;
```

Append to `model.rs` (above the `tests` module):

```rust
/// Host[:port] of a URL, dropping scheme, path, and any credentials.
pub fn host_of(url: &str) -> String {
    let after_scheme = url.split_once("://").map(|(_, rest)| rest).unwrap_or(url);
    let host_port = after_scheme.split(['/', '?', '#']).next().unwrap_or(after_scheme);
    // Drop any userinfo (user:pass@host) defensively.
    host_port.rsplit('@').next().unwrap_or(host_port).to_string()
}

pub struct HttpModel {
    agent: ureq::Agent,
    url: String,
    model: String,
    api_key: Option<String>,
    temperature: f32,
    max_tokens: usize,
}

impl HttpModel {
    pub fn new(
        endpoint: &str,
        model: &str,
        api_key: Option<String>,
        temperature: f32,
        max_tokens: usize,
    ) -> Self {
        let agent = ureq::AgentBuilder::new()
            .timeout(Duration::from_secs(120))
            .build();
        let url = format!("{}/chat/completions", endpoint.trim_end_matches('/'));
        Self {
            agent,
            url,
            model: model.to_string(),
            api_key,
            temperature,
            max_tokens,
        }
    }
}

impl Model for HttpModel {
    fn complete(&self, prompt: &str) -> anyhow::Result<Completion> {
        let body = serde_json::json!({
            "model": self.model,
            "messages": [{ "role": "user", "content": prompt }],
            "temperature": self.temperature,
            "max_tokens": self.max_tokens,
        });

        let mut req = self.agent.post(&self.url);
        if let Some(key) = &self.api_key {
            req = req.set("Authorization", &format!("Bearer {key}"));
        }

        let resp = req.send_json(body).map_err(|e| match e {
            ureq::Error::Status(code, r) => {
                let msg = r.into_string().unwrap_or_default();
                anyhow::anyhow!("endpoint returned HTTP {code}: {}", msg.trim())
            }
            ureq::Error::Transport(t) => anyhow::anyhow!("transport error: {t}"),
        })?;

        let json: serde_json::Value = resp
            .into_json()
            .map_err(|e| anyhow::anyhow!("response was not JSON: {e}"))?;

        let text = json["choices"][0]["message"]["content"]
            .as_str()
            .ok_or_else(|| anyhow::anyhow!("response missing choices[0].message.content"))?
            .to_string();

        let completion_tokens = json["usage"]["completion_tokens"]
            .as_u64()
            .map(|n| n as usize);

        Ok(Completion { text, completion_tokens })
    }
}
```

- [ ] **Step 4: Run tests to verify they pass**

Run: `cargo test -p reasonmetrics-cli --features bench model`
Expected: PASS (mock, host_of, and stub-endpoint tests).

- [ ] **Step 5: Commit**

```bash
git add crates/reasonmetrics-cli/src/bench/model.rs crates/reasonmetrics-cli/Cargo.toml
git commit -m "feat(bench): ureq OpenAI-compatible HttpModel + host_of"
```

---

### Task 6: Scorer + correctness adapter (`TaskRow`, `build_rows`)

**Files:**
- Create: `crates/reasonmetrics-cli/src/bench/score.rs`
- Modify: `crates/reasonmetrics-cli/src/bench/mod.rs` (add `pub mod score;`)

**Interfaces:**
- Consumes: `taskset::Task`, `model::Completion`, `reasonmetrics_core::config::ScoringConfig`, `reasonmetrics_core::scoring::score_traces`, `answers_match`, `extract_thinking`, `estimated_token_count`.
- Produces: `score::Attempt { task: Task, result: Result<Completion, String> }`; `score::TaskRow { id, correct, quality, tokens, tokens_estimated, error }` (derives `Serialize`, `Clone`); `score::split_completion(raw: &str) -> (String, String)`; `score::build_rows(attempts: &[Attempt], scoring: &ScoringConfig) -> Vec<TaskRow>`.

- [ ] **Step 1: Write the failing tests**

Create `crates/reasonmetrics-cli/src/bench/score.rs`:

```rust
#[cfg(test)]
mod tests {
    use super::*;
    use crate::bench::model::Completion;
    use crate::bench::taskset::Task;
    use reasonmetrics_core::config::ScoringConfig;

    fn task(id: &str, expected: &str) -> Task {
        Task { id: id.into(), problem: "What is 2+2?".into(), expected_answer: expected.into() }
    }

    #[test]
    fn split_completion_separates_think_and_answer() {
        let (think, ans) = split_completion("<think>2+2=4, check: 4</think> The answer is 4");
        assert_eq!(think, "2+2=4, check: 4");
        assert_eq!(ans, "The answer is 4");
    }

    #[test]
    fn split_completion_without_tags_puts_all_in_answer() {
        let (think, ans) = split_completion("just 4");
        assert_eq!(think, "");
        assert_eq!(ans, "just 4");
    }

    #[test]
    fn build_rows_marks_correct_and_carries_errors() {
        let scoring = ScoringConfig::default();
        let attempts = vec![
            Attempt {
                task: task("a", "4"),
                result: Ok(Completion {
                    text: "<think>2+2=4. verify 4</think> 4".into(),
                    completion_tokens: Some(9),
                }),
            },
            Attempt {
                task: task("b", "4"),
                result: Ok(Completion { text: "<think>hmm</think> 5".into(), completion_tokens: None }),
            },
            Attempt { task: task("c", "4"), result: Err("timeout".into()) },
        ];

        let rows = build_rows(&attempts, &scoring);
        assert_eq!(rows.len(), 3);

        assert_eq!(rows[0].id, "a");
        assert!(rows[0].correct);
        assert_eq!(rows[0].tokens, 9);
        assert!(!rows[0].tokens_estimated);
        assert!(rows[0].error.is_none());

        assert_eq!(rows[1].id, "b");
        assert!(!rows[1].correct); // "5" != "4"
        assert!(rows[1].tokens_estimated); // no usage → estimated
        assert!(rows[1].error.is_none());

        assert_eq!(rows[2].id, "c");
        assert!(!rows[2].correct);
        assert_eq!(rows[2].tokens, 0);
        assert_eq!(rows[2].error.as_deref(), Some("timeout"));
    }
}
```

Register the module — add to `crates/reasonmetrics-cli/src/bench/mod.rs`:

```rust
pub mod score;
```

- [ ] **Step 2: Run tests to verify they fail**

Run: `cargo test -p reasonmetrics-cli --features bench score`
Expected: FAIL to compile — `Attempt`, `TaskRow`, `split_completion`, `build_rows` undefined.

- [ ] **Step 3: Implement the adapter**

Prepend to `crates/reasonmetrics-cli/src/bench/score.rs`:

```rust
//! Turn model completions into scored, correctness-checked rows.

use serde::Serialize;

use reasonmetrics_core::config::ScoringConfig;
use reasonmetrics_core::scorers::accuracy_efficiency::answers_match;
use reasonmetrics_core::scoring::score_traces;
use reasonmetrics_core::trace::{estimated_token_count, extract_thinking, TraceRecord};

use crate::bench::model::Completion;
use crate::bench::taskset::Task;

/// One model attempt at one task: either a completion or an error message.
pub struct Attempt {
    pub task: Task,
    pub result: Result<Completion, String>,
}

#[derive(Debug, Clone, Serialize)]
pub struct TaskRow {
    pub id: String,
    pub correct: bool,
    pub quality: f32,
    pub tokens: usize,
    pub tokens_estimated: bool,
    pub error: Option<String>,
}

/// Split a raw completion into (thinking, answer). Thinking is the `<think>`…
/// content (via the core extractor); the answer is whatever follows the last
/// `</think>`, or the whole text when there are no tags.
pub fn split_completion(raw: &str) -> (String, String) {
    let thinking = if raw.contains("<think>") {
        extract_thinking(raw)
    } else {
        String::new()
    };
    let answer = match raw.rfind("</think>") {
        Some(idx) => raw[idx + "</think>".len()..].trim().to_string(),
        None => raw.trim().to_string(),
    };
    (thinking, answer)
}

pub fn build_rows(attempts: &[Attempt], scoring: &ScoringConfig) -> Vec<TaskRow> {
    // Build trace records for the successful attempts, remembering their index
    // so results can be re-interleaved with the errored ones in task order.
    let mut records: Vec<TraceRecord> = Vec::new();
    let mut ok_index: Vec<usize> = Vec::new();
    let mut tokens: Vec<(usize, bool)> = Vec::new(); // (count, estimated)

    for (i, att) in attempts.iter().enumerate() {
        if let Ok(c) = &att.result {
            let (thinking, answer) = split_completion(&c.text);
            let (count, estimated) = match c.completion_tokens {
                Some(n) => (n, false),
                None => (
                    estimated_token_count(&thinking) + estimated_token_count(&answer),
                    true,
                ),
            };
            tokens.push((count, estimated));
            ok_index.push(i);
            records.push(TraceRecord {
                id: att.task.id.clone(),
                problem: att.task.problem.clone(),
                thinking,
                answer,
                domain: None,
                source: None,
                expected_answer: Some(att.task.expected_answer.clone()),
                extra: std::collections::HashMap::new(),
            });
        }
    }

    let scored = score_traces(&records, scoring);

    // Assemble rows back in original task order.
    let mut ok_rows: std::collections::HashMap<usize, TaskRow> = std::collections::HashMap::new();
    for (slot, &i) in ok_index.iter().enumerate() {
        let att = &attempts[i];
        let expected = &att.task.expected_answer;
        let (count, estimated) = tokens[slot];
        ok_rows.insert(
            i,
            TaskRow {
                id: att.task.id.clone(),
                correct: answers_match(&records[slot].answer, expected),
                quality: scored[slot].quality_score,
                tokens: count,
                tokens_estimated: estimated,
                error: None,
            },
        );
    }

    attempts
        .iter()
        .enumerate()
        .map(|(i, att)| match &att.result {
            Ok(_) => ok_rows.remove(&i).expect("ok row built for every Ok attempt"),
            Err(msg) => TaskRow {
                id: att.task.id.clone(),
                correct: false,
                quality: 0.0,
                tokens: 0,
                tokens_estimated: false,
                error: Some(msg.clone()),
            },
        })
        .collect()
}
```

- [ ] **Step 4: Run tests to verify they pass**

Run: `cargo test -p reasonmetrics-cli --features bench score`
Expected: PASS.

- [ ] **Step 5: Commit**

```bash
git add crates/reasonmetrics-cli/src/bench/score.rs crates/reasonmetrics-cli/src/bench/mod.rs
git commit -m "feat(bench): scorer + correctness adapter (build_rows)"
```

---

### Task 7: Aggregator (`BenchMetrics`, `aggregate`)

**Files:**
- Create: `crates/reasonmetrics-cli/src/bench/aggregate.rs`
- Modify: `crates/reasonmetrics-cli/src/bench/mod.rs` (add `pub mod aggregate;`)

**Interfaces:**
- Consumes: `score::TaskRow`.
- Produces: `aggregate::BenchMetrics { n_attempted, n_scored, n_errored, accuracy, mean_quality, tokens_per_correct: Option<f32>, cost_per_1k_correct: Option<f32> }` (derives `Serialize`, `Debug`, `Clone`); `aggregate::aggregate(rows: &[TaskRow], cost_per_mtok: Option<f32>) -> BenchMetrics`.

- [ ] **Step 1: Write the failing tests**

Create `crates/reasonmetrics-cli/src/bench/aggregate.rs`:

```rust
#[cfg(test)]
mod tests {
    use super::*;
    use crate::bench::score::TaskRow;

    fn row(id: &str, correct: bool, quality: f32, tokens: usize, err: Option<&str>) -> TaskRow {
        TaskRow {
            id: id.into(),
            correct,
            quality,
            tokens,
            tokens_estimated: false,
            error: err.map(String::from),
        }
    }

    #[test]
    fn aggregates_counts_accuracy_and_costs() {
        let rows = vec![
            row("a", true, 80.0, 100, None),
            row("b", false, 40.0, 300, None),
            row("c", true, 60.0, 200, None),
            row("d", false, 0.0, 0, Some("timeout")),
        ];
        let m = aggregate(&rows, Some(0.50));

        assert_eq!(m.n_attempted, 4);
        assert_eq!(m.n_scored, 3);
        assert_eq!(m.n_errored, 1);
        assert!((m.accuracy - 2.0 / 3.0).abs() < 1e-6);
        assert!((m.mean_quality - 60.0).abs() < 1e-6); // (80+40+60)/3
        // total scored tokens = 600, correct = 2 → 300 tokens/correct
        assert!((m.tokens_per_correct.unwrap() - 300.0).abs() < 1e-6);
        // cost = 600/1e6 * 0.50 = 0.0003 ; per correct = 0.00015 ; per 1k = 0.15
        assert!((m.cost_per_1k_correct.unwrap() - 0.15).abs() < 1e-6);
    }

    #[test]
    fn zero_correct_yields_none_ratios() {
        let rows = vec![row("a", false, 10.0, 100, None)];
        let m = aggregate(&rows, Some(0.50));
        assert_eq!(m.accuracy, 0.0);
        assert!(m.tokens_per_correct.is_none());
        assert!(m.cost_per_1k_correct.is_none());
    }

    #[test]
    fn no_cost_flag_yields_none_cost() {
        let rows = vec![row("a", true, 90.0, 100, None)];
        let m = aggregate(&rows, None);
        assert!(m.tokens_per_correct.is_some());
        assert!(m.cost_per_1k_correct.is_none());
    }
}
```

Register the module — add to `crates/reasonmetrics-cli/src/bench/mod.rs`:

```rust
pub mod aggregate;
```

- [ ] **Step 2: Run tests to verify they fail**

Run: `cargo test -p reasonmetrics-cli --features bench aggregate`
Expected: FAIL to compile — `BenchMetrics`, `aggregate` undefined.

- [ ] **Step 3: Implement the aggregator**

Prepend to `crates/reasonmetrics-cli/src/bench/aggregate.rs`:

```rust
//! Reduce per-task rows into leaderboard metrics.

use serde::Serialize;

use crate::bench::score::TaskRow;

#[derive(Debug, Clone, Serialize)]
pub struct BenchMetrics {
    pub n_attempted: usize,
    pub n_scored: usize,
    pub n_errored: usize,
    pub accuracy: f32,
    pub mean_quality: f32,
    pub tokens_per_correct: Option<f32>,
    pub cost_per_1k_correct: Option<f32>,
}

pub fn aggregate(rows: &[TaskRow], cost_per_mtok: Option<f32>) -> BenchMetrics {
    let n_attempted = rows.len();
    let scored: Vec<&TaskRow> = rows.iter().filter(|r| r.error.is_none()).collect();
    let n_scored = scored.len();
    let n_errored = n_attempted - n_scored;

    let n_correct = scored.iter().filter(|r| r.correct).count();
    let accuracy = if n_scored > 0 {
        n_correct as f32 / n_scored as f32
    } else {
        0.0
    };
    let mean_quality = if n_scored > 0 {
        scored.iter().map(|r| r.quality).sum::<f32>() / n_scored as f32
    } else {
        0.0
    };

    let total_tokens: usize = scored.iter().map(|r| r.tokens).sum();
    let tokens_per_correct = if n_correct > 0 {
        Some(total_tokens as f32 / n_correct as f32)
    } else {
        None
    };
    let cost_per_1k_correct = match (cost_per_mtok, n_correct > 0) {
        (Some(cost), true) => {
            let total_cost = total_tokens as f32 / 1_000_000.0 * cost;
            Some(total_cost / n_correct as f32 * 1000.0)
        }
        _ => None,
    };

    BenchMetrics {
        n_attempted,
        n_scored,
        n_errored,
        accuracy,
        mean_quality,
        tokens_per_correct,
        cost_per_1k_correct,
    }
}
```

- [ ] **Step 4: Run tests to verify they pass**

Run: `cargo test -p reasonmetrics-cli --features bench aggregate`
Expected: PASS.

- [ ] **Step 5: Commit**

```bash
git add crates/reasonmetrics-cli/src/bench/aggregate.rs crates/reasonmetrics-cli/src/bench/mod.rs
git commit -m "feat(bench): leaderboard metric aggregation"
```

---

### Task 8: Result assembly + writer + leaderboard rendering

**Files:**
- Create: `crates/reasonmetrics-cli/src/bench/result.rs`
- Modify: `crates/reasonmetrics-cli/src/bench/mod.rs` (add `pub mod result;`)

**Interfaces:**
- Consumes: `aggregate::BenchMetrics`, `score::TaskRow`, `taskset::TaskSet`, `LeaderboardFormat`.
- Produces: `result::BenchResult` (derives `Serialize`) with constructor `BenchResult::new(...)` (signature below); `result::BenchResult::default_out_path(&self) -> std::path::PathBuf`; `result::BenchResult::write_json(&self, path: &std::path::Path) -> anyhow::Result<()>`; `result::BenchResult::render(&self, format: LeaderboardFormat) -> String`.

- [ ] **Step 1: Write the failing tests**

Create `crates/reasonmetrics-cli/src/bench/result.rs`:

```rust
#[cfg(test)]
mod tests {
    use super::*;
    use crate::bench::aggregate::BenchMetrics;
    use crate::bench::score::TaskRow;
    use crate::bench::LeaderboardFormat;

    fn sample() -> BenchResult {
        let metrics = BenchMetrics {
            n_attempted: 2,
            n_scored: 2,
            n_errored: 0,
            accuracy: 0.5,
            mean_quality: 70.0,
            tokens_per_correct: Some(300.0),
            cost_per_1k_correct: None,
        };
        let rows = vec![
            TaskRow { id: "a".into(), correct: true, quality: 80.0, tokens: 100, tokens_estimated: false, error: None },
            TaskRow { id: "b".into(), correct: false, quality: 60.0, tokens: 200, tokens_estimated: false, error: None },
        ];
        BenchResult::new(
            "reasonmetrics bench --model m".into(),
            ("overthinking-v1".into(), "abc123".into(), 2),
            "m".into(),
            "localhost:8000".into(),
            (0.0, 8192, 1),
            false,
            metrics,
            rows,
        )
    }

    #[test]
    fn default_out_path_uses_taskset_model_and_shorthash() {
        let r = sample();
        let p = r.default_out_path();
        assert_eq!(p, std::path::PathBuf::from("results/overthinking-v1-m-abc123.json"));
    }

    #[test]
    fn json_roundtrips_and_omits_secrets() {
        let r = sample();
        let json = serde_json::to_string(&r).unwrap();
        assert!(json.contains("\"schema_version\":\"1\""));
        assert!(json.contains("\"endpoint_host\":\"localhost:8000\""));
        assert!(json.contains("\"task_set\""));
        assert!(!json.to_lowercase().contains("authorization"));
    }

    #[test]
    fn table_render_has_header_and_a_row() {
        let out = sample().render(LeaderboardFormat::Table);
        assert!(out.contains("model"));
        assert!(out.contains("quality"));
        assert!(out.contains("50.0%") || out.contains("0.50")); // accuracy shown
        assert!(out.contains("m"));
    }
}
```

Register the module — add to `crates/reasonmetrics-cli/src/bench/mod.rs`:

```rust
pub mod result;
```

- [ ] **Step 2: Run tests to verify they fail**

Run: `cargo test -p reasonmetrics-cli --features bench result`
Expected: FAIL to compile — `BenchResult` undefined.

- [ ] **Step 3: Implement result assembly, writer, and rendering**

Prepend to `crates/reasonmetrics-cli/src/bench/result.rs`:

```rust
//! The committed result artifact and its leaderboard renderings.

use std::path::{Path, PathBuf};
use std::time::{SystemTime, UNIX_EPOCH};

use serde::Serialize;

use crate::bench::aggregate::BenchMetrics;
use crate::bench::score::TaskRow;
use crate::bench::LeaderboardFormat;

#[derive(Debug, Clone, Serialize)]
pub struct TaskSetMeta {
    pub name: String,
    pub sha256: String,
    pub n: usize,
}

#[derive(Debug, Clone, Serialize)]
pub struct Sampling {
    pub temperature: f32,
    pub max_tokens: usize,
    pub samples: usize,
}

#[derive(Debug, Clone, Serialize)]
pub struct BenchResult {
    pub schema_version: &'static str,
    pub tool_version: &'static str,
    pub generated_at: u64,
    pub command: String,
    pub task_set: TaskSetMeta,
    pub model: String,
    pub endpoint_host: String,
    pub sampling: Sampling,
    pub tokens_estimated: bool,
    pub metrics: BenchMetrics,
    pub results: Vec<TaskRow>,
}

impl BenchResult {
    /// `task_set` is (name, sha256, n); `sampling` is (temperature, max_tokens, samples).
    #[allow(clippy::too_many_arguments)]
    pub fn new(
        command: String,
        task_set: (String, String, usize),
        model: String,
        endpoint_host: String,
        sampling: (f32, usize, usize),
        tokens_estimated: bool,
        metrics: BenchMetrics,
        results: Vec<TaskRow>,
    ) -> Self {
        let generated_at = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .map(|d| d.as_secs())
            .unwrap_or(0);
        Self {
            schema_version: "1",
            tool_version: env!("CARGO_PKG_VERSION"),
            generated_at,
            command,
            task_set: TaskSetMeta { name: task_set.0, sha256: task_set.1, n: task_set.2 },
            model,
            endpoint_host,
            sampling: Sampling { temperature: sampling.0, max_tokens: sampling.1, samples: sampling.2 },
            tokens_estimated,
            metrics,
            results,
        }
    }

    pub fn default_out_path(&self) -> PathBuf {
        let model_slug: String = self
            .model
            .chars()
            .map(|c| if c.is_ascii_alphanumeric() || c == '-' || c == '.' { c } else { '_' })
            .collect();
        let short = &self.task_set.sha256[..self.task_set.sha256.len().min(6)];
        PathBuf::from(format!("results/{}-{}-{}.json", self.task_set.name, model_slug, short))
    }

    pub fn write_json(&self, path: &Path) -> anyhow::Result<()> {
        if let Some(parent) = path.parent() {
            if !parent.as_os_str().is_empty() {
                std::fs::create_dir_all(parent)?;
            }
        }
        let json = serde_json::to_string_pretty(self)?;
        std::fs::write(path, json)?;
        Ok(())
    }

    pub fn render(&self, format: LeaderboardFormat) -> String {
        match format {
            LeaderboardFormat::Json => serde_json::to_string_pretty(self).unwrap_or_default(),
            LeaderboardFormat::Table => self.render_table(),
            LeaderboardFormat::Md => self.render_md(),
            LeaderboardFormat::Html => self.render_html(),
        }
    }

    fn cells(&self) -> (String, String, String, String, String) {
        let m = &self.metrics;
        let quality = format!("{:.1}", m.mean_quality);
        let accuracy = format!("{:.1}%", m.accuracy * 100.0);
        let tpc = m.tokens_per_correct.map(|v| format!("{v:.0}")).unwrap_or_else(|| "-".into());
        let cost = m.cost_per_1k_correct.map(|v| format!("{v:.2}")).unwrap_or_else(|| "-".into());
        let n = format!("{}", m.n_scored);
        (quality, accuracy, tpc, cost, n)
    }

    fn render_table(&self) -> String {
        let (quality, accuracy, tpc, cost, n) = self.cells();
        format!(
            "{:<28} {:>8} {:>9} {:>13} {:>12} {:>5}\n{}\n{:<28} {:>8} {:>9} {:>13} {:>12} {:>5}\n",
            "model", "quality", "accuracy", "tokens/correct", "cost/1k", "n",
            "-".repeat(78),
            self.model, quality, accuracy, tpc, cost, n,
        )
    }

    fn render_md(&self) -> String {
        let (quality, accuracy, tpc, cost, n) = self.cells();
        format!(
            "| model | quality | accuracy | tokens/correct | cost/1k | n |\n\
             |---|---|---|---|---|---|\n\
             | {} | {} | {} | {} | {} | {} |\n",
            self.model, quality, accuracy, tpc, cost, n,
        )
    }

    fn render_html(&self) -> String {
        let (quality, accuracy, tpc, cost, n) = self.cells();
        format!(
            "<table>\n<tr><th>model</th><th>quality</th><th>accuracy</th>\
             <th>tokens/correct</th><th>cost/1k</th><th>n</th></tr>\n\
             <tr><td>{}</td><td>{}</td><td>{}</td><td>{}</td><td>{}</td><td>{}</td></tr>\n</table>\n",
            self.model, quality, accuracy, tpc, cost, n,
        )
    }
}
```

- [ ] **Step 4: Run tests to verify they pass**

Run: `cargo test -p reasonmetrics-cli --features bench result`
Expected: PASS.

- [ ] **Step 5: Commit**

```bash
git add crates/reasonmetrics-cli/src/bench/result.rs crates/reasonmetrics-cli/src/bench/mod.rs
git commit -m "feat(bench): result artifact, JSON writer, leaderboard rendering"
```

---

### Task 9: Runner + `run` wiring + end-to-end test

**Files:**
- Create: `crates/reasonmetrics-cli/src/bench/runner.rs`
- Modify: `crates/reasonmetrics-cli/src/bench/mod.rs` (add `pub mod runner;`, replace stub `run`)

**Interfaces:**
- Consumes: everything above (`taskset`, `model`, `score`, `aggregate`, `result`).
- Produces: `runner::run_tasks(model: &dyn model::Model, tasks: &[taskset::Task], concurrency: usize, retries: usize) -> Vec<score::Attempt>`; a real `bench::run` implementation.

- [ ] **Step 1: Write the failing test (runner over a MockModel)**

Create `crates/reasonmetrics-cli/src/bench/runner.rs`:

```rust
#[cfg(test)]
mod tests {
    use super::*;
    use crate::bench::model::{Completion, MockModel};
    use crate::bench::taskset::Task;

    #[test]
    fn run_tasks_preserves_order_and_records_errors() {
        let tasks = vec![
            Task { id: "a".into(), problem: "P-A".into(), expected_answer: "1".into() },
            Task { id: "b".into(), problem: "P-B".into(), expected_answer: "2".into() },
        ];
        // Only P-A has a canned response; P-B will error out.
        let mock = MockModel::new(vec![(
            "P-A".to_string(),
            Completion { text: "<think>..</think> 1".into(), completion_tokens: Some(3) },
        )]);

        let attempts = run_tasks(&mock, &tasks, 2, 0);
        assert_eq!(attempts.len(), 2);
        assert_eq!(attempts[0].task.id, "a");
        assert!(attempts[0].result.is_ok());
        assert_eq!(attempts[1].task.id, "b");
        assert!(attempts[1].result.is_err());
    }
}
```

Register the module and prepare to replace the stub — add to `crates/reasonmetrics-cli/src/bench/mod.rs`:

```rust
pub mod runner;
```

- [ ] **Step 2: Run test to verify it fails**

Run: `cargo test -p reasonmetrics-cli --features bench run_tasks`
Expected: FAIL to compile — `run_tasks` undefined.

- [ ] **Step 3: Implement the runner**

Prepend to `crates/reasonmetrics-cli/src/bench/runner.rs`:

```rust
//! Drive the task set against a model under a bounded rayon pool.

use std::time::Duration;

use rayon::prelude::*;

use crate::bench::model::Model;
use crate::bench::score::Attempt;
use crate::bench::taskset::Task;

fn complete_with_retries(model: &dyn Model, prompt: &str, retries: usize) -> Result<crate::bench::model::Completion, String> {
    let mut last = String::new();
    for attempt in 0..=retries {
        match model.complete(prompt) {
            Ok(c) => return Ok(c),
            Err(e) => {
                last = e.to_string();
                if attempt < retries {
                    // Linear backoff; keep it short so a run doesn't stall.
                    std::thread::sleep(Duration::from_millis(250 * (attempt as u64 + 1)));
                }
            }
        }
    }
    Err(last)
}

pub fn run_tasks(model: &dyn Model, tasks: &[Task], concurrency: usize, retries: usize) -> Vec<Attempt> {
    let pool = rayon::ThreadPoolBuilder::new()
        .num_threads(concurrency.max(1))
        .build()
        .expect("failed to build bench thread pool");

    pool.install(|| {
        tasks
            .par_iter()
            .map(|task| Attempt {
                task: task.clone(),
                result: complete_with_retries(model, &task.problem, retries),
            })
            .collect()
    })
}
```

- [ ] **Step 4: Run test to verify it passes**

Run: `cargo test -p reasonmetrics-cli --features bench run_tasks`
Expected: PASS.

- [ ] **Step 5: Replace the stub `run` with the real pipeline**

In `crates/reasonmetrics-cli/src/bench/mod.rs`, replace the stub `run` function body with:

```rust
pub fn run(args: BenchArgs, scoring: &ScoringConfig) -> anyhow::Result<()> {
    let task_set = taskset::load(&args.task_set)?;
    eprintln!(
        "Loaded task set `{}` ({} tasks, sha256 {}…)",
        task_set.name,
        task_set.tasks.len(),
        &task_set.sha256[..task_set.sha256.len().min(8)]
    );

    let api_key = match &args.api_key_env {
        Some(var) => Some(
            std::env::var(var)
                .map_err(|_| anyhow::anyhow!("env var `{var}` (from --api-key-env) is not set"))?,
        ),
        None => None,
    };

    let http = model::HttpModel::new(
        &args.endpoint,
        &args.model,
        api_key,
        args.temperature,
        args.max_tokens,
    );

    let attempts = runner::run_tasks(&http, &task_set.tasks, args.concurrency, args.retries);
    let rows = score::build_rows(&attempts, scoring);
    let metrics = aggregate::aggregate(&rows, args.cost_per_mtok);
    let any_estimated = rows.iter().any(|r| r.tokens_estimated);

    let command = std::env::args().collect::<Vec<_>>().join(" ");
    let result = result::BenchResult::new(
        command,
        (task_set.name.clone(), task_set.sha256.clone(), task_set.tasks.len()),
        args.model.clone(),
        model::host_of(&args.endpoint),
        (args.temperature, args.max_tokens, 1),
        any_estimated,
        metrics,
        rows,
    );

    let out_path = args.out.clone().unwrap_or_else(|| result.default_out_path());
    result.write_json(&out_path)?;
    eprintln!("Result written to {}", out_path.display());

    println!("{}", result.render(args.format));
    if result.metrics.n_errored > 0 {
        eprintln!(
            "Warning: {} of {} tasks errored and were excluded from accuracy.",
            result.metrics.n_errored, result.metrics.n_attempted
        );
    }
    Ok(())
}
```

Also add these imports at the top of `mod.rs` (below the existing `use` lines):

```rust
use crate::bench::{aggregate, model, result, runner, score, taskset};
```

- [ ] **Step 6: Run the full bench test suite**

Run: `cargo test -p reasonmetrics-cli --features bench`
Expected: PASS (all bench unit tests).

Run: `cargo build -p reasonmetrics-cli`
Expected: PASS (default features still compile; no `ureq`/`sha2`).

- [ ] **Step 7: Commit**

```bash
git add crates/reasonmetrics-cli/src/bench/runner.rs crates/reasonmetrics-cli/src/bench/mod.rs
git commit -m "feat(bench): runner + full run pipeline wiring"
```

---

### Task 10: CLI integration test against a stub endpoint

**Files:**
- Create: `crates/reasonmetrics-cli/tests/bench_cli.rs`

**Interfaces:**
- Consumes: the built `reasonmetrics` binary with `--features bench`.
- Produces: an end-to-end regression test.

- [ ] **Step 1: Write the failing integration test**

Create `crates/reasonmetrics-cli/tests/bench_cli.rs`:

```rust
#![cfg(feature = "bench")]

use std::io::Read;

// Spawn a stub OpenAI-compatible endpoint that answers every task with "the
// answer is 43", then run the real binary against it and check the artifact.
#[test]
fn bench_end_to_end_writes_result_json() {
    let server = tiny_http::Server::http("127.0.0.1:0").unwrap();
    let addr = server.server_addr().to_ip().unwrap();
    let base = format!("http://{}:{}/v1", addr.ip(), addr.port());

    let handle = std::thread::spawn(move || {
        // 12 tasks in overthinking-v1 → answer each request.
        for _ in 0..12 {
            let Ok(mut req) = server.recv() else { break };
            let mut _body = String::new();
            let _ = req.as_reader().read_to_string(&mut _body);
            let payload = r#"{"choices":[{"message":{"content":"<think>compute</think> 43"}}],
                             "usage":{"completion_tokens":4}}"#;
            let resp = tiny_http::Response::from_string(payload).with_header(
                "Content-Type: application/json".parse::<tiny_http::Header>().unwrap(),
            );
            let _ = req.respond(resp);
        }
    });

    let dir = tempfile::tempdir().unwrap();
    let out = dir.path().join("result.json");

    let mut cmd = assert_cmd::Command::cargo_bin("reasonmetrics").unwrap();
    cmd.args([
        "bench",
        "--endpoint", &base,
        "--model", "stub",
        "--task-set", "overthinking-v1",
        "--concurrency", "1",
        "--out", out.to_str().unwrap(),
        "--format", "json",
    ]);
    cmd.assert().success();
    handle.join().unwrap();

    let written = std::fs::read_to_string(&out).unwrap();
    let v: serde_json::Value = serde_json::from_str(&written).unwrap();
    assert_eq!(v["task_set"]["n"], 12);
    assert_eq!(v["metrics"]["n_scored"], 12);
    // ot-001 expects 43 → at least one correct; accuracy strictly positive.
    assert!(v["metrics"]["accuracy"].as_f64().unwrap() > 0.0);
    assert_eq!(v["schema_version"], "1");
}
```

Confirm `assert_cmd`, `tempfile`, and `tiny_http` are all in `[dev-dependencies]` (first two already present from the existing suite; `tiny_http` added in Task 5).

- [ ] **Step 2: Run test to verify it fails, then passes**

Run: `cargo test -p reasonmetrics-cli --features bench --test bench_cli`
Expected: PASS (this exercises already-implemented code end-to-end; if it fails, the failure localizes the integration gap).

- [ ] **Step 3: Commit**

```bash
git add crates/reasonmetrics-cli/tests/bench_cli.rs
git commit -m "test(bench): end-to-end CLI run against a stub endpoint"
```

---

### Task 11: Docs — subcommand, metrics, and caveats

**Files:**
- Modify: `README.md` (add a `bench` usage block under the CLI section)
- Create: `docs/BENCH.md`

**Interfaces:**
- Consumes: nothing.
- Produces: user-facing documentation.

- [ ] **Step 1: Write `docs/BENCH.md`**

Create `docs/BENCH.md`:

```markdown
# `reasonmetrics bench`

Run a fixed, version-pinned task set against any OpenAI-compatible
`/v1/chat/completions` endpoint, score each returned reasoning trace, and write
a commit-friendly result JSON plus a leaderboard.

Requires a build with the `bench` feature:

    cargo build --release --features bench

## Usage

    reasonmetrics bench \
      --endpoint http://localhost:8000/v1 \
      --model deepseek-r1:8b \
      --task-set overthinking-v1 \
      --temperature 0 \
      --cost-per-mtok 0.40 \
      --api-key-env OPENAI_API_KEY

The API key is read from the named env var, never a flag — so it never lands in
your shell history or the committed command.

## Metrics

- **quality** — mean ReasonMetrics composite (percentile vs real traces).
- **accuracy** — `n_correct / n_scored`.
- **tokens/correct** — total completion tokens over all attempted tasks divided
  by the number correct (so wasted tokens on wrong answers count against it).
- **cost/1k correct** — `(total_tokens/1e6 * cost_per_mtok) / n_correct * 1000`;
  shown only with `--cost-per-mtok`.

`n_attempted`, `n_scored`, and `n_errored` are all reported: an errored task is
never silently dropped from the denominator.

## Reproducibility & caveats

Each run writes a result JSON embedding the exact command, the task-set sha256,
and the tool version — so a leaderboard entry is a reviewable PR. `--temperature
0` reduces variance but does not guarantee bit-identical re-runs, and hosted
models change over time; the committed JSON is the record of what happened.

Correctness is a normalized answer match (numeric or string), not a proof — a
confident, tidy, wrong trace still scores its quality. See docs/LIMITATIONS.md.
```

- [ ] **Step 2: Add a short pointer in `README.md`**

Under the `## CLI` section, after the command examples block, add:

```markdown
Benchmark a model's reasoning over a fixed task set (build with `--features bench`):

```bash
reasonmetrics bench --endpoint http://localhost:8000/v1 --model deepseek-r1:8b --task-set overthinking-v1
```

See [docs/BENCH.md](docs/BENCH.md) for metrics and reproducibility notes.
```

- [ ] **Step 3: Verify the docs build/links**

Run: `cargo build -p reasonmetrics-cli --features bench`
Expected: PASS (sanity — docs change doesn't break build).

- [ ] **Step 4: Commit**

```bash
git add README.md docs/BENCH.md
git commit -m "docs(bench): document the bench subcommand, metrics, and caveats"
```

---

## Self-Review

**Spec coverage:**
- Subcommand, OpenAI-compatible endpoint → Tasks 2, 5, 9. ✓
- Fixed content-hashed task set → Task 3. ✓
- Quality + correctness scoring (reuse core) → Tasks 1, 6. ✓
- Metrics (quality/accuracy/tokens-per-correct/cost) → Task 7. ✓
- Result JSON (command + hash + version, host-only, n_attempted/scored/errored, tokens_estimated, generated_at seconds) → Task 8. ✓
- Error handling + retries + partial results → Tasks 6 (error rows), 9 (retries). ✓
- Feature-gating + lean default build → Tasks 2, verified in 9. ✓
- Testing (mock, correctness, metrics, golden-ish JSON, CLI integration, feature isolation) → Tasks 4–10. ✓
- Docs → Task 11. ✓
- Definition of Done items 1–5 → covered by Tasks 9 (both builds), 10 (real endpoint path via stub), 6–8 (schema), 11 (docs).

**Placeholder scan:** No TBD/TODO; every code step is complete. ✓

**Type consistency:** `Completion`, `Model`, `MockModel`, `HttpModel`, `host_of` (Tasks 4–5); `Attempt`, `TaskRow`, `split_completion`, `build_rows` (Task 6); `BenchMetrics`, `aggregate` (Task 7); `BenchResult::new/default_out_path/write_json/render` (Task 8); `run_tasks` (Task 9). Names/signatures referenced downstream match their definitions. ✓

**Deferred (out of v1, per spec):** multi-sample/pass@k, cross-run leaderboard assembly, expanding `overthinking-v1` from licensed datasets, and choosing a `ureq` TLS backend for release binaries. These are the spec's "Open implementation details" and are intentionally not tasks here.
