//! MCP server — exposes LSP functionality via Model Context Protocol tools.
//!
//! Uses the official [`rmcp`] SDK with manual `ServerHandler` implementation
//! for compatibility with `rmcp` v0.16.

use std::sync::Arc;

use lspz_core::codec::compact;
use rmcp::{
    ErrorData, ServerHandler,
    model::{
        CallToolRequestParams, CallToolResult, Content, ErrorCode, ListToolsResult,
        PaginatedRequestParams, ServerCapabilities, ServerInfo, Tool,
    },
    service::{RequestContext, RoleServer},
};
use tokio::sync::Mutex;
use tracing::info;

use crate::pool::LspPool;

// ─── Tool Definitions ──────────────────────────────────────────────────────

fn tool_definitions() -> Vec<Tool> {
    vec![
        Tool::new(
            "get_diagnostics",
            "Get LSP diagnostics for a file. Returns compressed diagnostics (compact format) \
             with dedup and delta range encoding for token-efficient AI consumption.",
            rmcp::model::object(GetDiagnosticsInput::json_schema()),
        ),
        Tool::new(
            "get_completions",
            "Request LSP completions at a specific cursor position. Returns completion items \
             with labels, kinds, and optional detail text.",
            rmcp::model::object(GetCompletionsInput::json_schema()),
        ),
        Tool::new(
            "get_symbols",
            "Retrieve document symbols (functions, classes, variables, etc.) from an LSP server.",
            rmcp::model::object(GetSymbolsInput::json_schema()),
        ),
    ]
}

// ─── Input Types ───────────────────────────────────────────────────────────

#[derive(Debug, serde::Deserialize)]
struct GetDiagnosticsInput {
    uri: String,
    backend: String,
    language: String,
}

#[derive(Debug, serde::Deserialize)]
struct GetCompletionsInput {
    uri: String,
    backend: String,
    language: String,
    line: u32,
    character: u32,
}

#[derive(Debug, serde::Deserialize)]
struct GetSymbolsInput {
    uri: String,
    backend: String,
    language: String,
}

// ─── MCP Server ────────────────────────────────────────────────────────────

/// MCP server that exposes LSP functionality as tools.
#[derive(Clone)]
pub struct McpServer {
    pool: Arc<Mutex<LspPool>>,
}

impl McpServer {
    /// Create a new MCP server with an empty connection pool.
    pub fn new() -> Self {
        Self {
            pool: Arc::new(Mutex::new(LspPool::new())),
        }
    }

    async fn handle_diagnostics(&self, input: GetDiagnosticsInput) -> Result<String, ErrorData> {
        let mut pool = self.pool.lock().await;
        let session = pool
            .get_or_spawn(&input.language, &input.backend)
            .await
            .map_err(|e| ErrorData::internal_error(e.to_string(), None))?;

        let path = input
            .uri
            .strip_prefix("file://")
            .ok_or_else(|| ErrorData::invalid_request("URI must start with file://", None))?;
        let content = tokio::fs::read_to_string(path)
            .await
            .map_err(|e| ErrorData::internal_error(format!("Cannot read file: {e}"), None))?;

        session
            .send_notification(
                "textDocument/didOpen",
                serde_json::json!({
                    "textDocument": {
                        "uri": input.uri,
                        "languageId": "",
                        "version": 1,
                        "text": content,
                    }
                }),
            )
            .await
            .map_err(|e| ErrorData::internal_error(e.to_string(), None))?;

        let params = session
            .wait_for_notification("textDocument/publishDiagnostics")
            .await
            .map_err(|e| ErrorData::internal_error(e.to_string(), None))?;

        let compressed = compact::compress(&params)
            .map_err(|e| ErrorData::internal_error(e.to_string(), None))?;

        serde_json::to_string_pretty(&compressed)
            .map_err(|e| ErrorData::internal_error(e.to_string(), None))
    }

    async fn handle_completions(&self, input: GetCompletionsInput) -> Result<String, ErrorData> {
        let mut pool = self.pool.lock().await;
        let session = pool
            .get_or_spawn(&input.language, &input.backend)
            .await
            .map_err(|e| ErrorData::internal_error(e.to_string(), None))?;

        let path = input
            .uri
            .strip_prefix("file://")
            .ok_or_else(|| ErrorData::invalid_request("URI must start with file://", None))?;
        let content = tokio::fs::read_to_string(path)
            .await
            .map_err(|e| ErrorData::internal_error(format!("Cannot read file: {e}"), None))?;

        session
            .send_notification(
                "textDocument/didOpen",
                serde_json::json!({
                    "textDocument": {
                        "uri": input.uri,
                        "languageId": input.language,
                        "version": 1,
                        "text": content,
                    }
                }),
            )
            .await
            .map_err(|e| ErrorData::internal_error(e.to_string(), None))?;

        let result = session
            .send_request(
                "textDocument/completion",
                serde_json::json!({
                    "textDocument": { "uri": input.uri },
                    "position": { "line": input.line, "character": input.character },
                }),
            )
            .await
            .map_err(|e| ErrorData::internal_error(e.to_string(), None))?;

        serde_json::to_string_pretty(&result)
            .map_err(|e| ErrorData::internal_error(e.to_string(), None))
    }

    async fn handle_symbols(&self, input: GetSymbolsInput) -> Result<String, ErrorData> {
        let mut pool = self.pool.lock().await;
        let session = pool
            .get_or_spawn(&input.language, &input.backend)
            .await
            .map_err(|e| ErrorData::internal_error(e.to_string(), None))?;

        let path = input
            .uri
            .strip_prefix("file://")
            .ok_or_else(|| ErrorData::invalid_request("URI must start with file://", None))?;
        let content = tokio::fs::read_to_string(path)
            .await
            .map_err(|e| ErrorData::internal_error(format!("Cannot read file: {e}"), None))?;

        session
            .send_notification(
                "textDocument/didOpen",
                serde_json::json!({
                    "textDocument": {
                        "uri": input.uri,
                        "languageId": input.language,
                        "version": 1,
                        "text": content,
                    }
                }),
            )
            .await
            .map_err(|e| ErrorData::internal_error(e.to_string(), None))?;

        let result = session
            .send_request(
                "textDocument/documentSymbol",
                serde_json::json!({
                    "textDocument": { "uri": input.uri },
                }),
            )
            .await
            .map_err(|e| ErrorData::internal_error(e.to_string(), None))?;

        serde_json::to_string_pretty(&result)
            .map_err(|e| ErrorData::internal_error(e.to_string(), None))
    }
}

impl Default for McpServer {
    fn default() -> Self {
        Self::new()
    }
}

// ─── ServerHandler Implementation ──────────────────────────────────────────

impl ServerHandler for McpServer {
    fn get_info(&self) -> ServerInfo {
        ServerInfo {
            instructions: Some(
                "lspz MCP server — exposes LSP diagnostics, completions, and symbols as MCP tools. "
                    .into(),
            ),
            capabilities: ServerCapabilities::builder().enable_tools().build(),
            ..Default::default()
        }
    }

    async fn list_tools(
        &self,
        _request: Option<PaginatedRequestParams>,
        _context: RequestContext<RoleServer>,
    ) -> Result<ListToolsResult, ErrorData> {
        Ok(ListToolsResult::with_all_items(tool_definitions()))
    }

    async fn call_tool(
        &self,
        request: CallToolRequestParams,
        _context: RequestContext<RoleServer>,
    ) -> Result<CallToolResult, ErrorData> {
        let CallToolRequestParams {
            name, arguments, ..
        } = request;

        match name.as_ref() {
            "get_diagnostics" => {
                let input: GetDiagnosticsInput = serde_json::from_value(serde_json::Value::Object(
                    arguments.unwrap_or_default(),
                ))
                .map_err(|e| ErrorData::invalid_request(e.to_string(), None))?;
                let result = self.handle_diagnostics(input).await?;
                info!("get_diagnostics completed");
                Ok(CallToolResult::success(vec![Content::text(result)]))
            }
            "get_completions" => {
                let input: GetCompletionsInput = serde_json::from_value(serde_json::Value::Object(
                    arguments.unwrap_or_default(),
                ))
                .map_err(|e| ErrorData::invalid_request(e.to_string(), None))?;
                let result = self.handle_completions(input).await?;
                info!("get_completions completed");
                Ok(CallToolResult::success(vec![Content::text(result)]))
            }
            "get_symbols" => {
                let input: GetSymbolsInput = serde_json::from_value(serde_json::Value::Object(
                    arguments.unwrap_or_default(),
                ))
                .map_err(|e| ErrorData::invalid_request(e.to_string(), None))?;
                let result = self.handle_symbols(input).await?;
                info!("get_symbols completed");
                Ok(CallToolResult::success(vec![Content::text(result)]))
            }
            _ => Err(ErrorData::new(
                ErrorCode::METHOD_NOT_FOUND,
                format!("Unknown tool: {name}"),
                None,
            )),
        }
    }
}

// ─── JSON Schema Helpers ───────────────────────────────────────────────────

trait JsonSchema {
    fn json_schema() -> serde_json::Value;
}

impl JsonSchema for GetDiagnosticsInput {
    fn json_schema() -> serde_json::Value {
        serde_json::json!({
            "type": "object",
            "properties": {
                "uri": { "type": "string", "description": "File URI (e.g. file:///path/to/file.go)" },
                "backend": { "type": "string", "description": "Backend LSP server command (e.g. gopls, rust-analyzer)" },
                "language": { "type": "string", "description": "Language identifier for pool key (e.g. go, rust)" }
            },
            "required": ["uri", "backend", "language"]
        })
    }
}

impl JsonSchema for GetCompletionsInput {
    fn json_schema() -> serde_json::Value {
        serde_json::json!({
            "type": "object",
            "properties": {
                "uri": { "type": "string", "description": "File URI" },
                "backend": { "type": "string", "description": "Backend LSP server command" },
                "language": { "type": "string", "description": "Language identifier" },
                "line": { "type": "integer", "description": "Line number (0-based)" },
                "character": { "type": "integer", "description": "Character offset (0-based)" }
            },
            "required": ["uri", "backend", "language", "line", "character"]
        })
    }
}

impl JsonSchema for GetSymbolsInput {
    fn json_schema() -> serde_json::Value {
        serde_json::json!({
            "type": "object",
            "properties": {
                "uri": { "type": "string", "description": "File URI" },
                "backend": { "type": "string", "description": "Backend LSP server command" },
                "language": { "type": "string", "description": "Language identifier" }
            },
            "required": ["uri", "backend", "language"]
        })
    }
}
