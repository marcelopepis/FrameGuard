//! Comandos Tauri para verificação e manutenção da saúde do sistema.
//!
//! Diferente dos tweaks de otimização (que alteram configurações persistentes),
//! estas ações são operações pontuais de verificação e limpeza: cada uma executa
//! um processo externo ou realiza operações de I/O com streaming de output em
//! tempo real para o frontend.
//!
//! Todos os comandos são `async` e delegam o trabalho bloqueante para
//! `tokio::task::spawn_blocking`, mantendo o event loop do Tauri responsivo
//! mesmo durante operações demoradas como DISM ScanHealth (2-15 minutos).
//!
//! Padrão de cada ação:
//!   1. Emitir evento `"started"` no canal de progresso correspondente
//!   2. Executar o comando ou operação — streaming linha a linha via Tauri events
//!   3. Analisar resultado (exit code + conteúdo do output)
//!   4. Retornar `HealthCheckResult` estruturado com status e detalhes completos
//!
//! Encoding: DISM e SFC são executados via PowerShell com
//! `[Console]::OutputEncoding = [System.Text.Encoding]::UTF8` para garantir
//! que a saída em PT-BR (CP-1252) seja recebida em UTF-8 pelo FrameGuard.

use chrono::Utc;
use serde::{Deserialize, Serialize};
use std::io::{BufRead, BufReader, Write};
use std::os::windows::process::CommandExt;
use std::path::Path;
use std::process::{Command, Stdio};
use std::sync::mpsc;
use std::thread;
use std::time::Instant;
use tauri::Emitter;

use crate::utils::command_runner::{run_command_with_progress, CommandEvent};
use crate::utils::file_locks;

/// Suprime janela de console ao lançar subprocessos — duplicado de command_runner (privado lá).
const CREATE_NO_WINDOW: u32 = 0x0800_0000;

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

// ─── Helpers internos ─────────────────────────────────────────────────────────

/// Retorna o instante atual em ISO 8601 UTC.
fn now_utc() -> String {
    Utc::now().format("%Y-%m-%dT%H:%M:%SZ").to_string()
}

/// Converte bytes para megabytes (truncado).
fn bytes_to_mb(bytes: u64) -> u64 {
    bytes / (1024 * 1024)
}

/// Emite um evento de progresso para o frontend usando o mesmo formato de `CommandEvent`.
/// Falhas de emissão são silenciadas para não interromper a operação em andamento.
fn emit_health_event(app: &tauri::AppHandle, event_name: &str, event_type: &str, data: &str) {
    let payload = CommandEvent {
        event_type: event_type.to_string(),
        data: data.to_string(),
        timestamp: now_utc(),
    };
    let _ = app.emit(event_name, payload);
}

/// Calcula recursivamente o tamanho total em bytes de um arquivo ou diretório.
/// Retorna 0 para caminhos inacessíveis, inexistentes ou links simbólicos circulares.
fn dir_size_bytes(path: &Path) -> u64 {
    if path.is_file() {
        return std::fs::metadata(path).map(|m| m.len()).unwrap_or(0);
    }
    let Ok(entries) = std::fs::read_dir(path) else {
        return 0;
    };
    entries
        .flatten()
        .map(|e| dir_size_bytes(&e.path()))
        .sum()
}

/// Resultado da tentativa de remoção de um diretório — erros + caminhos travados.
struct DeleteResult {
    /// Mensagens de erro formatadas para exibição no log.
    errors: Vec<String>,
    /// Caminhos completos dos arquivos que falharam por estarem em uso (os error 5 ou 32).
    locked_paths: Vec<String>,
}

/// Remove todos os itens dentro de `path` sem remover o diretório raiz em si.
///
/// Continua após erros individuais (arquivos em uso, sem permissão) para maximizar
/// a limpeza. Acumula os bytes efetivamente liberados em `freed`.
/// Para arquivos travados (os error 5/32), detecta qual processo está usando via
/// Restart Manager API e inclui a informação na mensagem de erro.
fn delete_dir_contents(path: &Path, freed: &mut u64) -> DeleteResult {
    let mut errors: Vec<String> = Vec::new();
    let mut locked_paths: Vec<String> = Vec::new();

    if !path.exists() {
        return DeleteResult { errors, locked_paths };
    }

    let entries = match std::fs::read_dir(path) {
        Ok(e) => e,
        Err(e) => {
            errors.push(format!("Erro ao abrir {}: {}", path.display(), e));
            return DeleteResult { errors, locked_paths };
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

    DeleteResult { errors, locked_paths }
}

/// Mensagem interna do canal entre as threads de leitura do chkdsk e o loop principal.
enum ChkdskLine {
    Out(String),
    Err(String),
    /// Sinaliza que uma das threads de leitura encerrou (pipe fechado)
    Done,
}

/// Executa chkdsk em um drive com streaming de progresso e confirmação automática
/// de agendamento via stdin.
fn run_chkdsk_drive(
    app: &tauri::AppHandle,
    drive: &str,
) -> Result<(String, String, i32, bool, u64), String> {
    let start = Instant::now();

    emit_health_event(
        app,
        "chkdsk_progress",
        "started",
        &format!("chkdsk.exe {} /r", drive),
    );

    let mut child = Command::new("chkdsk.exe")
        .args(&[drive, "/r"])
        .creation_flags(CREATE_NO_WINDOW)
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()
        .map_err(|e| {
            let msg = format!("Erro ao iniciar chkdsk.exe: {}", e);
            emit_health_event(app, "chkdsk_progress", "error", &msg);
            msg
        })?;

    // Confirma automaticamente o prompt de agendamento "Y/N".
    if let Some(mut stdin) = child.stdin.take() {
        let _ = stdin.write_all(b"Y\n");
    }

    let stdout = child
        .stdout
        .take()
        .ok_or_else(|| "Falha ao capturar stdout do chkdsk".to_string())?;
    let stderr = child
        .stderr
        .take()
        .ok_or_else(|| "Falha ao capturar stderr do chkdsk".to_string())?;

    let (tx_out, rx) = mpsc::channel::<ChkdskLine>();
    let tx_err = tx_out.clone();

    // Thread A: lê stdout com tolerância a encoding CP-1252
    thread::spawn(move || {
        let mut reader = BufReader::new(stdout);
        let mut buf = Vec::new();
        loop {
            buf.clear();
            match reader.read_until(b'\n', &mut buf) {
                Ok(0) => break,
                Ok(_) => {
                    while buf.last() == Some(&b'\n') || buf.last() == Some(&b'\r') {
                        buf.pop();
                    }
                    let line = String::from_utf8_lossy(&buf).into_owned();
                    if tx_out.send(ChkdskLine::Out(line)).is_err() {
                        break;
                    }
                }
                Err(_) => break,
            }
        }
        let _ = tx_out.send(ChkdskLine::Done);
    });

    // Thread B: lê stderr com tolerância a encoding CP-1252
    thread::spawn(move || {
        let mut reader = BufReader::new(stderr);
        let mut buf = Vec::new();
        loop {
            buf.clear();
            match reader.read_until(b'\n', &mut buf) {
                Ok(0) => break,
                Ok(_) => {
                    while buf.last() == Some(&b'\n') || buf.last() == Some(&b'\r') {
                        buf.pop();
                    }
                    let line = String::from_utf8_lossy(&buf).into_owned();
                    if tx_err.send(ChkdskLine::Err(line)).is_err() {
                        break;
                    }
                }
                Err(_) => break,
            }
        }
        let _ = tx_err.send(ChkdskLine::Done);
    });

    let mut stdout_lines: Vec<String> = Vec::new();
    let mut stderr_lines: Vec<String> = Vec::new();
    let mut done_count: u8 = 0;

    while done_count < 2 {
        match rx.recv() {
            Ok(ChkdskLine::Out(line)) => {
                emit_health_event(app, "chkdsk_progress", "stdout", &line);
                stdout_lines.push(line);
            }
            Ok(ChkdskLine::Err(line)) => {
                emit_health_event(app, "chkdsk_progress", "stderr", &line);
                stderr_lines.push(line);
            }
            Ok(ChkdskLine::Done) => done_count += 1,
            Err(_) => break,
        }
    }

    // Pipes já fechados — wait() retorna imediatamente
    let exit_status = child
        .wait()
        .map_err(|e| format!("Erro ao aguardar chkdsk: {}", e))?;

    let exit_code = exit_status.code().unwrap_or(-1);
    let duration_ms = start.elapsed().as_millis() as u64;

    emit_health_event(
        app,
        "chkdsk_progress",
        "completed",
        &format!("exit_code={} duration_ms={}", exit_code, duration_ms),
    );

    Ok((
        stdout_lines.join("\n"),
        stderr_lines.join("\n"),
        exit_code,
        exit_status.success(),
        duration_ms / 1000,
    ))
}

// ─── Helper de wrapper PowerShell com UTF-8 ───────────────────────────────────

/// Monta os argumentos para executar `dism_args` via PowerShell com
/// `[Console]::OutputEncoding = UTF8`, garantindo que PT-BR saia em UTF-8.
///
/// Retorna `(command, args_owned, display_label)` para passar a
/// `run_command_with_progress`.
fn ps_utf8_dism(dism_args: &str) -> (String, Vec<String>, String) {
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

// ═══════════════════════════════════════════════════════════════════════════════
// DISM — Cleanup-Image /StartComponentCleanup
// ═══════════════════════════════════════════════════════════════════════════════

/// Limpa componentes antigos de atualizações via DISM StartComponentCleanup.
#[tauri::command]
pub async fn run_dism_cleanup(app_handle: tauri::AppHandle) -> Result<HealthCheckResult, String> {
    tokio::task::spawn_blocking(move || {
        let start = Instant::now();
        let ts = now_utc();

        let (cmd, args_owned, label) =
            ps_utf8_dism("/Online /Cleanup-Image /StartComponentCleanup");
        let args_ref: Vec<&str> = args_owned.iter().map(|s| s.as_str()).collect();

        let result = run_command_with_progress(
            &app_handle,
            "dism_cleanup_progress",
            &cmd,
            &args_ref,
            Some(&label),
        )?;

        let duration = start.elapsed().as_secs();
        let stdout_lower = result.stdout.to_lowercase();

        let (status, message) = if result.success
            && (stdout_lower.contains("operation completed successfully")
                || stdout_lower.contains("concluída com êxito")
                || stdout_lower.contains("foi concluída"))
        {
            (
                CheckStatus::Success,
                "Component Store limpo com sucesso. Espaço de versões antigas liberado."
                    .to_string(),
            )
        } else if result.success {
            (
                CheckStatus::Warning,
                "DISM concluiu mas não foi possível confirmar o resultado.".to_string(),
            )
        } else {
            (
                CheckStatus::Error,
                format!(
                    "DISM falhou com código {}. Verifique se há atualizações pendentes.",
                    result.exit_code
                ),
            )
        };

        Ok(HealthCheckResult {
            id: "dism_cleanup".to_string(),
            name: "DISM Cleanup".to_string(),
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
// DISM — Cleanup-Image /CheckHealth
// ═══════════════════════════════════════════════════════════════════════════════

/// Verificação rápida de integridade do Component Store via DISM CheckHealth.
#[tauri::command]
pub async fn run_dism_checkhealth(
    app_handle: tauri::AppHandle,
) -> Result<HealthCheckResult, String> {
    tokio::task::spawn_blocking(move || {
        let start = Instant::now();
        let ts = now_utc();

        let (cmd, args_owned, label) = ps_utf8_dism("/Online /Cleanup-Image /CheckHealth");
        let args_ref: Vec<&str> = args_owned.iter().map(|s| s.as_str()).collect();

        let result = run_command_with_progress(
            &app_handle,
            "dism_checkhealth_progress",
            &cmd,
            &args_ref,
            Some(&label),
        )?;

        let duration = start.elapsed().as_secs();
        let stdout_lower = result.stdout.to_lowercase();

        // Detecta os três estados possíveis em inglês (EN) e PT-BR
        let (status, message) = if !result.success {
            (
                CheckStatus::Error,
                format!("DISM falhou com código {}.", result.exit_code),
            )
        } else if stdout_lower.contains("no component store corruption detected")
            || stdout_lower.contains("não está danificado")
        {
            (
                CheckStatus::Success,
                "Nenhuma corrupção detectada no Component Store.".to_string(),
            )
        } else if stdout_lower.contains("repairable") || stdout_lower.contains("reparável") {
            (
                CheckStatus::Warning,
                "Corrupção detectada no Component Store. Execute DISM RestoreHealth para reparar."
                    .to_string(),
            )
        } else if stdout_lower.contains("corrupted")
            || stdout_lower.contains("corrompido")
            || stdout_lower.contains("danificado")
        {
            (
                CheckStatus::Error,
                "Component Store corrompido. Execute DISM RestoreHealth imediatamente.".to_string(),
            )
        } else {
            (
                CheckStatus::Warning,
                "Não foi possível determinar o estado do Component Store. Verifique os detalhes."
                    .to_string(),
            )
        };

        Ok(HealthCheckResult {
            id: "dism_checkhealth".to_string(),
            name: "DISM CheckHealth".to_string(),
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
// DISM — Cleanup-Image /ScanHealth
// ═══════════════════════════════════════════════════════════════════════════════

/// Varredura profunda de integridade do Component Store via DISM ScanHealth.
#[tauri::command]
pub async fn run_dism_scanhealth(
    app_handle: tauri::AppHandle,
) -> Result<HealthCheckResult, String> {
    tokio::task::spawn_blocking(move || {
        let start = Instant::now();
        let ts = now_utc();

        let (cmd, args_owned, label) = ps_utf8_dism("/Online /Cleanup-Image /ScanHealth");
        let args_ref: Vec<&str> = args_owned.iter().map(|s| s.as_str()).collect();

        let result = run_command_with_progress(
            &app_handle,
            "dism_scanhealth_progress",
            &cmd,
            &args_ref,
            Some(&label),
        )?;

        let duration = start.elapsed().as_secs();
        let stdout_lower = result.stdout.to_lowercase();

        // ScanHealth usa as mesmas mensagens de diagnóstico que o CheckHealth
        let (status, message) = if !result.success {
            (
                CheckStatus::Error,
                format!("DISM ScanHealth falhou com código {}.", result.exit_code),
            )
        } else if stdout_lower.contains("no component store corruption detected")
            || stdout_lower.contains("não está danificado")
        {
            (
                CheckStatus::Success,
                "Nenhuma corrupção encontrada na varredura completa do Component Store."
                    .to_string(),
            )
        } else if stdout_lower.contains("repairable") || stdout_lower.contains("reparável") {
            (
                CheckStatus::Warning,
                "Corrupção reparável encontrada. Use DISM RestoreHealth para corrigir."
                    .to_string(),
            )
        } else if stdout_lower.contains("corrupted")
            || stdout_lower.contains("corrompido")
            || stdout_lower.contains("danificado")
        {
            (
                CheckStatus::Error,
                "Corrupção grave detectada no Component Store. Reparo necessário.".to_string(),
            )
        } else {
            (
                CheckStatus::Warning,
                "Varredura concluída. Resultado indeterminado — verifique os detalhes.".to_string(),
            )
        };

        Ok(HealthCheckResult {
            id: "dism_scanhealth".to_string(),
            name: "DISM ScanHealth".to_string(),
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
// DISM — Cleanup-Image /RestoreHealth
// ═══════════════════════════════════════════════════════════════════════════════

/// Repara o Component Store via DISM RestoreHealth usando o Windows Update.
#[tauri::command]
pub async fn run_dism_restorehealth(
    app_handle: tauri::AppHandle,
) -> Result<HealthCheckResult, String> {
    tokio::task::spawn_blocking(move || {
        let start = Instant::now();
        let ts = now_utc();

        let (cmd, args_owned, label) = ps_utf8_dism("/Online /Cleanup-Image /RestoreHealth");
        let args_ref: Vec<&str> = args_owned.iter().map(|s| s.as_str()).collect();

        let result = run_command_with_progress(
            &app_handle,
            "dism_restorehealth_progress",
            &cmd,
            &args_ref,
            Some(&label),
        )?;

        let duration = start.elapsed().as_secs();
        let stdout_lower = result.stdout.to_lowercase();

        let (status, message) = if result.success
            && (stdout_lower.contains("operation completed successfully")
                || stdout_lower.contains("concluída com êxito")
                || stdout_lower.contains("foi concluída"))
        {
            (
                CheckStatus::Success,
                "Component Store reparado com sucesso via Windows Update.".to_string(),
            )
        } else if !result.success
            && (stdout_lower.contains("source files could not be found")
                || stdout_lower.contains("arquivos de origem não foram encontrados")
                || stdout_lower.contains("ficheiros de origem não foram encontrados"))
        {
            (
                CheckStatus::Error,
                "Arquivos de origem não encontrados. Verifique a conexão com a internet."
                    .to_string(),
            )
        } else if !result.success {
            (
                CheckStatus::Error,
                format!(
                    "DISM RestoreHealth falhou com código {}. Verifique a conexão.",
                    result.exit_code
                ),
            )
        } else {
            (
                CheckStatus::Warning,
                "DISM concluiu mas não foi possível confirmar a conclusão do reparo.".to_string(),
            )
        };

        Ok(HealthCheckResult {
            id: "dism_restorehealth".to_string(),
            name: "DISM RestoreHealth".to_string(),
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
// SFC — System File Checker (/scannow)
// ═══════════════════════════════════════════════════════════════════════════════

/// Executa o System File Checker (sfc /scannow) com streaming de progresso.
#[tauri::command]
pub async fn run_sfc(app_handle: tauri::AppHandle) -> Result<HealthCheckResult, String> {
    tokio::task::spawn_blocking(move || {
        let start = Instant::now();
        let ts = now_utc();

        // SFC também usa o codepage do sistema; wrapper PowerShell garante UTF-8
        let script =
            "[Console]::OutputEncoding = [System.Text.Encoding]::UTF8; sfc.exe /scannow";
        let result = run_command_with_progress(
            &app_handle,
            "sfc_progress",
            "powershell.exe",
            &[
                "-NoProfile",
                "-NonInteractive",
                "-ExecutionPolicy",
                "Bypass",
                "-Command",
                script,
            ],
            Some("sfc.exe /scannow"),
        )?;

        let duration = start.elapsed().as_secs();
        let stdout_lower = result.stdout.to_lowercase();

        // Analisa os quatro estados possíveis do SFC (EN e PT-BR):
        let (status, message) = if stdout_lower
            .contains("did not find any integrity violations")
            || stdout_lower.contains("nenhuma violação de integridade")
        {
            (
                CheckStatus::Success,
                "Nenhuma violação de integridade encontrada. Arquivos do sistema íntegros."
                    .to_string(),
            )
        } else if stdout_lower.contains("found corrupt files and successfully repaired")
            || stdout_lower.contains("reparou com êxito")
            || stdout_lower.contains("reparados com êxito")
        {
            (
                CheckStatus::Success,
                "Arquivos corrompidos detectados e reparados com sucesso pelo SFC.".to_string(),
            )
        } else if stdout_lower.contains("found corrupt files but was unable to fix")
            || stdout_lower.contains("não foi possível reparar")
            || stdout_lower.contains("não conseguiu reparar")
        {
            (
                CheckStatus::Warning,
                "Arquivos corrompidos encontrados mas não reparados. Execute DISM RestoreHealth e tente novamente.".to_string(),
            )
        } else if stdout_lower.contains("could not perform the requested operation")
            || stdout_lower.contains("não pôde executar")
        {
            (
                CheckStatus::Error,
                "SFC não pôde executar a operação. Tente reiniciar e executar novamente."
                    .to_string(),
            )
        } else if !result.success {
            (
                CheckStatus::Error,
                format!("SFC falhou com código {}.", result.exit_code),
            )
        } else {
            (
                CheckStatus::Warning,
                "SFC concluído. Verifique o log em C:\\Windows\\Logs\\CBS\\CBS.log para detalhes."
                    .to_string(),
            )
        };

        Ok(HealthCheckResult {
            id: "sfc_scannow".to_string(),
            name: "SFC /scannow".to_string(),
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
// Check Disk (chkdsk /r)
// ═══════════════════════════════════════════════════════════════════════════════

/// Verifica e repara erros de disco via chkdsk com agendamento automático.
#[tauri::command]
pub async fn run_chkdsk(
    app_handle: tauri::AppHandle,
    drive_letter: Option<String>,
) -> Result<HealthCheckResult, String> {
    tokio::task::spawn_blocking(move || {
        let start_ts = now_utc();

        // Normaliza o drive: "C" → "C:", "C:" → "C:", None → "C:"
        let drive_raw = drive_letter.unwrap_or_else(|| "C".to_string());
        let drive = if drive_raw.ends_with(':') {
            drive_raw
        } else {
            format!("{}:", drive_raw.to_uppercase())
        };

        let (stdout, _stderr, exit_code, _success, duration_secs) =
            run_chkdsk_drive(&app_handle, &drive)?;

        let stdout_lower = stdout.to_lowercase();

        // Detecta se o chkdsk foi agendado para o próximo boot (disco do sistema em uso)
        let is_scheduled = stdout_lower.contains("schedule")
            || stdout_lower.contains("next restart")
            || stdout_lower.contains("próxima reinicialização")
            || stdout_lower.contains("cannot lock");

        let (status, message) = if is_scheduled {
            (
                CheckStatus::Warning,
                format!(
                    "Disco {} em uso pelo sistema. Verificação agendada para o próximo boot.",
                    drive
                ),
            )
        } else {
            match exit_code {
                0 => (
                    CheckStatus::Success,
                    format!("Disco {} verificado. Nenhum erro encontrado.", drive),
                ),
                1 => (
                    CheckStatus::Success,
                    format!("Disco {} verificado. Erros encontrados e corrigidos.", drive),
                ),
                2 => (
                    CheckStatus::Warning,
                    format!(
                        "Disco {} verificado. Limpeza de disco sugerida (execute Disk Cleanup).",
                        drive
                    ),
                ),
                _ => (
                    CheckStatus::Error,
                    format!(
                        "Erro ao verificar disco {} (código {}). O volume pode estar inacessível.",
                        drive, exit_code
                    ),
                ),
            }
        };

        Ok(HealthCheckResult {
            id: "chkdsk".to_string(),
            name: format!("Check Disk ({})", drive),
            status,
            message,
            details: stdout,
            duration_seconds: duration_secs,
            space_freed_mb: None,
            timestamp: start_ts,
            locking_processes: None,
        })
    })
    .await
    .unwrap_or_else(|e| Err(e.to_string()))
}

// ═══════════════════════════════════════════════════════════════════════════════
// TRIM de SSDs
// ═══════════════════════════════════════════════════════════════════════════════

/// Executa TRIM em todos os SSDs detectados via PowerShell Optimize-Volume.
#[tauri::command]
pub async fn run_ssd_trim(app_handle: tauri::AppHandle) -> Result<HealthCheckResult, String> {
    tokio::task::spawn_blocking(move || {
        let start = Instant::now();
        let ts = now_utc();

        let ps_script = r#"
[Console]::OutputEncoding = [System.Text.Encoding]::UTF8
$ssds = Get-PhysicalDisk | Where-Object { $_.MediaType -eq 'SSD' }
if ($ssds.Count -eq 0) {
    Write-Output "Nenhum SSD detectado no sistema (todos os discos sao HDD ou tipo desconhecido)"
    exit 0
}
Write-Output "Detectados $($ssds.Count) SSD(s) no sistema"
$trimmed = 0
foreach ($disk in $ssds) {
    Write-Output "Processando: $($disk.FriendlyName) [$($disk.Size / 1GB)GB]"
    $partitions = Get-Partition -DiskNumber $disk.DeviceId -ErrorAction SilentlyContinue
    if (-not $partitions) {
        Write-Output "  Sem particoes acessiveis neste disco"
        continue
    }
    foreach ($part in $partitions) {
        $vol = $part | Get-Volume -ErrorAction SilentlyContinue
        if ($vol -and $vol.DriveLetter) {
            Write-Output "  Executando TRIM em $($vol.DriveLetter): ($($vol.FileSystemLabel))"
            Optimize-Volume -DriveLetter $vol.DriveLetter -ReTrim -Verbose
            $trimmed++
        }
    }
}
Write-Output "TRIM concluido em $trimmed volume(s)"
"#;

        let result = run_command_with_progress(
            &app_handle,
            "trim_progress",
            "powershell.exe",
            &[
                "-NoProfile",
                "-NonInteractive",
                "-ExecutionPolicy",
                "Bypass",
                "-Command",
                ps_script,
            ],
            None,
        )?;

        let duration = start.elapsed().as_secs();
        let stdout_lower = result.stdout.to_lowercase();

        let (status, message) = if stdout_lower.contains("nenhum ssd detectado") {
            (
                CheckStatus::Warning,
                "Nenhum SSD detectado. O sistema pode estar usando apenas HDDs.".to_string(),
            )
        } else if result.success && stdout_lower.contains("trim concluido") {
            (
                CheckStatus::Success,
                "TRIM executado com sucesso em todos os SSDs detectados.".to_string(),
            )
        } else if !result.success {
            (
                CheckStatus::Error,
                format!(
                    "Erro ao executar TRIM (código {}). Verifique os detalhes.",
                    result.exit_code
                ),
            )
        } else {
            (
                CheckStatus::Warning,
                "TRIM executado mas o resultado não pôde ser confirmado.".to_string(),
            )
        };

        Ok(HealthCheckResult {
            id: "ssd_trim".to_string(),
            name: "TRIM de SSDs".to_string(),
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

/// Agrupa processos que travam arquivos por PID, contando quantos arquivos cada um trava.
/// Usa a Restart Manager API para re-consultar os caminhos e obter nomes de processo.
fn aggregate_locking_processes(locked_paths: &[String]) -> Vec<LockingProcessInfo> {
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

// ═══════════════════════════════════════════════════════════════════════════════
// Encerramento de processos que travam arquivos
// ═══════════════════════════════════════════════════════════════════════════════

/// Processos críticos do Windows que NUNCA devem ser encerrados.
const CRITICAL_PROCESSES: &[&str] = &[
    "system", "csrss", "smss", "wininit", "services", "lsass", "svchost",
    "explorer", "dwm", "winlogon", "taskmgr", "conhost", "ntoskrnl",
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
        if CRITICAL_PROCESSES.iter().any(|&c| base_name == c) {
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
            Ok(format!("Processo \"{}\" (PID {}) encerrado.", proc_name, pid))
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
