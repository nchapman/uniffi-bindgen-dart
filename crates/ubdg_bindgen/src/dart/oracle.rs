#[derive(Debug, Default, Clone)]
pub struct CodeOracle;

impl CodeOracle {
    pub fn class_name(&self, name: &str) -> String {
        name.to_string()
    }
}
