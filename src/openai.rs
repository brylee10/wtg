//! Utilities for querying the OpenAI API via the chat completions endpoint.
//!
//! For specific details on request/response schemas, see the [OpenAI API chat completionsdocs](https://platform.openai.com/docs/api-reference/chat/create).

use std::{
    env,
    io::{BufRead, BufReader, Write},
    str::FromStr,
};

use serde::{Deserialize, Serialize};

use crate::cli::{Model, DEFAULT_LLM, DEFAULT_QUERY};

/// A `chat/completions` `messages` item
#[derive(Serialize, Deserialize)]
pub struct ChatMessage {
    pub role: String,
    pub content: String,
}

/// A `chat/completions` request body
#[derive(Serialize, Deserialize)]
pub struct ChatRequest {
    pub model: String,
    pub messages: Vec<ChatMessage>,
    pub stream: bool,
}

/// A `chat/completions` streaming response delta
#[derive(Deserialize)]
pub struct ChatDelta {
    pub content: Option<String>,
}

/// A `chat/completions` streaming response choice
#[derive(Deserialize)]
pub struct ChatStreamChoice {
    pub delta: ChatDelta,
}

/// A `chat/completions` streaming response
#[derive(Deserialize)]
pub struct ChatStreamResponse {
    pub choices: Vec<ChatStreamChoice>,
}

/// Query the OpenAI API via the chat completions endpoint
pub fn query_chatgpt(
    context: &str,
    prompt: Option<&str>,
    model: Option<Model>,
) -> Result<String, Box<dyn std::error::Error>> {
    let openai_key = env::var("WTG_OPENAI_KEY").expect("WTG_OPENAI_KEY not set");
    let default_model = env::var("WTG_LLM").unwrap_or_else(|_| DEFAULT_LLM.to_string());
    let model = model
        .map(|m| m.to_string())
        .unwrap_or_else(|| default_model);
    let default_prompt = env::var("WTG_PROMPT").unwrap_or_else(|_| DEFAULT_QUERY.to_string());
    let prompt = prompt.unwrap_or_else(|| &default_prompt);

    // Validate the user model is supported
    if Model::from_str(&model).is_err() {
        return Err(format!(
            "Model {} is not a supported model, double check your WTG_LLM env var. Only {} are supported.",
            model,
            Model::all_models().join(", ")
        )
        .into());
    }

    // Useful for debugging the model inputs
    // println!("Context: {}", context);
    // println!("User Prompt: {}", prompt);

    let client = reqwest::blocking::Client::new();
    let url = "https://api.openai.com/v1/chat/completions";

    let system_msg = ChatMessage {
        role: "system".to_string(),
        content: format!(
            "You are a helpful assistant. The user has run a command and received the following output: {}",
            context
        ),
    };
    let user_msg = ChatMessage {
        role: "user".to_string(),
        content: prompt.to_string(),
    };
    let req_body = ChatRequest {
        model: model.to_string(),
        messages: vec![system_msg, user_msg],
        stream: true, // Request a streaming response.
    };

    let response = client
        .post(url)
        .bearer_auth(openai_key)
        .json(&req_body)
        .send()?
        .error_for_status()?;

    let mut reader = BufReader::new(response);
    let mut line = String::new();
    let mut complete_response = String::new();

    while reader.read_line(&mut line)? != 0 {
        let trimmed = line.trim();
        if trimmed.is_empty() {
            line.clear();
            continue;
        }
        if trimmed.starts_with("data: ") {
            let data = trimmed.trim_start_matches("data: ").trim();
            if data == "[DONE]" {
                break;
            }
            let parsed: ChatStreamResponse = serde_json::from_str(data)?;
            if let Some(choice) = parsed.choices.first() {
                if let Some(content) = &choice.delta.content {
                    print!("{}", content);
                    std::io::stdout().flush()?;
                }
            }
        }
        complete_response.push_str(&line);
        line.clear();
    }
    println!();
    Ok(complete_response)
}
