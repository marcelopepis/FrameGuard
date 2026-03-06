//! Comandos de verificação e manutenção de disco.
//!
//! Três operações:
//! - **SFC** — System File Checker (`sfc /scannow`)
//! - **chkdsk** — Check Disk com agendamento automático e streaming multi-thread
//! - **SSD TRIM** — Optimize-Volume em todos os SSDs detectados

use std::io::{BufRead, BufReader, Write};
use std::os::windows::process::CommandExt;
use std::process::{Command, Stdio};
use std::sync::mpsc;
use std::thread;
use std::time::Instant;

use crate::utils::command_runner::run_command_with_progress;

use super::{emit_health_event, now_utc, CheckStatus, HealthCheckResult};

/// Suprime janela de console ao lançar subprocessos — duplicado de command_runner (privado lá).
const CREATE_NO_WINDOW: u32 = 0x0800_0000;

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
        .args([drive, "/r"])
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
                    format!(
                        "Disco {} verificado. Erros encontrados e corrigidos.",
                        drive
                    ),
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
