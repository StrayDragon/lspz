//! Runtime configuration.
//!
//! Builder-pattern configuration with env-var overrides and TOML file support.

use std::path::Path;
use std::str::FromStr;

use serde::Deserialize;

use crate::error::LspzError;
use crate::metrics::MetricsConfig;

/// Output format for the proxy.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum OutputFormat {
    /// Compact JSON (current default).
    Json,
    /// TOON (Token-Oriented Object Notation).
    Toon,
    /// Standard LSP JSON passthrough (no compression in output).
    Passthrough,
}

impl FromStr for OutputFormat {
    type Err = LspzError;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        match s.trim().to_lowercase().as_str() {
            "json" => Ok(Self::Json),
            "toon" => Ok(Self::Toon),
            "passthrough" => Ok(Self::Passthrough),
            _ => Err(LspzError::Config(format!("unknown output format: {s}"))),
        }
    }
}

/// Per-type capping limits for LSP server responses.
///
/// A value of 0 means no limit (capping disabled for that type).
#[derive(Debug, Clone, Default, Deserialize)]
pub struct CappingConfig {
    /// Maximum number of diagnostics to keep (0 = unlimited).
    pub max_diags: usize,
    /// Maximum number of completion items to keep (0 = unlimited).
    pub max_completions: usize,
    /// Maximum number of document symbols to keep (0 = unlimited).
    pub max_symbols: usize,
}

impl CappingConfig {
    /// Returns `true` if any capping limit is set.
    pub fn any_enabled(&self) -> bool {
        self.max_diags > 0 || self.max_completions > 0 || self.max_symbols > 0
    }
}

/// Configuration for the lspz proxy.
#[derive(Debug, Clone, Deserialize)]
pub struct Config {
    /// Command used to launch the backend LSP server.
    pub backend_cmd: String,
    /// Per-type response capping limits.
    #[serde(default)]
    pub capping: CappingConfig,
    /// Whether to enable diagnostic compression.
    #[serde(default = "default_true")]
    pub enable_diag_compress: bool,
    /// Whether to enable completion compression (default: true).
    #[serde(default = "default_true")]
    pub enable_completion_compress: bool,
    /// Whether to enable hover compression (default: true).
    #[serde(default = "default_true")]
    pub enable_hover_compress: bool,
    /// Whether to enable document symbol compression (default: true).
    #[serde(default = "default_true")]
    pub enable_document_symbol_compress: bool,
    /// Whether to enable location compression (default: true).
    #[serde(default = "default_true")]
    pub enable_location_compress: bool,
    /// Whether to enable workspace symbol compression (default: true).
    #[serde(default = "default_true")]
    pub enable_workspace_symbol_compress: bool,
    /// Whether to enable workspace diagnostic compression (default: true).
    #[serde(default = "default_true")]
    pub enable_workspace_diag_compress: bool,
    /// Output format for intercepted messages (json, toon, passthrough).
    #[serde(default = "default_output_format")]
    pub output_format: OutputFormat,
    /// Log level (trace, debug, info, warn, error).
    #[serde(default = "default_log_level")]
    pub log_level: String,
    /// Runtime metrics configuration.
    #[serde(default)]
    pub metrics: MetricsConfig,
}

fn default_true() -> bool {
    true
}

fn default_output_format() -> OutputFormat {
    OutputFormat::Toon
}

fn default_log_level() -> String {
    "info".into()
}

impl Default for Config {
    fn default() -> Self {
        Self {
            backend_cmd: String::new(),
            capping: CappingConfig::default(),
            enable_diag_compress: true,
            enable_completion_compress: true,
            enable_hover_compress: true,
            enable_document_symbol_compress: true,
            enable_location_compress: true,
            enable_workspace_symbol_compress: true,
            enable_workspace_diag_compress: true,
            output_format: OutputFormat::Json,
            log_level: "info".into(),
            metrics: MetricsConfig::default(),
        }
    }
}

impl Config {
    /// Create a new [`ConfigBuilder`].
    pub fn builder() -> ConfigBuilder {
        ConfigBuilder::default()
    }

    /// Load config from a TOML file.
    ///
    /// Missing fields use their default values (same as `Config::default()`).
    pub fn from_file(path: impl AsRef<Path>) -> Result<Self, LspzError> {
        let content = std::fs::read_to_string(path.as_ref())
            .map_err(|e| LspzError::Config(format!("cannot read config file: {e}")))?;
        toml::from_str(&content).map_err(|e| LspzError::Config(format!("invalid config file: {e}")))
    }

    /// Returns `true` if the named interceptor is enabled in this config.
    ///
    /// Used by [`InterceptorChain`](crate::interceptors::InterceptorChain) at runtime
    /// to skip disabled interceptors without removing them from the chain.
    pub fn is_interceptor_enabled(&self, name: &str) -> bool {
        match name {
            "capping" => self.capping.any_enabled(),
            "diagnostics_compressor" => self.enable_diag_compress,
            "completion_compressor" => self.enable_completion_compress,
            "hover_compressor" => self.enable_hover_compress,
            "document_symbol_compressor" => self.enable_document_symbol_compress,
            "location_compressor" => self.enable_location_compress,
            "workspace_symbol_compressor" => self.enable_workspace_symbol_compress,
            "workspace_diagnostic_compressor" => self.enable_workspace_diag_compress,
            _ => true,
        }
    }
}

/// Builder for [`Config`].
#[derive(Debug, Default)]
pub struct ConfigBuilder {
    backend_cmd: Option<String>,
    capping: Option<CappingConfig>,
    enable_diag_compress: Option<bool>,
    enable_completion_compress: Option<bool>,
    enable_hover_compress: Option<bool>,
    enable_document_symbol_compress: Option<bool>,
    enable_location_compress: Option<bool>,
    enable_workspace_symbol_compress: Option<bool>,
    enable_workspace_diag_compress: Option<bool>,
    output_format: Option<OutputFormat>,
    log_level: Option<String>,
    metrics: Option<MetricsConfig>,
}

impl ConfigBuilder {
    /// Set the backend LSP server command.
    pub fn backend_cmd(mut self, cmd: impl Into<String>) -> Self {
        self.backend_cmd = Some(cmd.into());
        self
    }

    /// Enable or disable diagnostic compression.
    pub fn enable_diag_compress(mut self, enable: bool) -> Self {
        self.enable_diag_compress = Some(enable);
        self
    }

    /// Enable or disable completion compression.
    pub fn enable_completion_compress(mut self, enable: bool) -> Self {
        self.enable_completion_compress = Some(enable);
        self
    }

    /// Enable or disable hover compression.
    pub fn enable_hover_compress(mut self, enable: bool) -> Self {
        self.enable_hover_compress = Some(enable);
        self
    }

    /// Enable or disable document symbol compression.
    pub fn enable_document_symbol_compress(mut self, enable: bool) -> Self {
        self.enable_document_symbol_compress = Some(enable);
        self
    }

    /// Enable or disable location compression.
    pub fn enable_location_compress(mut self, enable: bool) -> Self {
        self.enable_location_compress = Some(enable);
        self
    }

    /// Enable or disable workspace symbol compression.
    pub fn enable_workspace_symbol_compress(mut self, enable: bool) -> Self {
        self.enable_workspace_symbol_compress = Some(enable);
        self
    }

    /// Enable or disable workspace diagnostic compression.
    pub fn enable_workspace_diag_compress(mut self, enable: bool) -> Self {
        self.enable_workspace_diag_compress = Some(enable);
        self
    }

    /// Set the output format (json, toon, passthrough).
    pub fn output_format(mut self, fmt: OutputFormat) -> Self {
        self.output_format = Some(fmt);
        self
    }

    /// Set the log level.
    pub fn log_level(mut self, level: impl Into<String>) -> Self {
        self.log_level = Some(level.into());
        self
    }

    /// Set the response capping limits.
    pub fn capping(mut self, capping: CappingConfig) -> Self {
        self.capping = Some(capping);
        self
    }

    /// Set the runtime metrics configuration.
    pub fn metrics(mut self, metrics: MetricsConfig) -> Self {
        self.metrics = Some(metrics);
        self
    }

    /// Build the [`Config`], validating required fields.
    pub fn build(self) -> Result<Config, LspzError> {
        let backend_cmd = self
            .backend_cmd
            .or_else(|| std::env::var("LSPZ_BACKEND_CMD").ok())
            .ok_or_else(|| LspzError::Config("backend_cmd is required".into()))?;

        let enable_diag_compress = self
            .enable_diag_compress
            .or_else(|| {
                std::env::var("LSPZ_ENABLE_DIAG_COMPRESS")
                    .ok()
                    .and_then(|v| v.parse().ok())
            })
            .unwrap_or(true);

        let enable_completion_compress = self
            .enable_completion_compress
            .or_else(|| {
                std::env::var("LSPZ_ENABLE_COMPLETION_COMPRESS")
                    .ok()
                    .and_then(|v| v.parse().ok())
            })
            .unwrap_or(true);

        let enable_hover_compress = self
            .enable_hover_compress
            .or_else(|| {
                std::env::var("LSPZ_ENABLE_HOVER_COMPRESS")
                    .ok()
                    .and_then(|v| v.parse().ok())
            })
            .unwrap_or(true);

        let enable_document_symbol_compress = self
            .enable_document_symbol_compress
            .or_else(|| {
                std::env::var("LSPZ_ENABLE_DOCUMENT_SYMBOL_COMPRESS")
                    .ok()
                    .and_then(|v| v.parse().ok())
            })
            .unwrap_or(true);

        let enable_location_compress = self
            .enable_location_compress
            .or_else(|| {
                std::env::var("LSPZ_ENABLE_LOCATION_COMPRESS")
                    .ok()
                    .and_then(|v| v.parse().ok())
            })
            .unwrap_or(true);

        let enable_workspace_symbol_compress = self
            .enable_workspace_symbol_compress
            .or_else(|| {
                std::env::var("LSPZ_ENABLE_WORKSPACE_SYMBOL_COMPRESS")
                    .ok()
                    .and_then(|v| v.parse().ok())
            })
            .unwrap_or(true);

        let enable_workspace_diag_compress = self
            .enable_workspace_diag_compress
            .or_else(|| {
                std::env::var("LSPZ_ENABLE_WORKSPACE_DIAG_COMPRESS")
                    .ok()
                    .and_then(|v| v.parse().ok())
            })
            .unwrap_or(true);

        let log_level = self
            .log_level
            .or_else(|| std::env::var("LSPZ_LOG_LEVEL").ok())
            .unwrap_or_else(|| "info".into());

        let output_format = self
            .output_format
            .or_else(|| {
                std::env::var("LSPZ_OUTPUT_FORMAT")
                    .ok()
                    .and_then(|v| OutputFormat::from_str(&v).ok())
            })
            .unwrap_or(OutputFormat::Toon);

        // Read metrics config from env vars or use builder value
        let metrics = self.metrics.unwrap_or_else(|| MetricsConfig {
            enabled: std::env::var("LSPZ_METRICS_ENABLED")
                .ok()
                .and_then(|v| v.parse().ok())
                .unwrap_or(false),
            report_interval_secs: std::env::var("LSPZ_METRICS_INTERVAL")
                .ok()
                .and_then(|v| v.parse().ok())
                .unwrap_or(0),
        });

        // Read capping from env vars or use builder value
        let capping = self.capping.unwrap_or_else(|| CappingConfig {
            max_diags: std::env::var("LSPZ_MAX_DIAGS")
                .ok()
                .and_then(|v| v.parse().ok())
                .unwrap_or(0),
            max_completions: std::env::var("LSPZ_MAX_COMPLETIONS")
                .ok()
                .and_then(|v| v.parse().ok())
                .unwrap_or(0),
            max_symbols: std::env::var("LSPZ_MAX_SYMBOLS")
                .ok()
                .and_then(|v| v.parse().ok())
                .unwrap_or(0),
        });

        Ok(Config {
            backend_cmd,
            capping,
            enable_diag_compress,
            enable_completion_compress,
            enable_hover_compress,
            enable_document_symbol_compress,
            enable_location_compress,
            enable_workspace_symbol_compress,
            enable_workspace_diag_compress,
            output_format,
            log_level,
            metrics,
        })
    }
}
