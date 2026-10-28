use serde::{Deserialize, Serialize};
use std::path::PathBuf;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Rule {
    pub id: String,
    pub name: String,
    pub category: String,
    pub description: String,
    /// Explicación para la persona: qué es, qué pasa si se limpia y si hay riesgo.
    pub help: String,
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
    fn builtin_rules_are_well_formed() {
        let rules = builtin_rules();
        let mut ids: Vec<_> = rules.iter().map(|r| r.id.as_str()).collect();
        ids.sort();
        ids.dedup();
        assert_eq!(ids.len(), rules.len(), "ids duplicados");
        for r in &rules {
            assert!(!r.paths.is_empty(), "{} sin rutas", r.id);
            assert!(r.help.len() > 40, "{} sin explicación", r.id);
            for p in &r.paths {
                // Toda ruta debe colgar de una variable de entorno: nada de rutas fijas ni raíces.
                assert!(p.starts_with('%') && p.matches('%').count() == 2, "{}: {p}", r.id);
                // Salvo %TEMP% (carpeta de temporales), no se limpia la raíz de una variable.
                assert!(p == "%TEMP%" || p.contains('\\'), "{}: {p} apunta a la raíz de la variable", r.id);
            }
        }
    }

    #[test]
    fn expand_known_and_unknown_vars() {
        std::env::set_var("WC_TEST_DIR", "base");
        assert_eq!(expand_env("%WC_TEST_DIR%/x").unwrap(), PathBuf::from("base/x"));
        assert!(expand_env("%WC_NOPE_123%/x").is_none());
        assert!(expand_env("100%").is_none());
    }
}
