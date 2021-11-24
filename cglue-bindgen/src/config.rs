use serde::{Deserialize, Serialize};

#[derive(Debug, Default, Clone, Serialize, Deserialize)]
pub struct Config {
    pub default_container: Option<String>,
    pub default_context: Option<String>,
    pub function_prefix: Option<String>,
}
