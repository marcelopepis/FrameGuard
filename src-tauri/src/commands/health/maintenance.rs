//! Comandos de manutenção geral do sistema.
//!
//! Três operações:
//! - **Flush DNS** — limpa o cache DNS via `ipconfig /flushdns`
//! - **Temp Cleanup** — remove arquivos temporários de %TEMP%, Windows\Temp
//!   e SoftwareDistribution\Download
//! - **Kill Process** — encerra processos que travam arquivos (com deny-list de segurança)

use std::os::windows::process::CommandExt;
use std::path::Path;
use std::process::Command;
use std::time::Instant;

use crate::utils::command_runner::run_command_with_progress;

use super::{
    aggregate_locking_processes, bytes_to_mb, delete_dir_contents, dir_size_bytes,
    emit_health_event, now_utc, CheckStatus, HealthCheckResult,
};

/// Suprime janela de console ao lançar subprocessos.
const CREATE_NO_WINDOW: u32 = 0x0800_0000;

// ═══════════════════════════════════════════════════════════════════════════════
// Flush DNS
// ═══════════════════════════════════════════════════════════════════════════════

/// Limpa o cache DNS do sistema via ipconfig /flushdns.
#[tauri::command]
pub async fn flush_dns(app_handle: tauri::AppHandle) -> Result<HealthCheckResult, String> {
    tokio::task::spawn_blocking(move || {
        let start = Instant::now();
        let ts = now_utc();

        let result = run_command_with_progress(
            &app_handle,
            "dns_flush_progress",
            "ipconfig.exe",
            &["/flushdns"],
            None,
        )?;

        let duration = start.elapsed().as_secs();
        let stdout_lower = result.stdout.to_lowercase();

        let (status, message) = if stdout_lower.contains("successfully flushed")
            || stdout_lower.contains("liberado com êxito")
            || stdout_lower.contains("liberado com exito")
            || result.success
        {
            (
                CheckStatus::Success,
                "Cache DNS limpo com sucesso. Novas consultas usarão endereços atualizados."
                    .to_string(),
            )
        } else {
            (
                CheckStatus::Error,
                "Falha ao limpar o cache DNS. Verifique se o serviço DNS Client está ativo."
                    .to_string(),
            )
        };

        Ok(HealthCheckResult {
            id: "flush_dns".to_string(),
            name: "Flush DNS".to_string(),
            status,
            message,
            details: result.stdout,
            duration_seconds: duration,
            space_freed_mb: None,
            timestamp: ts,
            locking_processes: None,
        })
    })
    .await
    .unwrap_or_else(|e| Err(e.to_string()))
}

// ═══════════════════════════════════════════════════════════════════════════════
// Limpeza de Arquivos Temporários
// ═══════════════════════════════════════════════════════════════════════════════

/// Remove arquivos temporários das pastas %TEMP%, Windows\Temp e
/// Windows\SoftwareDistribution\Download, calculando o espaço total liberado.
#[tauri::command]
pub async fn run_temp_cleanup(app_handle: tauri::AppHandle) -> Result<HealthCheckResult, String> {
    tokio::task::spawn_blocking(move || {
        let start = Instant::now();
        let ts = now_utc();

        // Define as pastas alvo com descrição legível para os eventos de progresso
        let targets: &[(&str, &str)] = &[
            ("%TEMP%", "Temp do usuário"),
            (r"C:\Windows\Temp", "Windows Temp"),
            (
                r"C:\Windows\SoftwareDistribution\Download",
                "Cache do Windows Update",
            ),
        ];

        let mut total_freed: u64 = 0;
        let mut all_errors: Vec<String> = Vec::new();
        let mut all_locked_paths: Vec<String> = Vec::new();
        let mut log_lines: Vec<String> = Vec::new();

        emit_health_event(
            &app_handle,
            "temp_cleanup_progress",
            "started",
            "Iniciando limpeza de arquivos temporários",
        );

        for (folder_template, description) in targets {
            // Expande variáveis de ambiente (ex: %TEMP% → C:\Users\...\AppData\Local\Temp)
            let folder_path = if folder_template.starts_with('%') {
                let var_name = folder_template.trim_matches('%');
                match std::env::var(var_name) {
                    Ok(p) => p,
                    Err(_) => {
                        let msg = format!(
                            "Variável de ambiente {} não encontrada — pulando",
                            folder_template
                        );
                        emit_health_event(
                            &app_handle,
                            "temp_cleanup_progress",
                            "stderr",
                            &msg,
                        );
                        log_lines.push(msg);
                        continue;
                    }
                }
            } else {
                folder_template.to_string()
            };

            let path = Path::new(&folder_path);

            if !path.exists() {
                let msg = format!("{}: pasta não existe — pulando", description);
                emit_health_event(&app_handle, "temp_cleanup_progress", "stdout", &msg);
                log_lines.push(msg);
                continue;
            }

            // Calcula tamanho antes da remoção para relatório preciso
            let size_before = dir_size_bytes(path);
            let msg = format!(
                "Limpando {} ({}) — {:.1} MB",
                description,
                folder_path,
                size_before as f64 / (1024.0 * 1024.0)
            );
            emit_health_event(&app_handle, "temp_cleanup_progress", "stdout", &msg);
            log_lines.push(msg.clone());

            let mut freed_this_folder: u64 = 0;
            let del_result = delete_dir_contents(path, &mut freed_this_folder);

            total_freed += freed_this_folder;

            let result_msg = format!(
                "  → Liberados {:.1} MB de {}",
                freed_this_folder as f64 / (1024.0 * 1024.0),
                description
            );
            emit_health_event(&app_handle, "temp_cleanup_progress", "stdout", &result_msg);
            log_lines.push(result_msg);

            // Reporta erros (arquivos em uso) como informativos, não como falha
            for err in &del_result.errors {
                let err_msg = format!("  [ignorado] {}", err);
                emit_health_event(&app_handle, "temp_cleanup_progress", "stderr", &err_msg);
                log_lines.push(err_msg);
            }
            all_errors.extend(del_result.errors);
            all_locked_paths.extend(del_result.locked_paths);
        }

        let duration = start.elapsed().as_secs();
        let freed_mb = bytes_to_mb(total_freed);

        // Agrega processos que travam arquivos (agrupados por PID)
        let locking_procs = aggregate_locking_processes(&all_locked_paths);

        let summary = format!(
            "Total liberado: {:.1} MB ({} arquivos/pastas inacessíveis ignorados)",
            total_freed as f64 / (1024.0 * 1024.0),
            all_errors.len()
        );
        emit_health_event(
            &app_handle,
            "temp_cleanup_progress",
            "completed",
            &summary,
        );
        log_lines.push(summary.clone());

        // Considera sucesso mesmo com erros parciais — arquivos em uso são esperados
        let (status, message) = if total_freed > 0 {
            (
                CheckStatus::Success,
                format!(
                    "Limpeza concluída. {:.1} MB liberados{}.",
                    total_freed as f64 / (1024.0 * 1024.0),
                    if all_errors.is_empty() {
                        String::new()
                    } else {
                        format!(" ({} arquivo(s) em uso ignorado(s))", all_errors.len())
                    }
                ),
            )
        } else if all_errors.is_empty() {
            (
                CheckStatus::Success,
                "Pastas temporárias já estavam vazias. Nada a limpar.".to_string(),
            )
        } else {
            (
                CheckStatus::Warning,
                format!(
                    "Nenhum arquivo foi removido ({} arquivo(s) em uso). Tente fechar outros programas.",
                    all_errors.len()
                ),
            )
        };

        Ok(HealthCheckResult {
            id: "temp_cleanup".to_string(),
            name: "Limpeza de Arquivos Temporários".to_string(),
            status,
            message,
            details: log_lines.join("\n"),
            duration_seconds: duration,
            space_freed_mb: Some(freed_mb),
            timestamp: ts,
            locking_processes: if locking_procs.is_empty() { None } else { Some(locking_procs) },
        })
    })
    .await
    .unwrap_or_else(|e| Err(e.to_string()))
}

// ═══════════════════════════════════════════════════════════════════════════════
// Encerramento de processos que travam arquivos
// ═══════════════════════════════════════════════════════════════════════════════

/// Processos críticos do Windows que NUNCA devem ser encerrados.
const CRITICAL_PROCESSES: &[&str] = &[
    "system",
    "csrss",
    "smss",
    "wininit",
    "services",
    "lsass",
    "svchost",
    "explorer",
    "dwm",
    "winlogon",
    "taskmgr",
    "conhost",
    "ntoskrnl",
    "frameguard",
];

/// Encerra um processo pelo PID usando `taskkill /F`.
///
/// Recusa encerrar processos críticos do Windows (csrss, svchost, explorer, etc.)
/// para evitar instabilidade do sistema.
#[tauri::command]
pub async fn kill_process(pid: u32) -> Result<String, String> {
    tokio::task::spawn_blocking(move || {
        // Identifica o nome do processo antes de encerrar (para validar contra deny-list)
        let name_output = Command::new("tasklist.exe")
            .args(["/FI", &format!("PID eq {}", pid), "/FO", "CSV", "/NH"])
            .creation_flags(CREATE_NO_WINDOW)
            .output()
            .map_err(|e| format!("Erro ao consultar processo: {}", e))?;

        let name_str = String::from_utf8_lossy(&name_output.stdout);
        let proc_name = name_str
            .lines()
            .next()
            .and_then(|line| line.split(',').next())
            .map(|s| s.trim_matches('"').to_lowercase())
            .unwrap_or_default();

        if proc_name.is_empty() {
            return Err(format!("Processo PID {} não encontrado.", pid));
        }

        // Verifica contra a deny-list de processos críticos
        let base_name = proc_name.trim_end_matches(".exe");
        if CRITICAL_PROCESSES.contains(&base_name) {
            return Err(format!(
                "Processo \"{}\" é crítico para o sistema e não pode ser encerrado.",
                proc_name
            ));
        }

        let output = Command::new("taskkill.exe")
            .args(["/PID", &pid.to_string(), "/F"])
            .creation_flags(CREATE_NO_WINDOW)
            .output()
            .map_err(|e| format!("Erro ao executar taskkill: {}", e))?;

        if output.status.success() {
            Ok(format!(
                "Processo \"{}\" (PID {}) encerrado.",
                proc_name, pid
            ))
        } else {
            Err(format!(
                "Falha ao encerrar \"{}\" (PID {}): {}",
                proc_name,
                pid,
                String::from_utf8_lossy(&output.stderr).trim()
            ))
        }
    })
    .await
    .unwrap_or_else(|e| Err(e.to_string()))
}
