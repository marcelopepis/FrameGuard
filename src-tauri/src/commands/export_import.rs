//! Exportação e importação de configurações do FrameGuard em arquivo `.fg`.
//!
//! O formato `.fg` é um JSON indentado e legível por humanos que preserva todos
//! os backups de tweaks e planos de execução, permitindo restauração completa
//! após uma reinstalação do sistema operacional ou migração de máquina.
//!
//! ## Formato do arquivo `.fg`
//! ```json
//! {
//!   "frameguard_export": true,
//!   "version": "1.0",
//!   "app_version": "0.1.0",
//!   "exported_at": "2025-01-15T10:30:45Z",
//!   "machine_info": { "hostname": "PC-GAMING", "os_version": "Windows 11 Pro" },
//!   "backups": { ... conteúdo completo de backups.json ... },
//!   "plans":   { ... conteúdo completo de plans.json ... },
//!   "settings": {}
//! }
//! ```
//!
//! ## Modos de importação
//! - **replace**: Substitui completamente os dados atuais pelos do arquivo
//! - **merge**: Adiciona backups/planos novos sem sobrescrever os existentes
//!   (backups são sempre atualizados; planos só são adicionados se o ID não existe)

use chrono::Utc;
use serde::{Deserialize, Serialize};
use serde_json::{json, Value};
use std::collections::HashMap;
use std::path::PathBuf;
use std::{fs, path::Path};
use tauri::AppHandle;
use tauri_plugin_dialog::DialogExt;

use crate::utils::backup::{self, BackupEntry, BackupFile, BackupStatus};
use crate::utils::plan_manager::{self, Plan, PlansFile};

// ─── Tipos públicos ───────────────────────────────────────────────────────────

/// Metadados da máquina de origem incluídos em cada arquivo `.fg`.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MachineInfo {
    /// Nome do computador (valor de `%COMPUTERNAME%`)
    pub hostname: String,
    /// Versão do sistema operacional (ex: `"Windows 11 Pro"`)
    pub os_version: String,
}

/// Estrutura completa de um arquivo `.fg` — serializada como JSON indentado.
///
/// A extensão `.fg` é apenas um alias amigável; o conteúdo é JSON padrão
/// e pode ser aberto em qualquer editor de texto para auditoria.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct FgExportFile {
    /// Marcador de identidade — sempre `true`; verificado na validação do arquivo
    pub frameguard_export: bool,
    /// Versão do formato de exportação — atualmente `"1.0"`
    pub version: String,
    /// Versão do aplicativo que gerou o arquivo (ex: `"0.1.0"`)
    pub app_version: String,
    /// Timestamp ISO 8601 UTC da exportação
    pub exported_at: String,
    /// Metadados da máquina de origem
    pub machine_info: MachineInfo,
    /// Conteúdo completo de `backups.json` serializado como JSON
    pub backups: Value,
    /// Conteúdo completo de `plans.json` serializado como JSON
    pub plans: Value,
    /// Preferências do usuário — reservado para uso futuro
    pub settings: Value,
    /// IDs de serviços desabilitados pelo FrameGuard (ex: `["DiagTrack", "dmwappushservice"]`)
    #[serde(default)]
    pub services_disabled: Vec<String>,
    /// IDs de tarefas agendadas desabilitadas pelo FrameGuard (ex: `["Consolidator"]`)
    #[serde(default)]
    pub tasks_disabled: Vec<String>,
}

/// Resultado de uma exportação bem-sucedida.
#[derive(Debug, Serialize)]
pub struct ExportResult {
    /// Caminho absoluto do arquivo `.fg` gerado
    pub file_path: String,
    /// Tamanho do arquivo gerado em bytes
    pub file_size_bytes: u64,
    /// Quantidade de entradas de backup incluídas
    pub backup_count: usize,
    /// Quantidade de planos incluídos
    pub plan_count: usize,
    /// Quantidade de serviços desabilitados incluídos
    pub services_count: usize,
    /// Quantidade de tarefas desabilitadas incluídas
    pub tasks_count: usize,
    /// Timestamp ISO 8601 UTC da exportação
    pub exported_at: String,
}

/// Informações extraídas de um arquivo `.fg` para preview sem importar os dados.
///
/// Retornado por `validate_fg_file` para que o frontend exiba um resumo
/// e permita ao usuário escolher o modo de importação com consciência.
#[derive(Debug, Serialize)]
pub struct FgFileInfo {
    /// Versão do formato de exportação contido no arquivo
    pub version: String,
    /// Versão do aplicativo que gerou o arquivo
    pub app_version: String,
    /// Timestamp ISO 8601 UTC da exportação
    pub exported_at: String,
    /// Metadados da máquina de origem
    pub machine_info: MachineInfo,
    /// Quantidade de entradas de backup no arquivo
    pub backup_count: usize,
    /// Quantidade de planos no arquivo
    pub plan_count: usize,
    /// IDs de serviços que serão desabilitados na importação
    pub services_disabled: Vec<String>,
    /// IDs de tarefas que serão desabilitadas na importação
    pub tasks_disabled: Vec<String>,
}

/// Resumo do resultado de uma importação concluída.
#[derive(Debug, Serialize)]
pub struct ImportResult {
    /// Modo utilizado: `"replace"` ou `"merge"`
    pub mode: String,
    /// Quantidade de entradas de backup importadas/atualizadas
    pub backups_imported: usize,
    /// Quantidade de planos importados/adicionados
    pub plans_imported: usize,
    /// Quantidade de serviços desabilitados com sucesso
    pub services_disabled: usize,
    /// Quantidade de tarefas desabilitadas com sucesso
    pub tasks_disabled: usize,
    /// Avisos não críticos ocorridos durante a importação (ex: seções inválidas ignoradas)
    pub warnings: Vec<String>,
}

// ─── Helpers internos ─────────────────────────────────────────────────────────

/// Retorna o instante atual em ISO 8601 UTC.
fn now_utc() -> String {
    Utc::now().format("%Y-%m-%dT%H:%M:%SZ").to_string()
}

/// Lê um arquivo JSON do diretório `%APPDATA%\FrameGuard\` como `Value` bruto.
///
/// Retorna `json!({})` se o arquivo não existir, não puder ser lido ou for inválido.
/// Usar leitura direta do disco garante que o arquivo reflita o estado mais recente
/// mesmo que o cache em memória ainda não tenha sido inicializado.
fn read_appdata_json(filename: &str) -> Value {
    let Ok(appdata) = std::env::var("APPDATA") else {
        return json!({});
    };
    let path = PathBuf::from(appdata).join("FrameGuard").join(filename);
    if !path.exists() {
        return json!({});
    }
    let Ok(contents) = fs::read_to_string(&path) else {
        return json!({});
    };
    serde_json::from_str(&contents).unwrap_or_else(|_| json!({}))
}

/// Lê o nome do produto do Windows a partir do registro.
///
/// Retorna `"Windows"` como fallback em caso de erro.
fn read_os_version() -> String {
    use winreg::enums::HKEY_LOCAL_MACHINE;
    use winreg::RegKey;

    let hklm = RegKey::predef(HKEY_LOCAL_MACHINE);
    hklm.open_subkey(r"SOFTWARE\Microsoft\Windows NT\CurrentVersion")
        .ok()
        .and_then(|key| key.get_value::<String, _>("ProductName").ok())
        .unwrap_or_else(|| "Windows".to_string())
}

/// Coleta informações sobre a máquina atual para inclusão nos metadados do `.fg`.
fn get_machine_info() -> MachineInfo {
    MachineInfo {
        hostname: std::env::var("COMPUTERNAME")
            .unwrap_or_else(|_| "Desconhecido".to_string()),
        os_version: read_os_version(),
    }
}

/// Conta o número de backups dentro de um `Value` que representa um `BackupFile`.
fn count_backups(v: &Value) -> usize {
    v.get("backups")
        .and_then(|b| b.as_object())
        .map(|m| m.len())
        .unwrap_or(0)
}

/// Conta o número de planos dentro de um `Value` que representa um `PlansFile`.
fn count_plans(v: &Value) -> usize {
    v.get("plans")
        .and_then(|p| p.as_object())
        .map(|m| m.len())
        .unwrap_or(0)
}

/// Extrai IDs de serviços com backup Applied (prefixo `svc_`).
fn extract_applied_service_ids() -> Vec<String> {
    backup::get_all_backups()
        .unwrap_or_default()
        .iter()
        .filter(|(k, e)| k.starts_with("svc_") && matches!(e.status, BackupStatus::Applied))
        .map(|(k, _)| k.strip_prefix("svc_").unwrap_or(k).to_string())
        .collect()
}

/// Extrai IDs de tarefas com backup Applied (prefixo `task_`).
fn extract_applied_task_ids() -> Vec<String> {
    backup::get_all_backups()
        .unwrap_or_default()
        .iter()
        .filter(|(k, e)| k.starts_with("task_") && matches!(e.status, BackupStatus::Applied))
        .map(|(k, _)| k.strip_prefix("task_").unwrap_or(k).to_string())
        .collect()
}

/// Lê e valida um arquivo `.fg` no caminho fornecido.
///
/// Verifica a presença de `frameguard_export: true` e a compatibilidade da versão.
/// Retorna erro se o arquivo não existir, não for JSON válido ou não for um
/// export FrameGuard reconhecido.
fn read_and_validate_fg(path: &Path) -> Result<FgExportFile, String> {
    let contents = fs::read_to_string(path)
        .map_err(|e| format!("Erro ao ler o arquivo: {}", e))?;

    let file: FgExportFile = serde_json::from_str(&contents)
        .map_err(|e| format!("Arquivo inválido ou corrompido: {}", e))?;

    if !file.frameguard_export {
        return Err("O arquivo não é uma exportação válida do FrameGuard".to_string());
    }

    if file.version != "1.0" {
        return Err(format!(
            "Versão de formato '{}' não suportada — versão esperada: '1.0'",
            file.version
        ));
    }

    Ok(file)
}

// ═══════════════════════════════════════════════════════════════════════════════
// Comandos Tauri
// ═══════════════════════════════════════════════════════════════════════════════

/// Exporta todos os dados do FrameGuard para um arquivo `.fg` escolhido pelo usuário.
///
/// Abre um diálogo nativo de salvar arquivo com filtro `.fg`. Se o usuário cancelar
/// a seleção, retorna erro `"Exportação cancelada pelo usuário"`.
///
/// ## Dados incluídos no arquivo
/// - **Backups**: estado de todos os tweaks aplicados (`backups.json`)
/// - **Planos**: todos os planos de execução cadastrados (`plans.json`)
/// - **Máquina**: hostname e versão do Windows da origem
///
/// ## Garantia de extensão
/// Se o usuário informar um nome sem `.fg`, a extensão é adicionada automaticamente.
///
/// # Retorna
/// `ExportResult` com caminho do arquivo, tamanho em bytes e contagens.
#[tauri::command]
pub fn export_config(app_handle: AppHandle) -> Result<ExportResult, String> {
    // Abre diálogo de salvar arquivo via plugin nativo
    let chosen = app_handle
        .dialog()
        .file()
        .add_filter("Configuração FrameGuard", &["fg"])
        .set_title("Exportar configurações do FrameGuard")
        .blocking_save_file();

    let Some(fp) = chosen else {
        return Err("Exportação cancelada pelo usuário".to_string());
    };

    // Garante extensão .fg mesmo que o usuário omita
    let mut path = PathBuf::from(fp.to_string());
    if path.extension().and_then(|e| e.to_str()) != Some("fg") {
        path.set_extension("fg");
    }

    // Lê os dados atuais diretamente do disco (sempre sincronizados após cada operação)
    let backups_value = read_appdata_json("backups.json");
    let plans_value = read_appdata_json("plans.json");
    let exported_at = now_utc();

    let backup_count = count_backups(&backups_value);
    let plan_count = count_plans(&plans_value);

    // Extrai serviços e tarefas atualmente desabilitados pelo FrameGuard
    let services_disabled = extract_applied_service_ids();
    let tasks_disabled = extract_applied_task_ids();
    let services_count = services_disabled.len();
    let tasks_count = tasks_disabled.len();

    // Constrói o arquivo de exportação
    let export_file = FgExportFile {
        frameguard_export: true,
        version: "1.0".to_string(),
        app_version: "0.1.0".to_string(),
        exported_at: exported_at.clone(),
        machine_info: get_machine_info(),
        backups: backups_value,
        plans: plans_value,
        settings: json!({}),
        services_disabled,
        tasks_disabled,
    };

    let json = serde_json::to_string_pretty(&export_file)
        .map_err(|e| format!("Erro ao serializar dados de exportação: {}", e))?;

    fs::write(&path, &json)
        .map_err(|e| format!("Erro ao gravar o arquivo .fg: {}", e))?;

    let file_size_bytes = fs::metadata(&path)
        .map(|m| m.len())
        .unwrap_or(json.len() as u64);

    Ok(ExportResult {
        file_path: path.to_string_lossy().to_string(),
        file_size_bytes,
        backup_count,
        plan_count,
        services_count,
        tasks_count,
        exported_at,
    })
}

/// Importa configurações de um arquivo `.fg` escolhido pelo usuário.
///
/// Abre um diálogo nativo de abrir arquivo com filtro `.fg`. Se o usuário cancelar
/// a seleção, retorna erro `"Importação cancelada pelo usuário"`.
///
/// ## Fluxo recomendado no frontend
/// 1. Chamar `validate_fg_file` (abrindo o diálogo JS ou pedindo o path) para
///    exibir um resumo do arquivo ao usuário
/// 2. Solicitar ao usuário que escolha o modo (`"replace"` ou `"merge"`)
/// 3. Chamar este comando com o modo escolhido — um novo diálogo será aberto para
///    confirmar o arquivo
///
/// ## Modos
/// - `"replace"`: Substitui **completamente** backups e planos atuais pelos do arquivo.
///   Os dados existentes são perdidos.
/// - `"merge"`: **Adiciona** backups (substituindo entradas com mesma chave) e planos
///   novos (planos com ID já existente são preservados sem alteração).
///
/// # Parâmetros
/// - `mode`: `"replace"` | `"merge"`
///
/// # Retorna
/// `ImportResult` com modo, contagens de itens importados e avisos não críticos.
#[tauri::command]
pub fn import_config(
    app_handle: AppHandle,
    mode: String,
) -> Result<ImportResult, String> {
    if mode != "replace" && mode != "merge" {
        return Err(format!(
            "Modo inválido: '{}'. Valores aceitos: 'replace' ou 'merge'.",
            mode
        ));
    }

    // Abre diálogo de abrir arquivo via plugin nativo
    let chosen = app_handle
        .dialog()
        .file()
        .add_filter("Configuração FrameGuard", &["fg"])
        .set_title("Importar configurações do FrameGuard")
        .blocking_pick_file();

    let Some(fp) = chosen else {
        return Err("Importação cancelada pelo usuário".to_string());
    };

    let path = PathBuf::from(fp.to_string());
    let fg_file = read_and_validate_fg(&path)?;

    let mut warnings: Vec<String> = Vec::new();
    let backups_imported;
    let plans_imported;

    if mode == "replace" {
        // ── Substituição completa ──────────────────────────────────────────────
        // Os dados existentes são completamente substituídos pelos do arquivo.
        // O cache em memória é atualizado atomicamente — sem necessidade de restart.

        let backup_file: BackupFile = serde_json::from_value(fg_file.backups)
            .map_err(|e| format!("Seção 'backups' do arquivo está corrompida: {}", e))?;
        let b_count = backup_file.backups.len();
        backup::replace_all_backups(backup_file)?;

        let plans_file: PlansFile = serde_json::from_value(fg_file.plans)
            .map_err(|e| format!("Seção 'plans' do arquivo está corrompida: {}", e))?;
        let p_count = plans_file.plans.len();
        plan_manager::replace_all_plans(plans_file)?;

        backups_imported = b_count;
        plans_imported = p_count;
    } else {
        // ── Mescla ────────────────────────────────────────────────────────────
        // Erros em seções individuais geram avisos mas não interrompem a importação.

        // Backups: todas as entradas são inseridas/atualizadas
        let backup_entries: HashMap<String, BackupEntry> =
            serde_json::from_value(fg_file.backups)
                .ok()
                .and_then(|f: BackupFile| Some(f.backups))
                .unwrap_or_else(|| {
                    warnings.push(
                        "Seção 'backups' ignorada — dados inválidos no arquivo".to_string(),
                    );
                    HashMap::new()
                });

        backups_imported = backup::merge_backups(backup_entries)?;

        // Planos: apenas IDs novos são adicionados
        let new_plans: Vec<Plan> = serde_json::from_value(fg_file.plans)
            .ok()
            .map(|f: PlansFile| f.plans.into_values().collect())
            .unwrap_or_else(|| {
                warnings.push(
                    "Seção 'plans' ignorada — dados inválidos no arquivo".to_string(),
                );
                Vec::new()
            });

        plans_imported = plan_manager::merge_plans(new_plans)?;
    }

    // ── Reaplicar serviços e tarefas desabilitados ──────────────────────────
    let mut svc_disabled_count: usize = 0;
    let mut task_disabled_count: usize = 0;

    if !fg_file.services_disabled.is_empty() {
        match super::services::disable_services(fg_file.services_disabled.clone()) {
            Ok(result) => {
                svc_disabled_count = result.succeeded.len();
                for fail in &result.failed {
                    warnings.push(format!("Serviço '{}': {}", fail.id, fail.error));
                }
            }
            Err(e) => warnings.push(format!("Erro ao desabilitar serviços: {}", e)),
        }
    }

    if !fg_file.tasks_disabled.is_empty() {
        match super::services::disable_tasks(fg_file.tasks_disabled.clone()) {
            Ok(result) => {
                task_disabled_count = result.succeeded.len();
                for fail in &result.failed {
                    warnings.push(format!("Tarefa '{}': {}", fail.id, fail.error));
                }
            }
            Err(e) => warnings.push(format!("Erro ao desabilitar tarefas: {}", e)),
        }
    }

    Ok(ImportResult {
        mode,
        backups_imported,
        plans_imported,
        services_disabled: svc_disabled_count,
        tasks_disabled: task_disabled_count,
        warnings,
    })
}

/// Valida um arquivo `.fg` e retorna suas informações sem importar os dados.
///
/// Use este comando para exibir um preview ao usuário antes de solicitar
/// confirmação e modo de importação — sem nenhum efeito colateral no sistema.
///
/// ## Exemplo de fluxo no frontend
/// ```
/// const info = await invoke("validate_fg_file", { filePath: path });
/// // Exibe: info.machine_info.hostname, info.exported_at,
/// //        info.backup_count, info.plan_count
/// // Solicita modo ao usuário → chama import_config(mode)
/// ```
///
/// # Parâmetros
/// - `file_path`: caminho absoluto do arquivo `.fg` a validar
///
/// # Retorna
/// `FgFileInfo` com versão, data, máquina de origem e contagens de conteúdo.
/// Retorna erro se o arquivo não existir, for inválido ou tiver versão incompatível.
#[tauri::command]
pub fn validate_fg_file(file_path: String) -> Result<FgFileInfo, String> {
    let path = PathBuf::from(&file_path);

    if !path.exists() {
        return Err(format!("Arquivo não encontrado: {}", file_path));
    }

    let fg_file = read_and_validate_fg(&path)?;

    Ok(FgFileInfo {
        version: fg_file.version,
        app_version: fg_file.app_version,
        exported_at: fg_file.exported_at,
        machine_info: fg_file.machine_info,
        backup_count: count_backups(&fg_file.backups),
        plan_count: count_plans(&fg_file.plans),
        services_disabled: fg_file.services_disabled,
        tasks_disabled: fg_file.tasks_disabled,
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    // ── count_backups ───────────────────────────────────────────────────────

    #[test]
    fn counts_backups_in_valid_structure() {
        let v = json!({
            "backups": {
                "disable_vbs": { "status": "applied" },
                "enable_hags": { "status": "reverted" }
            }
        });
        assert_eq!(count_backups(&v), 2);
    }

    #[test]
    fn zero_for_missing_backups_field() {
        let v = json!({ "version": "1.0" });
        assert_eq!(count_backups(&v), 0);
    }

    #[test]
    fn zero_for_non_object_backups() {
        let v = json!({ "backups": "not an object" });
        assert_eq!(count_backups(&v), 0);
    }

    #[test]
    fn zero_for_empty_backups() {
        let v = json!({ "backups": {} });
        assert_eq!(count_backups(&v), 0);
    }

    // ── count_plans ─────────────────────────────────────────────────────────

    #[test]
    fn counts_plans_correctly() {
        let v = json!({
            "plans": {
                "plan_1": { "name": "Manutenção" },
                "plan_2": { "name": "Gaming" },
                "plan_3": { "name": "Privacidade" }
            }
        });
        assert_eq!(count_plans(&v), 3);
    }

    #[test]
    fn zero_for_missing_plans() {
        let v = json!({});
        assert_eq!(count_plans(&v), 0);
    }

    // ── read_and_validate_fg ────────────────────────────────────────────────

    #[test]
    fn rejects_nonexistent_file() {
        let path = Path::new(r"C:\FrameGuard_test_nonexistent.fg");
        assert!(read_and_validate_fg(path).is_err());
    }

    #[test]
    fn validates_fg_roundtrip() {
        let fg = FgExportFile {
            frameguard_export: true,
            version: "1.0".to_string(),
            app_version: "0.1.0".to_string(),
            exported_at: "2025-01-01T00:00:00Z".to_string(),
            machine_info: MachineInfo {
                hostname: "TEST-PC".to_string(),
                os_version: "Windows 11 Pro".to_string(),
            },
            backups: json!({}),
            plans: json!({}),
            settings: json!({}),
            services_disabled: vec![],
            tasks_disabled: vec![],
        };

        // Serializa para um arquivo temporário
        let tmp = std::env::temp_dir().join("frameguard_test_export.fg");
        let content = serde_json::to_string_pretty(&fg).unwrap();
        std::fs::write(&tmp, &content).unwrap();

        // Valida
        let result = read_and_validate_fg(&tmp);
        assert!(result.is_ok());
        let parsed = result.unwrap();
        assert_eq!(parsed.app_version, "0.1.0");
        assert_eq!(parsed.machine_info.hostname, "TEST-PC");

        // Limpa
        let _ = std::fs::remove_file(&tmp);
    }

    #[test]
    fn rejects_wrong_version() {
        let fg = FgExportFile {
            frameguard_export: true,
            version: "2.0".to_string(),
            app_version: "0.1.0".to_string(),
            exported_at: "2025-01-01T00:00:00Z".to_string(),
            machine_info: MachineInfo {
                hostname: "PC".to_string(),
                os_version: "W11".to_string(),
            },
            backups: json!({}),
            plans: json!({}),
            settings: json!({}),
            services_disabled: vec![],
            tasks_disabled: vec![],
        };

        let tmp = std::env::temp_dir().join("frameguard_test_bad_version.fg");
        std::fs::write(&tmp, serde_json::to_string(&fg).unwrap()).unwrap();

        let result = read_and_validate_fg(&tmp);
        assert!(result.is_err());
        assert!(result.unwrap_err().contains("2.0"));

        let _ = std::fs::remove_file(&tmp);
    }

    #[test]
    fn rejects_non_export_file() {
        let fg = json!({
            "frameguard_export": false,
            "version": "1.0",
            "app_version": "0.1.0",
            "exported_at": "2025-01-01T00:00:00Z",
            "machine_info": { "hostname": "PC", "os_version": "W11" },
            "backups": {},
            "plans": {},
            "settings": {}
        });

        let tmp = std::env::temp_dir().join("frameguard_test_not_export.fg");
        std::fs::write(&tmp, serde_json::to_string(&fg).unwrap()).unwrap();

        let result = read_and_validate_fg(&tmp);
        assert!(result.is_err());

        let _ = std::fs::remove_file(&tmp);
    }
}
