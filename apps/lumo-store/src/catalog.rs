//! catalog.rs -- tipos e parsing do catalogo TOML do Lumo Store.

use serde::{Deserialize, Serialize};

/// Entrada de aplicativo no catalogo.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct AppEntry {
    pub name:        String,
    pub pkg:         String,
    pub icon:        String,
    pub category:    String,
    pub description: String,
}

/// Catalogo completo (array de [[app]] em TOML).
#[derive(Debug, Deserialize)]
pub struct Catalog {
    #[serde(rename = "app")]
    pub apps: Vec<AppEntry>,
}

impl Catalog {
    /// Carrega catalogo embutido em tempo de compilacao.
    pub fn embedded() -> Self {
        let raw = include_str!("../catalog.toml");
        toml::from_str(raw).expect("catalog.toml invalido")
    }

    /// Filtra por categoria. Case-insensitive.
    pub fn by_category<'a>(&'a self, cat: &str) -> impl Iterator<Item = &'a AppEntry> {
        let cat = cat.to_ascii_lowercase();
        self.apps.iter().filter(move |a| a.category.to_ascii_lowercase() == cat)
    }

    /// Busca por nome ou descricao. Case-insensitive.
    pub fn search<'a>(&'a self, query: &str) -> impl Iterator<Item = &'a AppEntry> {
        let q = query.to_ascii_lowercase();
        self.apps.iter().filter(move |a| {
            a.name.to_ascii_lowercase().contains(&q)
                || a.description.to_ascii_lowercase().contains(&q)
        })
    }

    /// Lista de categorias unicas, ordenadas.
    pub fn categories(&self) -> Vec<&str> {
        let mut cats: Vec<&str> = self.apps.iter().map(|a| a.category.as_str()).collect();
        cats.sort_unstable();
        cats.dedup();
        cats
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn catalog_loads_embedded() {
        let cat = Catalog::embedded();
        assert!(!cat.apps.is_empty(), "catalogo nao pode ser vazio");
    }

    #[test]
    fn catalog_has_expected_apps() {
        let cat = Catalog::embedded();
        let names: Vec<&str> = cat.apps.iter().map(|a| a.name.as_str()).collect();
        assert!(names.contains(&"Firefox"));
        assert!(names.contains(&"LibreOffice"));
        assert!(names.contains(&"VS Code"));
        assert!(names.contains(&"Krita"));
    }

    #[test]
    fn filter_by_category() {
        let cat = Catalog::embedded();
        let arte: Vec<_> = cat.by_category("Arte").collect();
        assert!(!arte.is_empty());
        for a in arte {
            assert_eq!(a.category, "Arte");
        }
    }

    #[test]
    fn search_case_insensitive() {
        let cat = Catalog::embedded();
        let res: Vec<_> = cat.search("firefox").collect();
        assert!(!res.is_empty());
        assert_eq!(res[0].pkg, "firefox");
    }

    #[test]
    fn categories_unique_sorted() {
        let cat = Catalog::embedded();
        let cats = cat.categories();
        let mut sorted = cats.clone();
        sorted.sort();
        sorted.dedup();
        assert_eq!(cats, sorted);
    }

    #[test]
    fn all_entries_have_pkg() {
        let cat = Catalog::embedded();
        for app in &cat.apps {
            assert!(!app.pkg.is_empty(), "pkg vazio: {}", app.name);
        }
    }
}
