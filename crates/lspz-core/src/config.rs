//! Runtime configuration.
//!
//! Builder-pattern configuration with env-var overrides.

use crate::error::LspzError;

/// Configuration for the lspz proxy.
#[derive(Debug, Clone)]
pub struct Config {
    /// Command used to launch the backend LSP server.
    pub backend_cmd: String,
    /// Whether to enable diagnostic compression.
    pub enable_diag_compress: bool,
    /// Whether to enable completion compression (default: true).
    pub enable_completion_compress: bool,
    /// Whether to enable hover compression (default: true).
    pub enable_hover_compress: bool,
    /// Log level (trace, debug, info, warn, error).
    pub log_level: String,
}

impl Default for Config {
    fn default() -> Self {
        Self {
            backend_cmd: String::new(),
            enable_diag_compress: true,
            enable_completion_compress: true,
            enable_hover_compress: true,
            log_level: "info".into(),
        }
    }
}

impl Config {
    /// Create a new [`ConfigBuilder`].
    pub fn builder() -> ConfigBuilder {
        ConfigBuilder::default()
    }
}

/// Builder for [`Config`].
#[derive(Debug, Default)]
pub struct ConfigBuilder {
    backend_cmd: Option<String>,
    enable_diag_compress: Option<bool>,
    enable_completion_compress: Option<bool>,
    enable_hover_compress: Option<bool>,
    log_level: Option<String>,
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

    /// Set the log level.
    pub fn log_level(mut self, level: impl Into<String>) -> Self {
        self.log_level = Some(level.into());
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

        let log_level = self
            .log_level
            .or_else(|| std::env::var("LSPZ_LOG_LEVEL").ok())
            .unwrap_or_else(|| "info".into());

        Ok(Config {
            backend_cmd,
            enable_diag_compress,
            enable_completion_compress,
            enable_hover_compress,
            log_level,
        })
    }
}
