//! # lspz
//!
//! LSP compression proxy for AI Coding Agents.

/// Configuration for the LSP proxy
#[derive(Debug, Clone)]
pub struct Config {
    pub backend_cmd: String,
    pub enable_diag_compress: bool,
}

impl Config {
    pub fn builder() -> ConfigBuilder {
        ConfigBuilder::default()
    }
}

/// Builder for Config
#[derive(Debug, Clone, Default)]
pub struct ConfigBuilder {
    backend_cmd: Option<String>,
    enable_diag_compress: bool,
}

impl ConfigBuilder {
    pub fn backend_cmd(mut self, cmd: &str) -> Self {
        self.backend_cmd = Some(cmd.to_string());
        self
    }

    pub fn enable_diag_compress(mut self, enable: bool) -> Self {
        self.enable_diag_compress = enable;
        self
    }

    pub fn build(self) -> Config {
        Config {
            backend_cmd: self
                .backend_cmd
                .unwrap_or_else(|| "rust-analyzer".to_string()),
            enable_diag_compress: self.enable_diag_compress,
        }
    }
}

/// LSP compression proxy
pub struct Proxy {
    _config: Config,
}

impl Proxy {
    pub fn new(config: Config) -> Result<Self, std::io::Error> {
        Ok(Proxy { _config: config })
    }

    pub fn initialize(&mut self) -> Result<(), std::io::Error> {
        Ok(())
    }

    pub fn get_diagnostics(&self, _uri: &str) -> Result<Vec<Diagnostic>, std::io::Error> {
        Ok(vec![])
    }
}

/// LSP Diagnostic
#[derive(Debug, Clone)]
pub struct Diagnostic {
    pub message: String,
    pub severity: Severity,
}

#[derive(Debug, Clone)]
pub enum Severity {
    Error,
    Warning,
    Info,
    Hint,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn config_builder_works() {
        let config = Config::builder()
            .backend_cmd("gopls")
            .enable_diag_compress(true)
            .build();

        assert_eq!(config.backend_cmd, "gopls");
        assert!(config.enable_diag_compress);
    }

    #[test]
    fn proxy_create_works() {
        let config = Config::builder().build();
        let mut proxy = Proxy::new(config).unwrap();
        assert!(proxy.initialize().is_ok());
    }
}
