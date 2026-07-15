//! The model backend abstraction: a `Model` yields a `Completion` for a prompt.

use std::collections::HashMap;
use std::time::Duration;

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

/// Host[:port] of a URL, dropping scheme, path, and any credentials.
pub fn host_of(url: &str) -> String {
    let after_scheme = url.split_once("://").map(|(_, rest)| rest).unwrap_or(url);
    let host_port = after_scheme
        .split(['/', '?', '#'])
        .next()
        .unwrap_or(after_scheme);
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

        Ok(Completion {
            text,
            completion_tokens,
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn mock_returns_canned_completion() {
        let mock = MockModel::new(vec![(
            "What is 2+2?".to_string(),
            Completion {
                text: "<think>2+2=4</think> 4".into(),
                completion_tokens: Some(5),
            },
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
            let resp = tiny_http::Response::from_string(body).with_header(
                "Content-Type: application/json"
                    .parse::<tiny_http::Header>()
                    .unwrap(),
            );
            req.respond(resp).unwrap();
        });

        let m = HttpModel::new(&base, "stub-model", None, 0.0, 128);
        let c = m.complete("What is 1+1?").unwrap();
        handle.join().unwrap();

        assert_eq!(c.text, "<think>1+1=2</think> 2");
        assert_eq!(c.completion_tokens, Some(7));
    }
}
