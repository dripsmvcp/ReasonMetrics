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
}
