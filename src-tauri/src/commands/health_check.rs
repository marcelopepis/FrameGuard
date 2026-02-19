//! Comandos Tauri para verificação e manutenção da saúde do sistema.
//!
//! Diferente dos tweaks de otimização (que alteram configurações persistentes),
//! estas ações são operações pontuais de verificação e limpeza: cada uma executa
//! um processo externo ou realiza operações de I/O com streaming de output em
//! tempo real para o frontend.
//!
//! Padrão de cada ação:
//!   1. Emitir evento `"started"` no canal de progresso correspondente
//!   2. Executar o comando ou operação — streaming linha a linha via Tauri events
//!   3. Analisar resultado (exit code + conteúdo do output)
//!   4. Retornar `HealthCheckResult` estruturado com status e detalhes completos

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

use crate::utils::command_runner::{run_command, run_command_with_progress, CommandEvent};

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

/// Remove todos os itens dentro de `path` sem remover o diretório raiz em si.
///
/// Continua após erros individuais (arquivos em uso, sem permissão) para maximizar
/// a limpeza. Acumula os bytes efetivamente liberados em `freed` e retorna a lista
/// de erros encontrados (usados no campo `details` do resultado).
fn delete_dir_contents(path: &Path, freed: &mut u64) -> Vec<String> {
    let mut errors: Vec<String> = Vec::new();

    if !path.exists() {
        return errors;
    }

    let entries = match std::fs::read_dir(path) {
        Ok(e) => e,
        Err(e) => {
            errors.push(format!("Erro ao abrir {}: {}", path.display(), e));
            return errors;
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
            // Arquivo em uso (ex: lock do Windows Update) ou sem permissão — pula e continua
            Err(e) => errors.push(format!(
                "{}: {}",
                entry_path.file_name().unwrap_or_default().to_string_lossy(),
                e
            )),
        }
    }

    errors
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
///
/// Quando o chkdsk tenta verificar o disco do sistema (ex: C:) em uso, ele não
/// consegue bloquear o volume e apresenta um prompt "Y/N" para agendar a verificação
/// no próximo boot. Este helper escreve `"Y\n"` no stdin imediatamente após o spawn,
/// garantindo que o agendamento seja confirmado sem intervenção do usuário.
///
/// O stdin é fechado logo após a escrita (drop implícito) para que o chkdsk
/// não fique aguardando mais input e prossiga normalmente.
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
    // O buffer do pipe mantém "Y\n" disponível para quando o chkdsk lê stdin.
    if let Some(mut stdin) = child.stdin.take() {
        let _ = stdin.write_all(b"Y\n");
        // drop(stdin) aqui — fecha o pipe; chkdsk não fica esperando mais input
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

    // Thread A: lê stdout linha a linha
    thread::spawn(move || {
        let reader = BufReader::new(stdout);
        for line in reader.lines().flatten() {
            if tx_out.send(ChkdskLine::Out(line)).is_err() {
                break;
            }
        }
        let _ = tx_out.send(ChkdskLine::Done);
    });

    // Thread B: lê stderr linha a linha
    thread::spawn(move || {
        let reader = BufReader::new(stderr);
        for line in reader.lines().flatten() {
            if tx_err.send(ChkdskLine::Err(line)).is_err() {
                break;
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

// ═══════════════════════════════════════════════════════════════════════════════
// DISM — Cleanup-Image /StartComponentCleanup
//
// O Windows Update mantém cópias antigas dos componentes do sistema no
// Component Store (WinSxS) para permitir rollback de atualizações. Com o tempo,
// esse acúmulo pode ocupar vários gigabytes. O StartComponentCleanup remove
// componentes de versões antigas que não são mais necessárias para rollback.
//
// Duração típica: 1–10 minutos dependendo do histórico de atualizações.
// Requer: Administrador.
// ═══════════════════════════════════════════════════════════════════════════════

/// Limpa componentes antigos de atualizações via DISM StartComponentCleanup.
///
/// Streaming de progresso emitido no canal `"dism_cleanup_progress"`.
/// Cada linha de output do DISM é enviada ao frontend em tempo real.
#[tauri::command]
pub fn run_dism_cleanup(app_handle: tauri::AppHandle) -> Result<HealthCheckResult, String> {
    let start = Instant::now();
    let ts = now_utc();

    let result = run_command_with_progress(
        &app_handle,
        "dism_cleanup_progress",
        "dism.exe",
        &["/Online", "/Cleanup-Image", "/StartComponentCleanup"],
    )?;

    let duration = start.elapsed().as_secs();
    let stdout_lower = result.stdout.to_lowercase();

    // DISM /StartComponentCleanup termina com "The operation completed successfully."
    // quando bem-sucedido, independente de quanto espaço foi liberado.
    let (status, message) = if result.success
        && stdout_lower.contains("operation completed successfully")
    {
        (
            CheckStatus::Success,
            "Component Store limpo com sucesso. Espaço de versões antigas liberado.".to_string(),
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
        space_freed_mb: None, // DISM não reporta o total exato liberado no stdout
        timestamp: ts,
    })
}

// ═══════════════════════════════════════════════════════════════════════════════
// DISM — Cleanup-Image /CheckHealth
//
// Verifica rapidamente se há indicações de corrupção no Component Store,
// consultando apenas metadados locais (não faz download nem reparo).
// É a verificação mais rápida — geralmente completa em menos de 30 segundos.
//
// Saídas possíveis:
//   "No component store corruption detected." → sem corrupção
//   "The component store is repairable."      → corrupção detectada, reparável
//   "The component store is corrupted."       → corrupção grave
// ═══════════════════════════════════════════════════════════════════════════════

/// Verificação rápida de integridade do Component Store via DISM CheckHealth.
///
/// Streaming de progresso emitido no canal `"dism_checkhealth_progress"`.
#[tauri::command]
pub fn run_dism_checkhealth(app_handle: tauri::AppHandle) -> Result<HealthCheckResult, String> {
    let start = Instant::now();
    let ts = now_utc();

    let result = run_command_with_progress(
        &app_handle,
        "dism_checkhealth_progress",
        "dism.exe",
        &["/Online", "/Cleanup-Image", "/CheckHealth"],
    )?;

    let duration = start.elapsed().as_secs();
    let stdout_lower = result.stdout.to_lowercase();

    // Analisa saída para distinguir os três estados possíveis
    let (status, message) = if !result.success {
        (
            CheckStatus::Error,
            format!("DISM falhou com código {}.", result.exit_code),
        )
    } else if stdout_lower.contains("no component store corruption detected") {
        (
            CheckStatus::Success,
            "Nenhuma corrupção detectada no Component Store.".to_string(),
        )
    } else if stdout_lower.contains("repairable") {
        (
            CheckStatus::Warning,
            "Corrupção detectada no Component Store. Execute DISM RestoreHealth para reparar."
                .to_string(),
        )
    } else if stdout_lower.contains("corrupted") {
        (
            CheckStatus::Error,
            "Component Store corrompido. Execute DISM RestoreHealth imediatamente.".to_string(),
        )
    } else {
        // Sem mensagem de status conhecida — reporta como warning para investigação
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
    })
}

// ═══════════════════════════════════════════════════════════════════════════════
// DISM — Cleanup-Image /ScanHealth
//
// Realiza uma varredura mais profunda que o CheckHealth: examina todos os
// arquivos do Component Store comparando com os metadados do manifesto.
// Não faz reparos — apenas detecta e documenta problemas encontrados.
//
// Duração típica: 2–15 minutos.
// ═══════════════════════════════════════════════════════════════════════════════

/// Varredura profunda de integridade do Component Store via DISM ScanHealth.
///
/// Streaming de progresso emitido no canal `"dism_scanhealth_progress"`.
/// Pode levar vários minutos dependendo do tamanho do Component Store.
#[tauri::command]
pub fn run_dism_scanhealth(app_handle: tauri::AppHandle) -> Result<HealthCheckResult, String> {
    let start = Instant::now();
    let ts = now_utc();

    let result = run_command_with_progress(
        &app_handle,
        "dism_scanhealth_progress",
        "dism.exe",
        &["/Online", "/Cleanup-Image", "/ScanHealth"],
    )?;

    let duration = start.elapsed().as_secs();
    let stdout_lower = result.stdout.to_lowercase();

    // ScanHealth usa as mesmas mensagens de diagnóstico que o CheckHealth
    let (status, message) = if !result.success {
        (
            CheckStatus::Error,
            format!("DISM ScanHealth falhou com código {}.", result.exit_code),
        )
    } else if stdout_lower.contains("no component store corruption detected") {
        (
            CheckStatus::Success,
            "Nenhuma corrupção encontrada na varredura completa do Component Store.".to_string(),
        )
    } else if stdout_lower.contains("repairable") {
        (
            CheckStatus::Warning,
            "Corrupção reparável encontrada. Use DISM RestoreHealth para corrigir.".to_string(),
        )
    } else if stdout_lower.contains("corrupted") {
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
    })
}

// ═══════════════════════════════════════════════════════════════════════════════
// DISM — Cleanup-Image /RestoreHealth
//
// Repara arquivos corrompidos do Component Store usando o Windows Update como
// fonte de arquivos limpos. Substitui componentes danificados por versões
// íntegras baixadas diretamente da Microsoft.
//
// Requer: conexão com a internet ativa.
// Duração típica: 5–30 minutos (inclui download dos arquivos necessários).
// ═══════════════════════════════════════════════════════════════════════════════

/// Repara o Component Store via DISM RestoreHealth usando o Windows Update.
///
/// Streaming de progresso emitido no canal `"dism_restorehealth_progress"`.
/// Requer conexão ativa com a internet para baixar os arquivos de reparo.
#[tauri::command]
pub fn run_dism_restorehealth(app_handle: tauri::AppHandle) -> Result<HealthCheckResult, String> {
    let start = Instant::now();
    let ts = now_utc();

    let result = run_command_with_progress(
        &app_handle,
        "dism_restorehealth_progress",
        "dism.exe",
        &["/Online", "/Cleanup-Image", "/RestoreHealth"],
    )?;

    let duration = start.elapsed().as_secs();
    let stdout_lower = result.stdout.to_lowercase();

    let (status, message) = if result.success
        && stdout_lower.contains("operation completed successfully")
    {
        (
            CheckStatus::Success,
            "Component Store reparado com sucesso via Windows Update.".to_string(),
        )
    } else if !result.success && stdout_lower.contains("source files could not be found") {
        // Erro comum: sem acesso ao Windows Update ou arquivo de origem ausente
        (
            CheckStatus::Error,
            "Arquivos de origem não encontrados. Verifique a conexão com a internet.".to_string(),
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
    })
}

// ═══════════════════════════════════════════════════════════════════════════════
// SFC — System File Checker (/scannow)
//
// Verifica a integridade de todos os arquivos protegidos do sistema Windows
// e repara automaticamente os que estiverem corrompidos ou modificados,
// substituindo-os pela versão correta do cache local do sistema.
//
// Diferença entre SFC e DISM RestoreHealth:
//   - SFC usa cache local (C:\Windows\System32\dllcache) → não precisa de internet
//   - DISM usa Windows Update → mais abrangente, requer internet
//   - Recomendação: rodar DISM RestoreHealth primeiro, depois SFC /scannow
//
// Nota técnica: sfc.exe escreve no stdout usando WriteConsoleW internamente.
// Ao redirecionar via pipe, a saída é capturada em codepage do sistema (CP-1252
// ou similar). String::from_utf8_lossy lida com isso graciosamente.
//
// Duração típica: 10–30 minutos.
// ═══════════════════════════════════════════════════════════════════════════════

/// Executa o System File Checker (sfc /scannow) com streaming de progresso.
///
/// Streaming de progresso emitido no canal `"sfc_progress"`.
#[tauri::command]
pub fn run_sfc(app_handle: tauri::AppHandle) -> Result<HealthCheckResult, String> {
    let start = Instant::now();
    let ts = now_utc();

    let result = run_command_with_progress(
        &app_handle,
        "sfc_progress",
        "sfc.exe",
        &["/scannow"],
    )?;

    let duration = start.elapsed().as_secs();
    let stdout_lower = result.stdout.to_lowercase();

    // Analisa os quatro estados possíveis do SFC:
    let (status, message) = if stdout_lower
        .contains("did not find any integrity violations")
    {
        (
            CheckStatus::Success,
            "Nenhuma violação de integridade encontrada. Arquivos do sistema íntegros.".to_string(),
        )
    } else if stdout_lower.contains("found corrupt files and successfully repaired") {
        // Arquivos corrompidos foram encontrados E reparados com sucesso
        (
            CheckStatus::Success,
            "Arquivos corrompidos detectados e reparados com sucesso pelo SFC.".to_string(),
        )
    } else if stdout_lower.contains("found corrupt files but was unable to fix") {
        // SFC encontrou corrupção mas não conseguiu reparar — executar DISM primeiro
        (
            CheckStatus::Warning,
            "Arquivos corrompidos encontrados mas não reparados. Execute DISM RestoreHealth e tente novamente.".to_string(),
        )
    } else if stdout_lower.contains("could not perform the requested operation") {
        (
            CheckStatus::Error,
            "SFC não pôde executar a operação. Tente reiniciar e executar novamente.".to_string(),
        )
    } else if !result.success {
        (
            CheckStatus::Error,
            format!("SFC falhou com código {}.", result.exit_code),
        )
    } else {
        // SFC concluiu mas sem mensagem de status reconhecida (pode ocorrer em versões localizadas)
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
    })
}

// ═══════════════════════════════════════════════════════════════════════════════
// Check Disk (chkdsk /r)
//
// Verifica e corrige erros lógicos e físicos no sistema de arquivos do disco.
// O flag /r implica /f (corrige erros) e adiciona verificação de setores.
//
// Comportamento no disco do sistema (C: em uso):
//   - chkdsk não consegue bloquear o volume em uso
//   - Apresenta prompt "Y/N" para agendar a verificação no próximo boot
//   - Este comando confirma automaticamente com "Y" via stdin
//   - O chkdsk retornará imediatamente; a verificação ocorre no próximo reinício
//
// Exit codes do chkdsk:
//   0 = Sem erros
//   1 = Erros encontrados e corrigidos
//   2 = Limpeza de disco sugerida (não crítico)
//   3 = Volume não pôde ser verificado
// ═══════════════════════════════════════════════════════════════════════════════

/// Verifica e repara erros de disco via chkdsk com agendamento automático.
///
/// Se `drive_letter` não for fornecido, verifica o drive C:\.
/// Streaming de progresso emitido no canal `"chkdsk_progress"`.
///
/// Quando executado no disco do sistema, agenda automaticamente para o
/// próximo boot e retorna imediatamente com status `Warning`.
#[tauri::command]
pub fn run_chkdsk(
    app_handle: tauri::AppHandle,
    drive_letter: Option<String>,
) -> Result<HealthCheckResult, String> {
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
    })
}

// ═══════════════════════════════════════════════════════════════════════════════
// TRIM de SSDs
//
// O TRIM é uma instrução que o sistema operacional envia ao SSD informando
// quais blocos de dados não estão mais em uso e podem ser apagados internamente.
// Sem TRIM, o SSD acumularia blocos "sujos" que degradam a performance de escrita.
//
// O Windows executa TRIM automaticamente via Scheduled Tasks, mas executar
// manualmente garante que todos os SSDs conectados estejam otimizados agora,
// independente do agendamento automático.
//
// Implementação: usa Optimize-Volume do PowerShell (mais robusto que o
// defrag.exe para SSDs) com detecção automática via Get-PhysicalDisk.
// ═══════════════════════════════════════════════════════════════════════════════

/// Executa TRIM em todos os SSDs detectados via PowerShell Optimize-Volume.
///
/// Streaming de progresso emitido no canal `"trim_progress"`.
/// Detecta automaticamente SSDs via `Get-PhysicalDisk | Where-Object MediaType -eq 'SSD'`.
#[tauri::command]
pub fn run_ssd_trim(app_handle: tauri::AppHandle) -> Result<HealthCheckResult, String> {
    let start = Instant::now();
    let ts = now_utc();

    // Script PowerShell: detecta SSDs e executa Optimize-Volume -ReTrim em cada um.
    // [Console]::OutputEncoding garante que os logs apareçam corretamente em UTF-8.
    // -ErrorAction SilentlyContinue em Get-Partition/Get-Volume evita erros em
    // partições de sistema que não têm letra de unidade atribuída.
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
    })
}

// ═══════════════════════════════════════════════════════════════════════════════
// Flush DNS
//
// O cache DNS local armazena resoluções de nomes recentes para acelerar
// conexões subsequentes. Pode ficar desatualizado ou conter entradas
// corrompidas que causam falhas de conexão.
//
// ipconfig /flushdns limpa o cache do Resolvedor de DNS do cliente Windows.
// Útil para resolver problemas de conectividade após alterações de DNS ou
// simplesmente para garantir que endereços desatualizados não sejam usados.
//
// Duração: instantânea (< 1 segundo normalmente).
// ═══════════════════════════════════════════════════════════════════════════════

/// Limpa o cache DNS do sistema via ipconfig /flushdns.
///
/// Streaming de progresso emitido no canal `"dns_flush_progress"`.
#[tauri::command]
pub fn flush_dns(app_handle: tauri::AppHandle) -> Result<HealthCheckResult, String> {
    let start = Instant::now();
    let ts = now_utc();

    let result = run_command_with_progress(
        &app_handle,
        "dns_flush_progress",
        "ipconfig.exe",
        &["/flushdns"],
    )?;

    let duration = start.elapsed().as_secs();
    let stdout_lower = result.stdout.to_lowercase();

    // ipconfig /flushdns exibe "Successfully flushed the DNS Resolver Cache."
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
    })
}

// ═══════════════════════════════════════════════════════════════════════════════
// Limpeza de Arquivos Temporários
//
// Arquivos temporários se acumulam no disco durante uso normal do Windows:
//   - %TEMP% / %TMP%: temporários do usuário atual (instaladores, extrações)
//   - C:\Windows\Temp: temporários do sistema e serviços Windows
//   - C:\Windows\SoftwareDistribution\Download: cache do Windows Update
//     (arquivos de atualização baixados mas já instalados — seguros para remover)
//
// Arquivos em uso são pulados silenciosamente (continue on error).
// O Windows Update pode recriar a pasta SoftwareDistribution\Download automaticamente.
//
// O espaço total liberado é calculado comparando tamanho antes e depois da remoção.
// ═══════════════════════════════════════════════════════════════════════════════

/// Remove arquivos temporários das pastas %TEMP%, Windows\Temp e
/// Windows\SoftwareDistribution\Download, calculando o espaço total liberado.
///
/// Streaming de progresso emitido no canal `"temp_cleanup_progress"`.
/// Arquivos em uso são ignorados silenciosamente sem interromper a limpeza.
#[tauri::command]
pub fn run_temp_cleanup(app_handle: tauri::AppHandle) -> Result<HealthCheckResult, String> {
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
                    let msg = format!("Variável de ambiente {} não encontrada — pulando", folder_template);
                    emit_health_event(&app_handle, "temp_cleanup_progress", "stderr", &msg);
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
        let errors = delete_dir_contents(path, &mut freed_this_folder);

        total_freed += freed_this_folder;

        let result_msg = format!(
            "  → Liberados {:.1} MB de {}",
            freed_this_folder as f64 / (1024.0 * 1024.0),
            description
        );
        emit_health_event(&app_handle, "temp_cleanup_progress", "stdout", &result_msg);
        log_lines.push(result_msg);

        // Reporta erros (arquivos em uso) como informativos, não como falha
        for err in &errors {
            let err_msg = format!("  [ignorado] {}", err);
            emit_health_event(&app_handle, "temp_cleanup_progress", "stderr", &err_msg);
            log_lines.push(err_msg);
        }
        all_errors.extend(errors);
    }

    let duration = start.elapsed().as_secs();
    let freed_mb = bytes_to_mb(total_freed);

    let summary = format!(
        "Total liberado: {:.1} MB ({} arquivos/pastas inacessíveis ignorados)",
        total_freed as f64 / (1024.0 * 1024.0),
        all_errors.len()
    );
    emit_health_event(&app_handle, "temp_cleanup_progress", "completed", &summary);
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
    })
}
