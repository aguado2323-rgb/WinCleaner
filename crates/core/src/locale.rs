//! Idioma del equipo, para elegir el de la interfaz: español si Windows está en español
//! (de cualquier país), inglés en cualquier otro caso.

/// `"es"` o `"en"` según el idioma de la interfaz de Windows.
pub fn system_language() -> &'static str {
    if imp::is_spanish() {
        "es"
    } else {
        "en"
    }
}

#[cfg(windows)]
mod imp {
    #[link(name = "kernel32")]
    extern "system" {
        fn GetUserDefaultUILanguage() -> u16;
    }

    /// Los 10 bits bajos del LANGID son el idioma principal; 0x0A es español (es-ES, es-MX…).
    pub fn is_spanish() -> bool {
        // SAFETY: función sin argumentos que solo devuelve un número.
        let id = unsafe { GetUserDefaultUILanguage() };
        super::primary_is_spanish(id)
    }
}

#[cfg(not(windows))]
mod imp {
    pub fn is_spanish() -> bool {
        ["LC_ALL", "LC_MESSAGES", "LANG"]
            .iter()
            .find_map(|v| std::env::var(v).ok().filter(|s| !s.is_empty()))
            .is_some_and(|s| s.to_lowercase().starts_with("es"))
    }
}

#[cfg_attr(not(windows), allow(dead_code))]
fn primary_is_spanish(langid: u16) -> bool {
    langid & 0x3ff == 0x0a
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn spanish_langids_of_any_country() {
        assert!(primary_is_spanish(0x0c0a)); // es-ES
        assert!(primary_is_spanish(0x080a)); // es-MX
        assert!(!primary_is_spanish(0x0409)); // en-US
        assert!(!primary_is_spanish(0x0416)); // pt-BR
        assert!(["es", "en"].contains(&system_language()));
    }
}
