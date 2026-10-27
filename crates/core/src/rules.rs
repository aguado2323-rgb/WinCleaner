use serde::{Deserialize, Serialize};
use std::path::PathBuf;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Rule {
    pub id: String,
    pub name: String,
    pub category: String,
    pub description: String,
    pub paths: Vec<String>,
    #[serde(default)]
    pub safe: bool,
}

#[derive(Deserialize)]
struct RuleFile {
    rule: Vec<Rule>,
}

const WINDOWS_RULES: &str = include_str!("../rules/windows.toml");

pub fn builtin_rules() -> Vec<Rule> {
    toml::from_str::<RuleFile>(WINDOWS_RULES)
        .expect("windows.toml embebido es válido")
        .rule
}

/// Expande `%VAR%`. Devuelve `None` si alguna variable no existe, para no
/// acabar limpiando una ruta distinta de la prevista.
pub fn expand_env(input: &str) -> Option<PathBuf> {
    let mut out = String::new();
    let mut rest = input;
    while let Some(start) = rest.find('%') {
        out.push_str(&rest[..start]);
        let after = &rest[start + 1..];
        let end = after.find('%')?;
        out.push_str(&std::env::var(&after[..end]).ok()?);
        rest = &after[end + 1..];
    }
    out.push_str(rest);
    Some(PathBuf::from(out))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn builtin_rules_parse() {
        let rules = builtin_rules();
        assert!(rules.iter().any(|r| r.id == "user-temp"));
    }

    #[test]
    fn expand_known_and_unknown_vars() {
        std::env::set_var("WC_TEST_DIR", "base");
        assert_eq!(expand_env("%WC_TEST_DIR%/x").unwrap(), PathBuf::from("base/x"));
        assert!(expand_env("%WC_NOPE_123%/x").is_none());
        assert!(expand_env("100%").is_none());
    }
}
