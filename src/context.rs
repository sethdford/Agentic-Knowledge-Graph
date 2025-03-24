use std::sync::Arc;
use crate::config::Config;

/// Application context containing shared configuration and resources
#[derive(Clone)]
pub struct Context {
    /// Shared configuration
    pub config: Arc<Config>,
}

impl Context {
    /// Create a new Context with the given configuration
    pub fn new(config: Config) -> Self {
        Self {
            config: Arc::new(config),
        }
    }
} 