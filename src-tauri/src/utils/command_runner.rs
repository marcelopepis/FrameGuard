//! Execução de comandos externos (PowerShell, CMD, DISM) com suporte a
//! streaming de output linha a linha para o frontend via eventos Tauri.

use chrono::Utc;
use serde::Serialize;
use std::io::{BufRead, BufReader};
use std::os::windows::process::CommandExt;
use std::process::{Command, Stdio};
use std::sync::mpsc;
use std::thread;
use std::time::Instant;
use tauri::Emitter;

/// Suprime a janela de console ao lançar subprocessos em apps com GUI.
const CREATE_NO_WINDOW: u32 = 0x0800_0000;

// ─── Tipos públicos ───────────────────────────────────────────────────────────

/// Resultado completo de um comando após conclusão.
#[derive(Debug, Serialize)]
pub struct CommandOutput {
    /// Conteúdo integral do stdout (linhas unidas por `\n`, sem espaço final)
    pub stdout: String,
    /// Conteúdo integral do stderr
    pub stderr: String,
    /// Código de saída do processo; `-1` se o SO não disponibilizou o valor
    pub exit_code: i32,
    /// `true` se o processo terminou com código de saída 0
    pub success: bool,
    /// Tempo total de execução em milissegundos
    pub duration_ms: u64,
}

/// Payload dos eventos Tauri emitidos durante [`run_command_with_progress`].
#[derive(Debug, Serialize, Clone)]
pub struct CommandEvent {
    /// Tipo do evento: `"started"` | `"stdout"` | `"stderr"` | `"completed"` | `"error"`
    pub event_type: String,
    /// Linha de output, mensagem descritiva ou código de saída em `"completed"`
    pub data: String,
    /// Timestamp ISO 8601 UTC do momento em que o evento foi gerado
    pub timestamp: String,
}

/// Mensagem interna do canal entre as threads de leitura e o loop principal.
enum StreamLine {
    Out(String),
    Err(String),
    /// Sinaliza que a thread de leitura terminou (pipe fechado)
    Done,
}

// ─── Helpers internos ─────────────────────────────────────────────────────────

/// Retorna o instante atual formatado em ISO 8601 UTC.
fn now_utc() -> String {
    Utc::now().format("%Y-%m-%dT%H:%M:%SZ").to_string()
}

/// Emite um evento Tauri com payload [`CommandEvent`].
/// Falhas de emissão são ignoradas — não devem interromper a execução do comando.
fn emit_event(app: &tauri::AppHandle, event_name: &str, event_type: &str, data: &str) {
    let payload = CommandEvent {
        event_type: event_type.to_string(),
        data: data.to_string(),
        timestamp: now_utc(),
    };
    let _ = app.emit(event_name, payload);
}

// ─── API pública ──────────────────────────────────────────────────────────────

/// Executa um comando externo e aguarda a conclusão completa antes de retornar.
///
/// Captura stdout e stderr de forma integral via pipes internos.
/// Não emite eventos Tauri — use [`run_command_with_progress`] quando a UI
/// precisar de feedback em tempo real.
///
/// A janela de console é suprimida automaticamente (`CREATE_NO_WINDOW`).
///
/// # Parâmetros
/// - `command`: caminho ou nome do executável (ex: `"powershell.exe"`)
/// - `args`: argumentos a passar para o executável
///
/// # Erros
/// Retorna `Err` se o executável não for encontrado ou não puder ser iniciado.
pub fn run_command(command: &str, args: &[&str]) -> Result<CommandOutput, String> {
    let start = Instant::now();

    let output = Command::new(command)
        .args(args)
        .creation_flags(CREATE_NO_WINDOW)
        .output()
        .map_err(|e| format!("Erro ao executar '{}': {}", command, e))?;

    let duration_ms = start.elapsed().as_millis() as u64;

    Ok(CommandOutput {
        stdout: String::from_utf8_lossy(&output.stdout).trim_end().to_string(),
        stderr: String::from_utf8_lossy(&output.stderr).trim_end().to_string(),
        exit_code: output.status.code().unwrap_or(-1),
        success: output.status.success(),
        duration_ms,
    })
}

/// Executa um comando externo emitindo eventos Tauri em tempo real,
/// linha a linha, conforme stdout e stderr são produzidos.
///
/// Útil para comandos demorados como DISM, SFC ou chkdsk onde o usuário
/// precisa de feedback contínuo na interface. Internamente usa duas threads
/// de leitura (uma por pipe) alimentando um canal `mpsc`, enquanto o loop
/// principal emite os eventos e acumula as linhas para o retorno final.
///
/// # Eventos emitidos (nome = `event_name`, payload = [`CommandEvent`])
/// | `event_type`  | `data`                                      |
/// |---------------|---------------------------------------------|
/// | `"started"`   | Nome do executável                          |
/// | `"stdout"`    | Linha do stdout                             |
/// | `"stderr"`    | Linha do stderr                             |
/// | `"completed"` | `"exit_code=N duration_ms=N"`               |
/// | `"error"`     | Mensagem de erro se o processo não iniciar  |
///
/// # Parâmetros
/// - `app_handle`: handle do app Tauri para emissão dos eventos
/// - `event_name`: nome do evento Tauri (ex: `"dism-progress"`)
/// - `command`: executável a executar
/// - `args`: argumentos do executável
pub fn run_command_with_progress(
    app_handle: &tauri::AppHandle,
    event_name: &str,
    command: &str,
    args: &[&str],
) -> Result<CommandOutput, String> {
    let start = Instant::now();

    emit_event(app_handle, event_name, "started", command);

    // Inicia o processo com pipes nos dois streams
    let mut child = Command::new(command)
        .args(args)
        .creation_flags(CREATE_NO_WINDOW)
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()
        .map_err(|e| {
            let msg = format!("Erro ao iniciar '{}': {}", command, e);
            emit_event(app_handle, event_name, "error", &msg);
            msg
        })?;

    let stdout = child
        .stdout
        .take()
        .ok_or_else(|| "Falha ao capturar stdout do processo".to_string())?;
    let stderr = child
        .stderr
        .take()
        .ok_or_else(|| "Falha ao capturar stderr do processo".to_string())?;

    // Canal mpsc para agregar linhas de ambas as threads de leitura
    let (tx_out, rx) = mpsc::channel::<StreamLine>();
    let tx_err = tx_out.clone();

    // Thread A: lê stdout linha a linha e envia para o canal
    thread::spawn(move || {
        let reader = BufReader::new(stdout);
        for line in reader.lines().flatten() {
            if tx_out.send(StreamLine::Out(line)).is_err() {
                break;
            }
        }
        // Sinaliza ao loop principal que este pipe foi esgotado
        let _ = tx_out.send(StreamLine::Done);
    });

    // Thread B: lê stderr linha a linha e envia para o canal
    thread::spawn(move || {
        let reader = BufReader::new(stderr);
        for line in reader.lines().flatten() {
            if tx_err.send(StreamLine::Err(line)).is_err() {
                break;
            }
        }
        let _ = tx_err.send(StreamLine::Done);
    });

    // Loop principal: processa e emite eventos em tempo real até ambas as
    // threads sinalizarem conclusão (pipes fechados = processo encerrado)
    let mut stdout_lines: Vec<String> = Vec::new();
    let mut stderr_lines: Vec<String> = Vec::new();
    let mut done_count: u8 = 0;

    while done_count < 2 {
        match rx.recv() {
            Ok(StreamLine::Out(line)) => {
                emit_event(app_handle, event_name, "stdout", &line);
                stdout_lines.push(line);
            }
            Ok(StreamLine::Err(line)) => {
                emit_event(app_handle, event_name, "stderr", &line);
                stderr_lines.push(line);
            }
            Ok(StreamLine::Done) => {
                done_count += 1;
            }
            // Sender foi dropado inesperadamente — encerra o loop
            Err(_) => break,
        }
    }

    // Limpa o processo e obtém o status de saída.
    // Como os pipes já fecharam, wait() retorna imediatamente.
    let exit_status = child
        .wait()
        .map_err(|e| format!("Erro ao obter status de saída do processo: {}", e))?;

    let exit_code = exit_status.code().unwrap_or(-1);
    let duration_ms = start.elapsed().as_millis() as u64;

    emit_event(
        app_handle,
        event_name,
        "completed",
        &format!("exit_code={} duration_ms={}", exit_code, duration_ms),
    );

    Ok(CommandOutput {
        stdout: stdout_lines.join("\n"),
        stderr: stderr_lines.join("\n"),
        exit_code,
        success: exit_status.success(),
        duration_ms,
    })
}

/// Executa um script PowerShell inline e retorna o resultado completo.
///
/// Usa `powershell.exe` com as flags:
/// - `-NoProfile`: pula o perfil do usuário (startup mais rápido)
/// - `-NonInteractive`: suprime prompts interativos
/// - `-ExecutionPolicy Bypass`: permite execução sem restrições de política
///
/// O output é decodificado via `from_utf8_lossy`; para scripts que produzem
/// texto em codepage não-UTF8, considere forçar UTF-8 com
/// `[Console]::OutputEncoding = [Text.Encoding]::UTF8` no início do script.
///
/// # Parâmetros
/// - `script`: script PowerShell inline (ex: `"Get-PSDrive -PSProvider FileSystem"`)
#[allow(dead_code)]
pub fn run_powershell(script: &str) -> Result<CommandOutput, String> {
    run_command(
        "powershell.exe",
        &[
            "-NoProfile",
            "-NonInteractive",
            "-ExecutionPolicy",
            "Bypass",
            "-Command",
            script,
        ],
    )
}

/// Executa um comando DISM com streaming de progresso para o frontend.
///
/// DISM (Deployment Image Servicing and Management) é usado para operações
/// como verificação de integridade (`/ScanHealth`), restauração de arquivos
/// (`/RestoreHealth`) e gerenciamento de recursos do Windows. Requer que o
/// processo esteja rodando como administrador.
///
/// Os eventos de progresso são emitidos com o nome `"dism-progress"`.
/// O frontend deve registrar um listener com `listen("dism-progress", handler)`.
///
/// # Parâmetros
/// - `app_handle`: handle do app Tauri para emissão dos eventos de progresso
/// - `args`: argumentos do DISM (ex: `&["/Online", "/Cleanup-Image", "/ScanHealth"]`)
///
/// # Exemplo de uso
/// ```ignore
/// run_dism(&app, &["/Online", "/Cleanup-Image", "/RestoreHealth"])?;
/// ```
#[allow(dead_code)]
pub fn run_dism(app_handle: &tauri::AppHandle, args: &[&str]) -> Result<CommandOutput, String> {
    run_command_with_progress(app_handle, "dism-progress", "dism.exe", args)
}
