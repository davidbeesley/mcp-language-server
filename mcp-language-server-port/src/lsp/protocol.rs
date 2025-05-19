use serde::{Deserialize, Serialize};
use serde_json::Value;
use std::fmt;

/// MessageID represents a JSON-RPC ID which can be a string, number, or null
/// per the JSON-RPC 2.0 specification
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(untagged)]
pub enum MessageID {
    Number(i32),
    String(String),
    Null,
}

impl MessageID {
    pub fn as_string(&self) -> String {
        match self {
            MessageID::Number(n) => n.to_string(),
            MessageID::String(s) => s.clone(),
            MessageID::Null => "<null>".to_string(),
        }
    }

    pub fn equals(&self, other: &MessageID) -> bool {
        match (self, other) {
            (MessageID::Number(a), MessageID::Number(b)) => a == b,
            (MessageID::String(a), MessageID::String(b)) => a == b,
            (MessageID::Null, MessageID::Null) => true,
            _ => false,
        }
    }
}

impl fmt::Display for MessageID {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            MessageID::Number(n) => write!(f, "{}", n),
            MessageID::String(s) => write!(f, "{}", s),
            MessageID::Null => write!(f, "<null>"),
        }
    }
}

/// ResponseError represents a JSON-RPC 2.0 error
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ResponseError {
    pub code: i32,
    pub message: String,
}

/// Message represents a JSON-RPC 2.0 message
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Message {
    pub jsonrpc: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub id: Option<MessageID>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub method: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub params: Option<Value>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub result: Option<Value>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub error: Option<ResponseError>,
}

impl Message {
    pub fn new_request<T: Serialize>(
        id: MessageID,
        method: &str,
        params: T,
    ) -> Result<Self, serde_json::Error> {
        let params_value = serde_json::to_value(params)?;

        Ok(Self {
            jsonrpc: "2.0".to_string(),
            id: Some(id),
            method: Some(method.to_string()),
            params: Some(params_value),
            result: None,
            error: None,
        })
    }

    pub fn new_notification<T: Serialize>(
        method: &str,
        params: T,
    ) -> Result<Self, serde_json::Error> {
        let params_value = serde_json::to_value(params)?;

        Ok(Self {
            jsonrpc: "2.0".to_string(),
            id: None, // Notifications don't have an ID
            method: Some(method.to_string()),
            params: Some(params_value),
            result: None,
            error: None,
        })
    }

    pub fn new_response<T: Serialize>(id: MessageID, result: T) -> Result<Self, serde_json::Error> {
        let result_value = serde_json::to_value(result)?;

        Ok(Self {
            jsonrpc: "2.0".to_string(),
            id: Some(id),
            method: None,
            params: None,
            result: Some(result_value),
            error: None,
        })
    }

    pub fn new_error_response(id: MessageID, code: i32, message: &str) -> Self {
        Self {
            jsonrpc: "2.0".to_string(),
            id: Some(id),
            method: None,
            params: None,
            result: None,
            error: Some(ResponseError {
                code,
                message: message.to_string(),
            }),
        }
    }

    pub fn is_request(&self) -> bool {
        self.method.is_some() && self.id.is_some()
    }

    pub fn is_notification(&self) -> bool {
        self.method.is_some() && self.id.is_none()
    }

    pub fn is_response(&self) -> bool {
        self.method.is_none()
            && self.id.is_some()
            && (self.result.is_some() || self.error.is_some())
    }
}
