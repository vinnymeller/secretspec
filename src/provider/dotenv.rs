use super::Provider;
use crate::Result;
use std::collections::HashMap;
use std::fs;
use std::path::PathBuf;

pub struct DotEnvStorage {
    dotenv_path: PathBuf,
}

impl DotEnvStorage {
    pub fn new(dotenv_path: PathBuf) -> Self {
        Self { dotenv_path }
    }

    fn load_env_vars(&self) -> Result<HashMap<String, String>> {
        if !self.dotenv_path.exists() {
            return Ok(HashMap::new());
        }

        let mut vars = HashMap::new();
        let env_vars = dotenvy::from_path_iter(&self.dotenv_path)?;
        for item in env_vars {
            let (key, value) = item?;
            vars.insert(key, value);
        }
        Ok(vars)
    }

    fn save_env_vars(&self, vars: &HashMap<String, String>) -> Result<()> {
        let mut content = String::new();
        for (key, value) in vars {
            content.push_str(&format!("{}={}\n", key, value));
        }
        fs::write(&self.dotenv_path, content)?;
        Ok(())
    }
}

impl Provider for DotEnvStorage {
    fn get(&self, _project: &str, key: &str, _profile: Option<&str>) -> Result<Option<String>> {
        let vars = self.load_env_vars()?;
        Ok(vars.get(key).cloned())
    }

    fn set(&self, _project: &str, key: &str, value: &str, _profile: Option<&str>) -> Result<()> {
        let mut vars = self.load_env_vars()?;
        vars.insert(key.to_string(), value.to_string());
        self.save_env_vars(&vars)
    }
}
