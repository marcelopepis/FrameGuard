// Comando para verificar atualizações via GitHub Releases API.

use serde::Serialize;

const CURRENT_VERSION: &str = env!("CARGO_PKG_VERSION");
const GITHUB_REPO: &str = "marcelopepis/FrameGuard";

#[derive(Debug, Serialize)]
pub struct UpdateCheckResult {
    pub current_version: String,
    pub latest_version: String,
    pub is_update_available: bool,
    pub release_url: String,
    pub release_notes: String,
}

#[tauri::command]
pub async fn check_for_updates() -> Result<UpdateCheckResult, String> {
    tokio::task::spawn_blocking(|| {
        let url = format!(
            "https://api.github.com/repos/{}/releases/latest",
            GITHUB_REPO
        );

        let resp = reqwest::blocking::Client::new()
            .get(&url)
            .header("User-Agent", "FrameGuard")
            .header("Accept", "application/vnd.github.v3+json")
            .send()
            .map_err(|e| format!("Erro ao conectar ao GitHub: {}", e))?;

        if resp.status() == reqwest::StatusCode::NOT_FOUND {
            return Ok(UpdateCheckResult {
                current_version: CURRENT_VERSION.to_string(),
                latest_version: CURRENT_VERSION.to_string(),
                is_update_available: false,
                release_url: format!("https://github.com/{}", GITHUB_REPO),
                release_notes: "Nenhuma release publicada ainda.".to_string(),
            });
        }

        if !resp.status().is_success() {
            return Err(format!("GitHub API retornou status {}", resp.status()));
        }

        let json: serde_json::Value = resp
            .json()
            .map_err(|e| format!("Erro ao processar resposta: {}", e))?;

        let tag = json["tag_name"]
            .as_str()
            .unwrap_or("")
            .trim_start_matches('v')
            .to_string();

        let release_url = json["html_url"]
            .as_str()
            .unwrap_or(&format!("https://github.com/{}/releases", GITHUB_REPO))
            .to_string();

        let release_notes = json["body"]
            .as_str()
            .unwrap_or("")
            .to_string();

        let is_update_available = version_is_newer(&tag, CURRENT_VERSION);

        Ok(UpdateCheckResult {
            current_version: CURRENT_VERSION.to_string(),
            latest_version: if tag.is_empty() { CURRENT_VERSION.to_string() } else { tag },
            is_update_available,
            release_url,
            release_notes,
        })
    })
    .await
    .map_err(|e| format!("Erro interno: {}", e))?
}

/// Compara versões semver simples (major.minor.patch).
fn version_is_newer(latest: &str, current: &str) -> bool {
    let parse = |v: &str| -> Vec<u32> {
        v.split('.')
            .filter_map(|s| s.parse::<u32>().ok())
            .collect()
    };
    let l = parse(latest);
    let c = parse(current);
    for i in 0..3 {
        let lv = l.get(i).copied().unwrap_or(0);
        let cv = c.get(i).copied().unwrap_or(0);
        if lv > cv {
            return true;
        }
        if lv < cv {
            return false;
        }
    }
    false
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn newer_major() {
        assert!(version_is_newer("2.0.0", "1.0.0"));
    }

    #[test]
    fn newer_minor() {
        assert!(version_is_newer("1.1.0", "1.0.0"));
    }

    #[test]
    fn newer_patch() {
        assert!(version_is_newer("1.0.1", "1.0.0"));
    }

    #[test]
    fn same_version() {
        assert!(!version_is_newer("1.0.0", "1.0.0"));
    }

    #[test]
    fn older_version() {
        assert!(!version_is_newer("0.9.0", "1.0.0"));
    }

    #[test]
    fn partial_version_strings() {
        assert!(version_is_newer("2", "1.9.9"));
        assert!(!version_is_newer("1", "1.0.0"));
    }

    #[test]
    fn empty_strings() {
        assert!(!version_is_newer("", ""));
        assert!(!version_is_newer("", "1.0.0"));
    }

    #[test]
    fn complex_comparison() {
        assert!(version_is_newer("0.2.0", "0.1.9"));
        assert!(!version_is_newer("0.1.9", "0.2.0"));
    }
}
