use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TokenUsage {
    pub input: u64,
    pub output: u64,
    pub model: String,
}

impl TokenUsage {
    pub fn new(input: u64, output: u64, model: impl Into<String>) -> Self {
        Self {
            input,
            output,
            model: model.into(),
        }
    }

    pub fn total(&self) -> u64 {
        self.input + self.output
    }

    pub fn merge(&mut self, other: &TokenUsage) {
        self.input += other.input;
        self.output += other.output;
    }
}

/// Precio por 1M de tokens (input, output)
const PRICING: &[(&str, f64, f64)] = &[
    // Claude 4 (Sonnet)
    ("claude-sonnet-4-20250514", 3.0, 15.0),
    ("claude-sonnet-4", 3.0, 15.0),
    // Claude 4 (Opus)
    ("claude-opus-4-20250514", 15.0, 75.0),
    ("claude-opus-4", 15.0, 75.0),
    // Claude 3.7 Sonnet
    ("claude-3-7-sonnet-20250219", 3.0, 15.0),
    ("claude-3-7-sonnet", 3.0, 15.0),
    // Claude 3.5 Sonnet
    ("claude-3-5-sonnet-20241022", 3.0, 15.0),
    ("claude-3-5-sonnet-20240620", 3.0, 15.0),
    ("claude-3-5-sonnet", 3.0, 15.0),
    // Claude 3.5 Haiku
    ("claude-3-5-haiku-20241022", 0.80, 4.0),
    ("claude-3-5-haiku", 0.80, 4.0),
    // Claude 3 Opus
    ("claude-3-opus-20240229", 15.0, 75.0),
    ("claude-3-opus", 15.0, 75.0),
    // Claude 3 Sonnet
    ("claude-3-sonnet-20240229", 3.0, 15.0),
    ("claude-3-sonnet", 3.0, 15.0),
    // Claude 3 Haiku
    ("claude-3-haiku-20240307", 0.25, 1.25),
    ("claude-3-haiku", 0.25, 1.25),
];

pub fn estimate_cost(model: &str, input_tokens: u64, output_tokens: u64) -> Option<f64> {
    let model_lower = model.to_lowercase();

    for (pattern, input_price, output_price) in PRICING {
        if model_lower.contains(&pattern.to_lowercase()) {
            let input_cost = (input_tokens as f64 / 1_000_000.0) * input_price;
            let output_cost = (output_tokens as f64 / 1_000_000.0) * output_price;
            return Some(input_cost + output_cost);
        }
    }

    None
}
