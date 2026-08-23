use serde::Serialize;

#[derive(Debug, Serialize)]
pub struct DomainToolResult {
    pub content: Vec<DomainContent>,
    #[serde(rename = "isError", skip_serializing_if = "Option::is_none")]
    pub is_error: Option<bool>,
}

#[derive(Debug, Serialize)]
#[serde(tag = "type")]
pub enum DomainContent {
    #[serde(rename = "text")]
    Text { text: String },
}

impl DomainToolResult {
    pub fn text(text: impl Into<String>) -> Self {
        Self {
            content: vec![DomainContent::Text { text: text.into() }],
            is_error: None,
        }
    }

    pub fn error(message: impl Into<String>) -> Self {
        Self {
            content: vec![DomainContent::Text {
                text: message.into(),
            }],
            is_error: Some(true),
        }
    }
}

pub type ToolResult = DomainToolResult;
pub type ToolContent = DomainContent;
