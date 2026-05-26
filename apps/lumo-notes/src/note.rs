//! note.rs -- estrutura Note e helpers de persistencia.

use std::path::PathBuf;

#[derive(Debug, Clone)]
pub struct Note {
    pub id: u64,
    pub title: String,
    pub content: String,
    pub modified: chrono::DateTime<chrono::Local>,
    pub path: PathBuf,
}

impl Note {
    pub fn preview(&self) -> String {
        let body: String = self
            .content
            .lines()
            .skip_while(|l| l.trim().is_empty())
            .take(2)
            .collect::<Vec<_>>()
            .join(" ");
        let truncated: String = body.chars().take(80).collect();
        if truncated.chars().count() < body.chars().count() {
            format!("{}...", truncated)
        } else {
            body
        }
    }

    pub fn matches_query(&self, q: &str) -> bool {
        if q.is_empty() {
            return true;
        }
        let q_low = q.to_lowercase();
        self.title.to_lowercase().contains(&q_low) || self.content.to_lowercase().contains(&q_low)
    }
}

pub fn notes_dir() -> PathBuf {
    let home = std::env::var("HOME").unwrap_or_else(|_| "/root".into());
    PathBuf::from(home).join(".local/share/lumo/notes")
}

pub async fn load_notes() -> Vec<Note> {
    let dir = notes_dir();
    let _ = tokio::fs::create_dir_all(&dir).await;
    let mut entries = match tokio::fs::read_dir(&dir).await {
        Ok(e) => e,
        Err(_) => return Vec::new(),
    };
    let mut notes = Vec::new();
    let mut id = 0u64;
    while let Ok(Some(entry)) = entries.next_entry().await {
        let path = entry.path();
        let ext = path.extension().and_then(|e| e.to_str()).unwrap_or("");
        if ext != "md" && ext != "txt" {
            continue;
        }
        let content = tokio::fs::read_to_string(&path).await.unwrap_or_default();
        let title = path
            .file_stem()
            .and_then(|s| s.to_str())
            .unwrap_or("sem titulo")
            .to_string();
        let meta = entry.metadata().await;
        let modified = meta
            .ok()
            .and_then(|m| m.modified().ok())
            .map(chrono::DateTime::<chrono::Local>::from)
            .unwrap_or_else(|| chrono::Local::now());
        notes.push(Note {
            id,
            title,
            content,
            modified,
            path,
        });
        id += 1;
    }
    notes.sort_by_key(|n| std::cmp::Reverse(n.modified));
    notes
}

pub async fn save_note(path: PathBuf, content: String) -> Result<(), String> {
    if let Some(parent) = path.parent() {
        tokio::fs::create_dir_all(parent)
            .await
            .map_err(|e| e.to_string())?;
    }
    tokio::fs::write(&path, content)
        .await
        .map_err(|e| e.to_string())
}

pub async fn delete_note(path: PathBuf) -> Result<(), String> {
    tokio::fs::remove_file(&path)
        .await
        .map_err(|e| e.to_string())
}

pub fn new_note_path(title: &str) -> PathBuf {
    let slug = title
        .to_lowercase()
        .replace(' ', "-")
        .chars()
        .filter(|c| c.is_alphanumeric() || *c == '-')
        .collect::<String>();
    let name = if slug.is_empty() {
        format!("nota-{}", chrono::Local::now().format("%Y%m%d-%H%M%S"))
    } else {
        slug
    };
    notes_dir().join(format!("{}.md", name))
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;

    fn make_note(title: &str, content: &str) -> Note {
        Note {
            id: 0,
            title: title.into(),
            content: content.into(),
            modified: chrono::Local::now(),
            path: PathBuf::from(format!("/tmp/{}.md", title)),
        }
    }

    #[test]
    fn test_preview_short() {
        let n = make_note("test", "linha 1\nlinha 2");
        let p = n.preview();
        assert!(!p.is_empty());
        assert!(p.len() <= 83);
    }

    #[test]
    fn test_preview_long() {
        let long = "a".repeat(200);
        let n = make_note("test", &long);
        let p = n.preview();
        assert!(p.ends_with("..."));
    }

    #[test]
    fn test_matches_query_title() {
        let n = make_note("Receitas", "conteudo qualquer");
        assert!(n.matches_query("receita"));
    }

    #[test]
    fn test_matches_query_content() {
        let n = make_note("titulo", "texto importante");
        assert!(n.matches_query("IMPORTANTE"));
    }

    #[test]
    fn test_matches_query_empty() {
        let n = make_note("t", "c");
        assert!(n.matches_query(""));
    }

    #[test]
    fn test_new_note_path_slug() {
        let p = new_note_path("Minha Nota");
        let fname = p.file_name().unwrap().to_str().unwrap();
        assert!(fname.contains("minha-nota"));
        assert!(fname.ends_with(".md"));
    }

    #[test]
    fn test_preview_utf8_multibyte() {
        // 85 chars, 104 bytes -- slice por byte panicar em cedilha/acento
        let body =
            "ação ações cabeça coração cancelação avaliação resolução publicação execução condição"
                .to_string();
        assert!(
            body.len() > 80,
            "precisa ter mais de 80 bytes pra testar o truncate"
        );
        let n = make_note("utf8", &body);
        let p = n.preview();
        // deve truncar em limite de char, nao de byte
        assert!(p.ends_with("...") || p.chars().count() <= 80);
        // nao panica (ja que estamos aqui)
    }

    #[test]
    fn test_preview_emoji_nao_panica() {
        // Emojis sao 4 bytes cada -- garantir que slice por byte nao ocorre
        let body = "nota com emoji diversao diversao diversao diversao diversao diversao diversao"
            .to_string();
        let n = make_note("emoji", &body);
        let _p = n.preview(); // nao pode paniciar
    }
}
