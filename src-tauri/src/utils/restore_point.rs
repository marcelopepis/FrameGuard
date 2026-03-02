//! Gerenciamento de Pontos de Restauração do Windows.
//!
//! Cria restore points automaticamente antes de aplicar tweaks ou executar planos,
//! garantindo uma safety net para o usuário reverter alterações pelo Windows Recovery.
//!
//! ## Limitações do Windows
//! - Restore points são limitados a 1 a cada 24h por padrão
//!   (chave `SystemRestorePointCreationFrequency` em HKLM)
//! - Se o recurso estiver desabilitado, a criação falha silenciosamente
//!   (erro retornado para o chamador decidir como tratar)

use std::sync::Mutex;
use std::time::{Duration, Instant};

use crate::utils::command_runner::run_powershell;

/// Intervalo mínimo entre tentativas de criação de restore point (24h).
/// O Windows já impõe esse limite, mas verificamos localmente para evitar
/// chamadas PowerShell desnecessárias.
const MIN_INTERVAL: Duration = Duration::from_secs(24 * 60 * 60);

/// Timestamp da última criação bem-sucedida de restore point nesta sessão.
static LAST_CREATED: Mutex<Option<Instant>> = Mutex::new(None);

/// Resultado da tentativa de criação de um restore point.
#[derive(Debug, Clone)]
pub enum RestorePointResult {
    /// Restore point criado com sucesso.
    Created,
    /// Já existe um restore point recente (< 24h) — criação pulada silenciosamente.
    Skipped,
    /// Restore points estão desabilitados no sistema.
    Disabled(String),
    /// Outro erro ao criar o restore point.
    Failed(String),
}

/// Verifica se já existe um restore point criado pelo FrameGuard nas últimas 24h
/// (verificação local na sessão).
pub fn has_recent_restore_point() -> bool {
    let guard = LAST_CREATED.lock().unwrap_or_else(|e| e.into_inner());
    match *guard {
        Some(instant) => instant.elapsed() < MIN_INTERVAL,
        None => false,
    }
}

/// Cria um ponto de restauração do Windows via PowerShell.
///
/// Se já existe um restore point recente (< 24h nesta sessão), retorna `Skipped`.
/// Se restore points estiverem desabilitados, retorna `Disabled` com mensagem orientativa.
///
/// # Argumentos
/// - `description`: Descrição do restore point (ex: "Antes de aplicar: Game DVR")
pub fn create_restore_point(description: &str) -> RestorePointResult {
    // Verifica cache local para evitar chamada PowerShell desnecessária
    if has_recent_restore_point() {
        return RestorePointResult::Skipped;
    }

    // Escapa aspas simples na descrição para evitar injeção no PowerShell
    let safe_desc = description.replace('\'', "''");

    // Script PowerShell para criar o restore point.
    // Checkpoint-Computer requer elevação (o app já roda como admin).
    // -RestorePointType MODIFY_SETTINGS é o tipo mais adequado para tweaks de registro.
    let script = format!(
        r#"
        try {{
            Checkpoint-Computer -Description 'FrameGuard: {desc}' -RestorePointType MODIFY_SETTINGS -ErrorAction Stop
            Write-Output 'OK'
        }} catch {{
            $msg = $_.Exception.Message
            if ($msg -match 'frequen|1077|already.*created|já.*criado') {{
                Write-Output 'FREQUENCY_LIMIT'
            }} elseif ($msg -match 'disabled|desabilitad|not enabled|não.*habilit|turned off') {{
                Write-Output 'DISABLED'
            }} else {{
                Write-Output "ERROR:$msg"
            }}
        }}
        "#,
        desc = safe_desc,
    );

    match run_powershell(&script) {
        Ok(output) => {
            let stdout = output.stdout.trim().to_string();

            if stdout.contains("OK") {
                // Sucesso — atualiza cache local
                let mut guard = LAST_CREATED.lock().unwrap_or_else(|e| e.into_inner());
                *guard = Some(Instant::now());
                RestorePointResult::Created
            } else if stdout.contains("FREQUENCY_LIMIT") {
                // Windows impôs o limite de 24h — marcar como se tivéssemos criado
                let mut guard = LAST_CREATED.lock().unwrap_or_else(|e| e.into_inner());
                *guard = Some(Instant::now());
                RestorePointResult::Skipped
            } else if stdout.contains("DISABLED") {
                RestorePointResult::Disabled(
                    "A Proteção do Sistema está desabilitada. Habilite em Propriedades do Sistema > Proteção do Sistema.".to_string()
                )
            } else if stdout.starts_with("ERROR:") {
                RestorePointResult::Failed(stdout.replacen("ERROR:", "", 1))
            } else {
                // Output inesperado — tratar como possível sucesso se exit_code == 0
                if output.success {
                    let mut guard = LAST_CREATED.lock().unwrap_or_else(|e| e.into_inner());
                    *guard = Some(Instant::now());
                    RestorePointResult::Created
                } else {
                    RestorePointResult::Failed(format!(
                        "Saída inesperada do PowerShell (exit {}): {}",
                        output.exit_code, stdout
                    ))
                }
            }
        }
        Err(e) => RestorePointResult::Failed(format!("Falha ao executar PowerShell: {}", e)),
    }
}
