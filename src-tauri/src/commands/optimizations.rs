//! Tipos compartilhados e utilitários de backup para tweaks de otimização.
//!
//! Os tweaks individuais foram migrados para submódulos em `commands/tweaks/`.
//! Este módulo mantém apenas os tipos públicos (`TweakInfo`, `RiskLevel`, etc.)
//! e funções utilitárias usadas por múltiplos módulos de tweaks.

use serde::{Deserialize, Serialize};
use serde_json::Value;

use crate::utils::backup::{get_all_backups, BackupStatus};
use crate::utils::command_runner::run_powershell;
use crate::utils::registry::{delete_value, key_exists, write_dword, write_string, Hive};

// ─── Tipos compartilhados ─────────────────────────────────────────────────────

/// Nível de risco associado à aplicação de um tweak.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum RiskLevel {
    /// Alteração cosmética ou de preferência; facilmente reversível sem impacto sistêmico
    Low,
    /// Modifica comportamento do sistema; reversível, mas pode exigir reinicialização
    Medium,
    /// Impacto sistêmico significativo; pode afetar estabilidade ou compatibilidade
    High,
}

/// Nível de evidência técnica/científica que sustenta o benefício de um tweak.
///
/// Permite ao usuário avaliar o grau de confiança antes de aplicar uma otimização,
/// diferenciando tweaks comprovados em benchmarks de sugestões populares sem base formal.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum EvidenceLevel {
    /// Benefício confirmado por benchmarks documentados e/ou documentação oficial
    Proven,
    /// Raciocínio técnico sólido e mecanismo bem compreendido, mas sem benchmarks
    /// rigorosos publicados que quantifiquem o ganho de forma reproduzível
    Plausible,
    /// Amplamente compartilhado na comunidade gamer, porém sem evidência formal
    /// — resultados variam ou são negligenciáveis em benchmarks independentes
    Unproven,
}

/// Filtro de hardware para tweaks vendor-specific.
///
/// Quando `None` no `TweakInfo`, o tweak é universal (roda em qualquer hardware).
/// Quando `Some(...)`, apenas o hardware com vendor correspondente deve exibir/executar.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct HardwareFilter {
    /// Vendor de GPU requerido: `"nvidia"`, `"amd"` ou `"intel"`
    #[serde(skip_serializing_if = "Option::is_none")]
    pub gpu_vendor: Option<String>,
    /// Vendor de CPU requerido: `"intel"` ou `"amd"`
    #[serde(skip_serializing_if = "Option::is_none")]
    pub cpu_vendor: Option<String>,
}

/// Informações completas de um tweak para exibição na UI.
///
/// Combina metadados estáticos (nome, descrição, risco) com o estado dinâmico
/// atual do sistema (is_applied, has_backup, last_applied). Enviado ao frontend
/// como resposta aos comandos `get_X_info`.
#[derive(Debug, Serialize)]
pub struct TweakInfo {
    /// Identificador único em snake_case (ex: `"disable_wallpaper_compression"`)
    pub id: String,
    /// Nome legível exibido na UI (ex: `"Desabilitar Compressão de Wallpaper"`)
    pub name: String,
    /// Descrição detalhada do efeito para o usuário final
    pub description: String,
    /// Categoria para agrupamento na UI (ex: `"optimization"`, `"cleanup"`)
    pub category: String,
    /// `true` se o tweak está atualmente ativo no sistema
    pub is_applied: bool,
    /// `true` se a mudança só tem efeito após reinicialização do Windows
    pub requires_restart: bool,
    /// Timestamp ISO 8601 UTC da última aplicação; `null` se nunca aplicado com backup
    pub last_applied: Option<String>,
    /// `true` se existe backup com status `Applied` disponível para reversão
    pub has_backup: bool,
    /// Nível de risco do tweak
    pub risk_level: RiskLevel,
    /// Grau de evidência técnica que sustenta o benefício declarado do tweak
    pub evidence_level: EvidenceLevel,
    /// Descrição do valor padrão do Windows para exibição no botão "Restaurar Padrão"
    pub default_value_description: String,
    /// Filtro de hardware: `None` = universal, `Some(...)` = vendor-specific.
    /// Mantido em sincronia com `get_tweak_hardware_filter()` em `plans.rs`.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub hardware_filter: Option<HardwareFilter>,
}

// ─── Utilitários internos ───────────────────────────────────────────────────

/// Consulta `backups.json` e extrai `(has_backup, last_applied)` para um tweak.
///
/// - `has_backup`: `true` quando há entrada com status `Applied` (backup utilizável)
/// - `last_applied`: timestamp `backed_up_at` quando aplicado; `None` caso contrário
pub fn backup_info(tweak_id: &str) -> (bool, Option<String>) {
    match get_all_backups() {
        Ok(backups) => match backups.get(tweak_id) {
            Some(entry) if entry.status == BackupStatus::Applied => {
                (true, Some(entry.backed_up_at.clone()))
            }
            _ => (false, None),
        },
        Err(_) => (false, None),
    }
}

/// Restaura múltiplas entradas de registro (e opcionalmente um serviço Windows) a partir
/// de um array JSON armazenado no campo `value` de um backup `MULTI_DWORD`.
///
/// Cada elemento do array deve ter o formato:
/// - Registro: `{"hive":"HKCU"|"HKLM", "path":"...", "key":"...", "value":null|number}`
/// - Serviço:  `{"type":"service", "name":"...", "value":"Automatic"|"Manual"|"Disabled"|null}`
///
/// Para entradas de registro, `"value": null` significa que a chave não existia antes
/// do tweak — nesse caso ela é deletada para restaurar o padrão do Windows.
pub(crate) fn restore_multi_entries(entries: &[Value]) -> Result<(), String> {
    for entry in entries {
        let entry_type = entry
            .get("type")
            .and_then(|v| v.as_str())
            .unwrap_or("registry");

        match entry_type {
            "service" => {
                let name = entry["name"].as_str().unwrap_or("");
                // null = serviço não existia antes; não há nada a restaurar
                if let Some(start_type) = entry["value"].as_str() {
                    let script = format!(
                        "Set-Service -Name '{}' -StartupType {} -ErrorAction SilentlyContinue",
                        name, start_type
                    );
                    run_powershell(&script)?;
                }
            }
            _ => {
                let hive_str = entry["hive"].as_str().unwrap_or("HKCU");
                let path = entry["path"].as_str().unwrap_or("");
                let key = entry["key"].as_str().unwrap_or("");
                let hive = if hive_str == "HKLM" {
                    Hive::LocalMachine
                } else {
                    Hive::CurrentUser
                };

                match &entry["value"] {
                    Value::Null => {
                        if key_exists(hive, path, key)? {
                            delete_value(hive, path, key)?;
                        }
                    }
                    Value::Number(n) => {
                        write_dword(hive, path, key, n.as_u64().unwrap_or(0) as u32)?;
                    }
                    // REG_SZ: usado por tweaks como aceleração do mouse (MouseSpeed, etc.)
                    Value::String(s) => {
                        write_string(hive, path, key, s)?;
                    }
                    other => {
                        return Err(format!(
                            "Tipo inesperado no backup multi-entry (key={}): {:?}",
                            key, other
                        ));
                    }
                }
            }
        }
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn backup_info_returns_false_for_nonexistent() {
        // backup_info deve retornar (false, None) para tweak que não existe no backup
        let (has, ts) = backup_info("tweak_that_never_existed_12345");
        assert!(!has);
        assert!(ts.is_none());
    }
}
