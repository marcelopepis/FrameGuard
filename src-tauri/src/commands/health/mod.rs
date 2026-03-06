//! Comandos Tauri para verificação e manutenção da saúde do sistema.
//!
//! Diferente dos tweaks de otimização (que alteram configurações persistentes),
//! estas ações são operações pontuais de verificação e limpeza: cada uma executa
//! um processo externo ou realiza operações de I/O com streaming de output em
//! tempo real para o frontend.
//!
//! Submódulos:
//! - `dism` — DISM Cleanup, CheckHealth, ScanHealth, RestoreHealth
//! - `disk` — SFC, chkdsk, SSD TRIM
//! - `maintenance` — Flush DNS, limpeza de temporários, kill process

pub mod disk;
pub mod dism;
pub mod maintenance;

pub use disk::*;
pub use dism::*;
pub use maintenance::*;

use chrono::Utc;
use serde::{Deserialize, Serialize};
use std::path::Path;
use tauri::Emitter;

use crate::utils::command_runner::CommandEvent;
use crate::utils::file_locks;

// ─── Tipos públicos ───────────────────────────────────────────────────────────

/// Status final de uma ação de saúde do sistema.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum CheckStatus {
    /// Operação concluída sem erros ou problemas detectados
    Success,
    /// Operação concluída com alertas (ex: corrupção reparada, agendamento para boot)
    Warning,
    /// Operação falhou ou encontrou erros irrecuperáveis
    Error,
}

/// Informação sobre um processo que está travando arquivos durante a limpeza.
#[derive(Debug, Clone, Serialize)]
pub struct LockingProcessInfo {
    /// PID do processo
    pub pid: u32,
    /// Nome do processo (ex: "chrome", "explorer")
    pub name: String,
    /// Quantidade de arquivos travados por este processo
    pub file_count: usize,
}

/// Resultado completo de uma ação de saúde, retornado ao frontend.
#[derive(Debug, Serialize)]
pub struct HealthCheckResult {
    /// Identificador da ação em snake_case (ex: `"dism_checkhealth"`)
    pub id: String,
    /// Nome legível exibido na UI (ex: `"DISM CheckHealth"`)
    pub name: String,
    /// Status final da ação
    pub status: CheckStatus,
    /// Mensagem resumida do resultado para exibição no card da UI
    pub message: String,
    /// Output completo do comando para exibição no painel de detalhes/log
    pub details: String,
    /// Tempo total de execução em segundos
    pub duration_seconds: u64,
    /// Espaço liberado em MB; `null` para ações que não liberam espaço em disco
    pub space_freed_mb: Option<u64>,
    /// Timestamp ISO 8601 UTC do momento de execução
    pub timestamp: String,
    /// Processos que estão travando arquivos (apenas para temp_cleanup).
    /// Agrupados por processo com contagem de arquivos.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub locking_processes: Option<Vec<LockingProcessInfo>>,
}

// ─── Helpers internos (pub(crate) para uso nos submódulos) ──────────────────

/// Retorna o instante atual em ISO 8601 UTC.
pub(crate) fn now_utc() -> String {
    Utc::now().format("%Y-%m-%dT%H:%M:%SZ").to_string()
}

/// Converte bytes para megabytes (truncado).
pub(crate) fn bytes_to_mb(bytes: u64) -> u64 {
    bytes / (1024 * 1024)
}

/// Emite um evento de progresso para o frontend usando o mesmo formato de `CommandEvent`.
/// Falhas de emissão são silenciadas para não interromper a operação em andamento.
pub(crate) fn emit_health_event(
    app: &tauri::AppHandle,
    event_name: &str,
    event_type: &str,
    data: &str,
) {
    let payload = CommandEvent {
        event_type: event_type.to_string(),
        data: data.to_string(),
        timestamp: now_utc(),
    };
    let _ = app.emit(event_name, payload);
}

/// Calcula recursivamente o tamanho total em bytes de um arquivo ou diretório.
/// Retorna 0 para caminhos inacessíveis, inexistentes ou links simbólicos circulares.
pub(crate) fn dir_size_bytes(path: &Path) -> u64 {
    if path.is_file() {
        return std::fs::metadata(path).map(|m| m.len()).unwrap_or(0);
    }
    let Ok(entries) = std::fs::read_dir(path) else {
        return 0;
    };
    entries.flatten().map(|e| dir_size_bytes(&e.path())).sum()
}

/// Resultado da tentativa de remoção de um diretório — erros + caminhos travados.
pub(crate) struct DeleteResult {
    /// Mensagens de erro formatadas para exibição no log.
    pub errors: Vec<String>,
    /// Caminhos completos dos arquivos que falharam por estarem em uso (os error 5 ou 32).
    pub locked_paths: Vec<String>,
}

/// Remove todos os itens dentro de `path` sem remover o diretório raiz em si.
///
/// Continua após erros individuais (arquivos em uso, sem permissão) para maximizar
/// a limpeza. Acumula os bytes efetivamente liberados em `freed`.
/// Para arquivos travados (os error 5/32), detecta qual processo está usando via
/// Restart Manager API e inclui a informação na mensagem de erro.
pub(crate) fn delete_dir_contents(path: &Path, freed: &mut u64) -> DeleteResult {
    let mut errors: Vec<String> = Vec::new();
    let mut locked_paths: Vec<String> = Vec::new();

    if !path.exists() {
        return DeleteResult {
            errors,
            locked_paths,
        };
    }

    let entries = match std::fs::read_dir(path) {
        Ok(e) => e,
        Err(e) => {
            errors.push(format!("Erro ao abrir {}: {}", path.display(), e));
            return DeleteResult {
                errors,
                locked_paths,
            };
        }
    };

    for entry in entries.flatten() {
        let entry_path = entry.path();

        // Calcula o tamanho antes da remoção para contabilizar corretamente
        let size = dir_size_bytes(&entry_path);

        let result = if entry_path.is_dir() {
            std::fs::remove_dir_all(&entry_path)
        } else {
            std::fs::remove_file(&entry_path)
        };

        match result {
            Ok(()) => *freed += size,
            // Arquivo em uso ou sem permissão — detecta processo e continua
            Err(e) => {
                let raw_err = e.raw_os_error();
                let is_locked = raw_err == Some(32) || raw_err == Some(5);

                // Tenta identificar o processo que trava o arquivo
                let lock_info = if is_locked && entry_path.is_file() {
                    let full_path = entry_path.to_string_lossy().to_string();
                    let procs = file_locks::get_locking_processes(&full_path);
                    if !procs.is_empty() {
                        locked_paths.push(full_path);
                        let names: Vec<String> = procs
                            .iter()
                            .map(|p| format!("{} (PID {})", p.name, p.pid))
                            .collect();
                        format!(" [travado por: {}]", names.join(", "))
                    } else {
                        if is_locked {
                            locked_paths.push(entry_path.to_string_lossy().to_string());
                        }
                        String::new()
                    }
                } else {
                    if is_locked {
                        locked_paths.push(entry_path.to_string_lossy().to_string());
                    }
                    String::new()
                };

                errors.push(format!(
                    "{}: {}{}",
                    entry_path.file_name().unwrap_or_default().to_string_lossy(),
                    e,
                    lock_info
                ));
            }
        }
    }

    DeleteResult {
        errors,
        locked_paths,
    }
}

/// Agrupa processos que travam arquivos por PID, contando quantos arquivos cada um trava.
/// Usa a Restart Manager API para re-consultar os caminhos e obter nomes de processo.
pub(crate) fn aggregate_locking_processes(locked_paths: &[String]) -> Vec<LockingProcessInfo> {
    use std::collections::HashMap;

    let mut proc_map: HashMap<u32, (String, usize)> = HashMap::new();

    for path in locked_paths {
        let procs = file_locks::get_locking_processes(path);
        for p in procs {
            let entry = proc_map.entry(p.pid).or_insert_with(|| (p.name.clone(), 0));
            entry.1 += 1;
        }
    }

    let mut result: Vec<LockingProcessInfo> = proc_map
        .into_iter()
        .map(|(pid, (name, count))| LockingProcessInfo {
            pid,
            name,
            file_count: count,
        })
        .collect();

    // Ordena por quantidade de arquivos travados (mais primeiro)
    result.sort_by(|a, b| b.file_count.cmp(&a.file_count));
    result
}

/// Monta os argumentos para executar `dism_args` via PowerShell com
/// `[Console]::OutputEncoding = UTF8`, garantindo que PT-BR saia em UTF-8.
///
/// Retorna `(command, args_owned, display_label)` para passar a
/// `run_command_with_progress`.
pub(crate) fn ps_utf8_dism(dism_args: &str) -> (String, Vec<String>, String) {
    let script = format!(
        "[Console]::OutputEncoding = [System.Text.Encoding]::UTF8; dism.exe {}",
        dism_args
    );
    let args = vec![
        "-NoProfile".to_string(),
        "-NonInteractive".to_string(),
        "-ExecutionPolicy".to_string(),
        "Bypass".to_string(),
        "-Command".to_string(),
        script,
    ];
    let label = format!("dism.exe {}", dism_args);
    ("powershell.exe".to_string(), args, label)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn zero_bytes() {
        assert_eq!(bytes_to_mb(0), 0);
    }

    #[test]
    fn exactly_one_mb() {
        assert_eq!(bytes_to_mb(1024 * 1024), 1);
    }

    #[test]
    fn truncates_fractional_mb() {
        assert_eq!(bytes_to_mb(1_572_864), 1);
    }

    #[test]
    fn large_value() {
        assert_eq!(bytes_to_mb(10 * 1024 * 1024 * 1024), 10240);
    }

    #[test]
    fn nonexistent_path_returns_zero() {
        let path = Path::new(r"C:\FrameGuard_test_nonexistent_12345");
        assert_eq!(dir_size_bytes(path), 0);
    }

    fn parse_dism_checkhealth(stdout: &str, success: bool) -> CheckStatus {
        let stdout_lower = stdout.to_lowercase();
        if !success {
            CheckStatus::Error
        } else if stdout_lower.contains("no component store corruption detected")
            || stdout_lower.contains("não está danificado")
        {
            CheckStatus::Success
        } else if stdout_lower.contains("repairable") || stdout_lower.contains("reparável") {
            CheckStatus::Warning
        } else if stdout_lower.contains("corrupted")
            || stdout_lower.contains("corrompido")
            || stdout_lower.contains("danificado")
        {
            CheckStatus::Error
        } else {
            CheckStatus::Warning
        }
    }

    #[test]
    fn dism_ok_english() {
        assert!(matches!(
            parse_dism_checkhealth("No component store corruption detected.", true),
            CheckStatus::Success
        ));
    }

    #[test]
    fn dism_ok_ptbr() {
        assert!(matches!(
            parse_dism_checkhealth("O repositório de componentes não está danificado.", true),
            CheckStatus::Success
        ));
    }

    #[test]
    fn dism_repairable_english() {
        assert!(matches!(
            parse_dism_checkhealth("The component store is repairable.", true),
            CheckStatus::Warning
        ));
    }

    #[test]
    fn dism_repairable_ptbr() {
        assert!(matches!(
            parse_dism_checkhealth("O repositório de componentes é reparável.", true),
            CheckStatus::Warning
        ));
    }

    #[test]
    fn dism_corrupted_english() {
        assert!(matches!(
            parse_dism_checkhealth("The component store is corrupted.", true),
            CheckStatus::Error
        ));
    }

    #[test]
    fn dism_corrupted_ptbr() {
        assert!(matches!(
            parse_dism_checkhealth("O repositório de componentes está corrompido.", true),
            CheckStatus::Error
        ));
    }

    #[test]
    fn dism_failure_exit_code() {
        assert!(matches!(
            parse_dism_checkhealth("", false),
            CheckStatus::Error
        ));
    }

    #[test]
    fn dism_unknown_output() {
        assert!(matches!(
            parse_dism_checkhealth("Some unexpected output from DISM", true),
            CheckStatus::Warning
        ));
    }
}
