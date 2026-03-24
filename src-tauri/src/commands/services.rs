//! Gerenciamento de serviços e tarefas agendadas do Windows.
//!
//! Contém uma lista curada de serviços (~33) e tarefas agendadas (~8) que são
//! seguros para desabilitar em PCs de gaming, organizados por categoria
//! (telemetria, diagnósticos, hardware, acesso remoto, enterprise).
//!
//! Todas as operações fazem backup automático do estado original antes de
//! alterar, permitindo restauração posterior via `restore_services`/`restore_tasks`.

use serde::{Deserialize, Serialize};
use serde_json::{json, Value};

use crate::utils::backup::{
    backup_before_apply, get_all_backups, restore_from_backup, BackupStatus, OriginalValue,
    TweakCategory,
};
use crate::utils::command_runner::run_powershell;

// ── Tipos serializáveis (retornados ao frontend) ─────────────────────────────

/// Informações de um serviço Windows curado, com estado atual e metadados.
#[derive(Debug, Serialize)]
pub struct ServiceItem {
    /// Nome interno do serviço (ex: `"DiagTrack"`)
    pub id: String,
    /// Nome amigável para exibição na UI (ex: `"Connected User Experiences and Telemetry"`)
    pub display_name: String,
    /// Descrição do que o serviço faz e por que é seguro desabilitar
    pub description: String,
    /// Categoria de agrupamento: `"telemetry"`, `"diagnostics"`, `"hardware"`, `"remote"`, `"enterprise"`
    pub category: String,
    /// Estado atual do serviço: `"Running"`, `"Stopped"`, `"NotFound"`, etc.
    pub status: String,
    /// Tipo de inicialização: `"Automatic"`, `"Manual"`, `"Disabled"`, `"NotFound"`
    pub startup_type: String,
    /// `true` se o serviço só deve ser desabilitado em condições específicas (ex: sem Bluetooth)
    pub is_conditional: bool,
    /// Nota explicativa exibida quando `is_conditional` é `true`
    pub conditional_note: Option<String>,
    /// Aviso crítico exibido em destaque na UI (ex: impacto em Xbox Game Pass)
    pub warning: Option<String>,
    /// Vendor de CPU requerido para exibir este serviço: `"intel"`, `"amd"` ou `null` (universal)
    #[serde(skip_serializing_if = "Option::is_none")]
    pub cpu_vendor: Option<String>,
    /// `true` se existe backup Applied para este serviço (foi desabilitado pelo FrameGuard)
    pub has_backup: bool,
}

/// Informações de uma tarefa agendada curada, com estado atual e metadados.
#[derive(Debug, Serialize)]
pub struct TaskItem {
    /// Nome interno da tarefa (ex: `"Microsoft Compatibility Appraiser"`)
    pub id: String,
    /// Nome amigável para exibição na UI
    pub display_name: String,
    /// Descrição do que a tarefa faz e por que é seguro desabilitar
    pub description: String,
    /// Categoria de agrupamento: `"telemetry"`, `"ceip"`, `"diagnostics"`
    pub category: String,
    /// Estado atual: `"Ready"`, `"Disabled"`, `"Running"`, `"NotFound"`
    pub state: String,
    /// Caminho da tarefa no Task Scheduler (ex: `"\\Microsoft\\Windows\\Application Experience\\"`)
    pub task_path: String,
    /// Nome da tarefa no Task Scheduler
    pub task_name: String,
    /// `true` se existe backup Applied para esta tarefa
    pub has_backup: bool,
}

/// Resultado de uma operação em batch (desabilitar/restaurar múltiplos itens).
#[derive(Debug, Serialize)]
pub struct BatchResult {
    /// IDs dos itens que foram processados com sucesso
    pub succeeded: Vec<String>,
    /// Itens que falharam, com detalhes do erro
    pub failed: Vec<BatchError>,
}

/// Erro individual ocorrido durante uma operação em batch.
#[derive(Debug, Serialize)]
pub struct BatchError {
    /// ID do serviço ou tarefa que falhou
    pub id: String,
    /// Mensagem de erro descritiva
    pub error: String,
}

// ── Tipos de deserialização (saída PowerShell JSON) ──────────────────────────

#[derive(Deserialize)]
struct PsServiceInfo {
    #[serde(rename = "Name")]
    name: String,
    #[serde(rename = "Status")]
    status: String,
    #[serde(rename = "StartType")]
    start_type: String,
}

#[derive(Deserialize)]
struct PsTaskInfo {
    #[serde(rename = "Path")]
    path: String,
    #[serde(rename = "Name")]
    name: String,
    #[serde(rename = "State")]
    state: String,
}

// ── Definição interna da lista curada ────────────────────────────────────────

struct CuratedService {
    name: &'static str,
    display_name: &'static str,
    description: &'static str,
    category: &'static str,
    is_conditional: bool,
    conditional_note: Option<&'static str>,
    warning: Option<&'static str>,
    /// Vendor de CPU requerido: `Some("intel")`, `Some("amd")` ou `None` (universal)
    cpu_vendor: Option<&'static str>,
}

struct CuratedTask {
    path: &'static str,
    name: &'static str,
    display_name: &'static str,
    description: &'static str,
    category: &'static str,
}

// ── Lista curada de serviços seguros para desabilitar em PCs de gaming ───────

const CURATED_SERVICES: &[CuratedService] = &[
    // ── Telemetria e Diagnósticos ──
    CuratedService {
        name: "DiagTrack",
        display_name: "Connected User Experiences and Telemetry",
        description: "Coleta e envia dados de diagnóstico e uso para a Microsoft. Principal canal de telemetria do Windows.",
        category: "telemetry",
        is_conditional: false,
        conditional_note: None,
        warning: None,
        cpu_vendor: None,
    },
    CuratedService {
        name: "dmwappushservice",
        display_name: "WAP Push Message Routing",
        description: "Serviço auxiliar de telemetria que roteia mensagens WAP Push. Trabalha em conjunto com o DiagTrack.",
        category: "telemetry",
        is_conditional: false,
        conditional_note: None,
        warning: None,
        cpu_vendor: None,
    },
    CuratedService {
        name: "diagnosticshub.standardcollector.service",
        display_name: "Diagnostics Hub Standard Collector",
        description: "Coleta dados de diagnóstico em tempo real para ferramentas de depuração da Microsoft.",
        category: "telemetry",
        is_conditional: false,
        conditional_note: None,
        warning: None,
        cpu_vendor: None,
    },
    // ── Diagnósticos e Compatibilidade ──
    CuratedService {
        name: "diagsvc",
        display_name: "Diagnostic Execution Service",
        description: "Executa ações de diagnóstico para suporte a troubleshooting. Desnecessário em uso normal.",
        category: "diagnostics",
        is_conditional: false,
        conditional_note: None,
        warning: None,
        cpu_vendor: None,
    },
    CuratedService {
        name: "WerSvc",
        display_name: "Relatório de Erros do Windows",
        description: "Envia relatórios de erro e crash para a Microsoft. Consome recursos e envia dados.",
        category: "diagnostics",
        is_conditional: false,
        conditional_note: None,
        warning: None,
        cpu_vendor: None,
    },
    CuratedService {
        name: "PcaSvc",
        display_name: "Program Compatibility Assistant",
        description: "Monitora programas e detecta problemas de compatibilidade. Consome recursos em background.",
        category: "diagnostics",
        is_conditional: false,
        conditional_note: None,
        warning: None,
        cpu_vendor: None,
    },
    // ── Hardware não utilizado ──
    CuratedService {
        name: "BTAGService",
        display_name: "Bluetooth Audio Gateway",
        description: "Suporte a perfil de áudio Bluetooth (chamadas via fone BT). Desnecessário se não usa Bluetooth para áudio.",
        category: "hardware",
        is_conditional: true,
        conditional_note: Some("Necessário se você usa fones Bluetooth para chamadas de voz"),
        warning: None,
        cpu_vendor: None,
    },
    CuratedService {
        name: "bthserv",
        display_name: "Bluetooth Support Service",
        description: "Gerencia descoberta e associação de dispositivos Bluetooth. Desabilitar impede qualquer conexão BT.",
        category: "hardware",
        is_conditional: true,
        conditional_note: Some("Necessário se você usa qualquer dispositivo Bluetooth (mouse, teclado, fone)"),
        warning: None,
        cpu_vendor: None,
    },
    CuratedService {
        name: "WbioSrvc",
        display_name: "Windows Biometric Service",
        description: "Captura, compara e gerencia dados biométricos (impressão digital, reconhecimento facial).",
        category: "hardware",
        is_conditional: true,
        conditional_note: Some("Necessário se você usa Windows Hello (impressão digital ou reconhecimento facial)"),
        warning: None,
        cpu_vendor: None,
    },
    CuratedService {
        name: "Fax",
        display_name: "Serviço de Fax",
        description: "Permite enviar e receber faxes. Praticamente obsoleto em PCs modernos.",
        category: "hardware",
        is_conditional: false,
        conditional_note: None,
        warning: None,
        cpu_vendor: None,
    },
    CuratedService {
        name: "Spooler",
        display_name: "Spooler de Impressão",
        description: "Gerencia a fila de impressão. Desabilitar impede qualquer impressão local ou em rede.",
        category: "hardware",
        is_conditional: true,
        conditional_note: Some("Desabilite apenas se não possui impressora. Nota: remover este serviço também elimina o vetor de ataque PrintNightmare (CVE-2021-34527)"),
        warning: None,
        cpu_vendor: None,
    },
    CuratedService {
        name: "PhoneSvc",
        display_name: "Phone Service",
        description: "Gerencia o estado de telefonia do dispositivo. Desnecessário em desktops sem modem.",
        category: "hardware",
        is_conditional: false,
        conditional_note: None,
        warning: None,
        cpu_vendor: None,
    },
    CuratedService {
        name: "SensorService",
        display_name: "Sensor Service",
        description: "Gerencia sensores do dispositivo (luminosidade, acelerômetro). Desnecessário em desktops.",
        category: "hardware",
        is_conditional: false,
        conditional_note: None,
        warning: None,
        cpu_vendor: None,
    },
    CuratedService {
        name: "SensrSvc",
        display_name: "Sensor Monitoring Service",
        description: "Monitora sensores e dispara eventos baseados em dados de sensores.",
        category: "hardware",
        is_conditional: false,
        conditional_note: None,
        warning: None,
        cpu_vendor: None,
    },
    CuratedService {
        name: "SensorDataService",
        display_name: "Sensor Data Service",
        description: "Entrega dados de sensores do dispositivo. Desnecessário em PCs de mesa.",
        category: "hardware",
        is_conditional: false,
        conditional_note: None,
        warning: None,
        cpu_vendor: None,
    },
    // ── Acesso remoto ──
    CuratedService {
        name: "RemoteRegistry",
        display_name: "Registro Remoto",
        description: "Permite usuários remotos modificarem o registro do Windows. Risco de segurança se habilitado.",
        category: "remote",
        is_conditional: false,
        conditional_note: None,
        warning: None,
        cpu_vendor: None,
    },
    CuratedService {
        name: "RemoteAccess",
        display_name: "Routing and Remote Access",
        description: "Oferece serviços de roteamento para redes locais e remotas. Desnecessário para uso doméstico.",
        category: "remote",
        is_conditional: false,
        conditional_note: None,
        warning: None,
        cpu_vendor: None,
    },
    CuratedService {
        name: "TermService",
        display_name: "Serviços de Área de Trabalho Remota",
        description: "Permite acesso remoto ao computador via RDP. Risco de segurança se não utilizado.",
        category: "remote",
        is_conditional: true,
        conditional_note: Some("Desabilite apenas se não utiliza Conexão de Área de Trabalho Remota (RDP)"),
        warning: None,
        cpu_vendor: None,
    },
    // ── Enterprise / Não utilizado ──
    CuratedService {
        name: "MapsBroker",
        display_name: "Gerenciador de Mapas Baixados",
        description: "Gerencia mapas offline do Windows. Consome recursos para manter mapas atualizados em background.",
        category: "enterprise",
        is_conditional: false,
        conditional_note: None,
        warning: None,
        cpu_vendor: None,
    },
    CuratedService {
        name: "lfsvc",
        display_name: "Serviço de Geolocalização",
        description: "Monitora localização geográfica do sistema. Desnecessário em desktops de gaming.",
        category: "enterprise",
        is_conditional: false,
        conditional_note: None,
        warning: None,
        cpu_vendor: None,
    },
    CuratedService {
        name: "RetailDemo",
        display_name: "Serviço de Demonstração de Varejo",
        description: "Modo de demonstração para lojas de varejo. Completamente desnecessário em PCs pessoais.",
        category: "enterprise",
        is_conditional: false,
        conditional_note: None,
        warning: None,
        cpu_vendor: None,
    },
    CuratedService {
        name: "wisvc",
        display_name: "Serviço do Windows Insider",
        description: "Infraestrutura do programa Windows Insider (builds de preview). Desnecessário se não participa do programa.",
        category: "enterprise",
        is_conditional: false,
        conditional_note: None,
        warning: None,
        cpu_vendor: None,
    },
    CuratedService {
        name: "WpcMonSvc",
        display_name: "Parental Controls",
        description: "Monitora e aplica controles parentais no sistema. Desnecessário em PCs de uso pessoal adulto.",
        category: "enterprise",
        is_conditional: false,
        conditional_note: None,
        warning: None,
        cpu_vendor: None,
    },
    CuratedService {
        name: "SEMgrSvc",
        display_name: "Pagamentos e NFC",
        description: "Gerencia pagamentos NFC e elementos seguros. Desnecessário em desktops sem NFC.",
        category: "enterprise",
        is_conditional: true,
        conditional_note: Some("Desabilite apenas se seu PC não possui leitor NFC"),
        warning: None,
        cpu_vendor: None,
    },
    CuratedService {
        name: "AJRouter",
        display_name: "Serviço de Roteador AllJoyn",
        description: "Roteia mensagens AllJoyn para dispositivos IoT. Desnecessário se não usa dispositivos IoT compatíveis.",
        category: "enterprise",
        is_conditional: false,
        conditional_note: None,
        warning: None,
        cpu_vendor: None,
    },
    CuratedService {
        name: "WalletService",
        display_name: "WalletService",
        description: "Gerencia objetos usados pelo sistema de carteira digital. Desnecessário em desktops de gaming.",
        category: "enterprise",
        is_conditional: false,
        conditional_note: None,
        warning: None,
        cpu_vendor: None,
    },
    CuratedService {
        name: "SCardSvr",
        display_name: "Smart Card",
        description: "Gerencia acesso a smart cards (cartões inteligentes). Desnecessário se não usa autenticação por smart card.",
        category: "enterprise",
        is_conditional: false,
        conditional_note: None,
        warning: None,
        cpu_vendor: None,
    },
    CuratedService {
        name: "SCPolicySvc",
        display_name: "Smart Card Removal Policy",
        description: "Aplica política de bloqueio ao remover smart card. Desnecessário sem smart cards.",
        category: "enterprise",
        is_conditional: false,
        conditional_note: None,
        warning: None,
        cpu_vendor: None,
    },
    // ── Xbox e Gaming ──
    CuratedService {
        name: "XblAuthManager",
        display_name: "Xbox Live Auth Manager",
        description: "Gerencia autenticação com o Xbox Live. Necessário para jogos do Game Pass e conquistas.",
        category: "xbox",
        is_conditional: true,
        conditional_note: Some("Necessário se você usa Xbox Game Pass, jogos da Microsoft Store ou conquistas Xbox"),
        warning: Some("CRÍTICO: Desabilitar este serviço impede o login no Xbox Live. Jogos do Game Pass não iniciarão e conquistas (achievements) pararão de funcionar em todos os jogos Xbox."),
        cpu_vendor: None,
    },
    CuratedService {
        name: "XblGameSave",
        display_name: "Xbox Live Game Save",
        description: "Sincroniza saves de jogos Xbox com a nuvem. Necessário para manter progresso entre dispositivos.",
        category: "xbox",
        is_conditional: true,
        conditional_note: Some("Necessário se você usa saves na nuvem em jogos Xbox/Game Pass"),
        warning: Some("CRÍTICO: Desabilitar este serviço interrompe a sincronização de saves na nuvem. Progresso de jogos pode não ser salvo ou restaurado entre sessões e dispositivos."),
        cpu_vendor: None,
    },
    CuratedService {
        name: "XboxNetApiSvc",
        display_name: "Xbox Live Networking Service",
        description: "Gerencia rede Xbox Live para multiplayer online, detecção de NAT e party chat.",
        category: "xbox",
        is_conditional: true,
        conditional_note: Some("Necessário se você joga multiplayer online em jogos Xbox/Game Pass"),
        warning: Some("CRÍTICO: Desabilitar este serviço quebra o multiplayer online em jogos Xbox Live, detecção de NAT e party chat."),
        cpu_vendor: None,
    },
    CuratedService {
        name: "XboxGipSvc",
        display_name: "Xbox Accessory Management Service",
        description: "Gerencia periféricos Xbox conectados via USB ou wireless (controles, headsets).",
        category: "xbox",
        is_conditional: true,
        conditional_note: Some("Necessário se você usa controles Xbox Elite, Series ou outros periféricos Xbox"),
        warning: Some("Atenção: Desabilitar impede atualizações de firmware e remapeamento de botões em controles Xbox Elite e Series. Seguro apenas se não usa periféricos Xbox."),
        cpu_vendor: None,
    },
    // ── Telemetria Intel (cpu_vendor: intel) ──
    // NOTA: Os nomes de serviço abaixo correspondem aos drivers Intel mais comuns,
    // mas podem variar conforme a versão do driver instalado. Validar em hardware
    // Intel real com `sc query type= service state= all | findstr /i intel`.
    CuratedService {
        name: "igfxCUIService2.0.0.0",
        display_name: "Telemetria Intel Graphics",
        description: "Serviço de coleta de dados de uso da Intel Graphics. Envia dados de telemetria sobre o driver gráfico. Seguro desabilitar.",
        category: "telemetry",
        is_conditional: false,
        conditional_note: None,
        warning: None,
        cpu_vendor: Some("intel"),
    },
    CuratedService {
        name: "DTTSvc",
        display_name: "Intel Dynamic Tuning Telemetry",
        description: "Coleta dados de temperatura e performance para a Intel Dynamic Tuning Technology. Seguro desabilitar sem afetar o funcionamento do processador.",
        category: "telemetry",
        is_conditional: false,
        conditional_note: None,
        warning: None,
        cpu_vendor: Some("intel"),
    },
];

// ── Lista curada de tarefas agendadas ────────────────────────────────────────

const CURATED_TASKS: &[CuratedTask] = &[
    // ── Telemetria ──
    CuratedTask {
        path: "\\Microsoft\\Windows\\Application Experience\\",
        name: "Microsoft Compatibility Appraiser",
        display_name: "Compatibility Appraiser",
        description:
            "Coleta dados de compatibilidade de programas e envia para a Microsoft via CEIP.",
        category: "telemetry",
    },
    CuratedTask {
        path: "\\Microsoft\\Windows\\Application Experience\\",
        name: "ProgramDataUpdater",
        display_name: "Program Data Updater",
        description: "Atualiza cache de dados de telemetria de compatibilidade de aplicações.",
        category: "telemetry",
    },
    // ── CEIP ──
    CuratedTask {
        path: "\\Microsoft\\Windows\\Customer Experience Improvement Program\\",
        name: "Consolidator",
        display_name: "CEIP Consolidator",
        description: "Consolida e envia dados do Customer Experience Improvement Program.",
        category: "ceip",
    },
    CuratedTask {
        path: "\\Microsoft\\Windows\\Customer Experience Improvement Program\\",
        name: "UsbCeip",
        display_name: "USB CEIP",
        description: "Coleta dados de uso de dispositivos USB para o programa CEIP.",
        category: "ceip",
    },
    CuratedTask {
        path: "\\Microsoft\\Windows\\Customer Experience Improvement Program\\",
        name: "KernelCeipTask",
        display_name: "Kernel CEIP",
        description: "Coleta dados de telemetria do kernel para o programa CEIP.",
        category: "ceip",
    },
    // ── Diagnósticos ──
    CuratedTask {
        path: "\\Microsoft\\Windows\\DiskDiagnostic\\",
        name: "Microsoft-Windows-DiskDiagnosticDataCollector",
        display_name: "Disk Diagnostic Data Collector",
        description: "Coleta dados de diagnóstico de disco para envio à Microsoft.",
        category: "diagnostics",
    },
    CuratedTask {
        path: "\\Microsoft\\Windows\\Feedback\\Siuf\\",
        name: "DmClient",
        display_name: "Feedback DM Client",
        description: "Cliente de dispositivo para o sistema de feedback do Windows.",
        category: "diagnostics",
    },
    CuratedTask {
        path: "\\Microsoft\\Windows\\Feedback\\Siuf\\",
        name: "DmClientOnScenarioDownload",
        display_name: "Feedback Scenario Download",
        description: "Download de cenários para coleta de feedback do Windows.",
        category: "diagnostics",
    },
];

// ── Helpers ──────────────────────────────────────────────────────────────────

/// Gera o ID de backup para um serviço (prefixo `svc_`).
fn svc_backup_id(name: &str) -> String {
    format!("svc_{}", name)
}

/// Gera o ID de backup para uma tarefa agendada (prefixo `task_`).
fn task_backup_id(name: &str) -> String {
    format!("task_{}", name)
}

/// Verifica se existe um backup com status `Applied` para o ID fornecido.
fn has_applied_backup(backup_id: &str) -> bool {
    match get_all_backups() {
        Ok(backups) => matches!(
            backups.get(backup_id).map(|e| &e.status),
            Some(BackupStatus::Applied)
        ),
        Err(_) => false,
    }
}

/// Constrói script PowerShell para consultar todos os serviços curados de uma vez.
fn build_services_query_script() -> String {
    let names: Vec<String> = CURATED_SERVICES
        .iter()
        .map(|s| format!("'{}'", s.name))
        .collect();

    let mut ps = String::with_capacity(512);
    ps.push_str("$names = @(");
    ps.push_str(&names.join(","));
    ps.push_str("); $r = @(); foreach($n in $names) { try { ");
    ps.push_str("$s = Get-Service -Name $n -EA Stop; ");
    ps.push_str("$r += [PSCustomObject]@{Name=$n;Status=$s.Status.ToString();StartType=$s.StartType.ToString()} ");
    ps.push_str("} catch { ");
    ps.push_str("$r += [PSCustomObject]@{Name=$n;Status='NotFound';StartType='NotFound'} ");
    ps.push_str("} }; ConvertTo-Json @($r) -Compress");
    ps
}

/// Constrói script PowerShell para consultar todas as tarefas curadas de uma vez.
fn build_tasks_query_script() -> String {
    let mut ps = String::with_capacity(1024);
    ps.push_str("$tasks = @(");

    for (i, t) in CURATED_TASKS.iter().enumerate() {
        if i > 0 {
            ps.push(',');
        }
        // PowerShell hashtable: @{P='path';N='name'}
        ps.push_str(&format!("@{{P='{}';N='{}'}}", t.path, t.name));
    }

    ps.push_str("); $r = @(); foreach($t in $tasks) { try { ");
    ps.push_str("$st = Get-ScheduledTask -TaskPath $t.P -TaskName $t.N -EA Stop; ");
    ps.push_str("$r += [PSCustomObject]@{Path=$t.P;Name=$t.N;State=$st.State.ToString()} ");
    ps.push_str("} catch { ");
    ps.push_str("$r += [PSCustomObject]@{Path=$t.P;Name=$t.N;State='NotFound'} ");
    ps.push_str("} }; ConvertTo-Json @($r) -Compress");
    ps
}

/// Tenta parsear saída JSON como array; se for objeto único, encapsula em `Vec`.
///
/// PowerShell retorna `[...]` para múltiplos resultados, mas `{...}` para
/// resultado único. Esta função normaliza ambos os casos.
fn parse_json_array<T: for<'de> Deserialize<'de>>(json_str: &str) -> Result<Vec<T>, String> {
    let trimmed = json_str.trim();
    if trimmed.starts_with('[') {
        serde_json::from_str(trimmed).map_err(|e| format!("Erro ao parsear JSON: {}", e))
    } else if trimmed.starts_with('{') {
        let single: T =
            serde_json::from_str(trimmed).map_err(|e| format!("Erro ao parsear JSON: {}", e))?;
        Ok(vec![single])
    } else {
        Err(format!("Saída inesperada do PowerShell: {}", trimmed))
    }
}

// ── Comandos: Serviços ──────────────────────────────────────────────────────

/// Retorna o status atual de todos os serviços curados.
///
/// Executa uma única consulta PowerShell em batch para obter o estado
/// de todos os serviços da lista curada, evitando múltiplas invocações.
///
/// # Erros
/// Retorna `Err` se o PowerShell falhar ao executar ou se o JSON retornado
/// não puder ser deserializado.
#[tauri::command]
pub async fn get_services_status() -> Result<Vec<ServiceItem>, String> {
    tokio::task::spawn_blocking(|| {
        let script = build_services_query_script();
        let output = run_powershell(&script)?;

        if !output.success {
            return Err(format!("Erro ao consultar serviços: {}", output.stderr));
        }

        let ps_items: Vec<PsServiceInfo> = parse_json_array(&output.stdout)?;

        let mut items = Vec::with_capacity(CURATED_SERVICES.len());
        for curated in CURATED_SERVICES {
            let ps = ps_items.iter().find(|p| p.name == curated.name);
            let (status, startup_type) = match ps {
                Some(p) => (p.status.clone(), p.start_type.clone()),
                None => ("NotFound".into(), "NotFound".into()),
            };

            let bid = svc_backup_id(curated.name);
            items.push(ServiceItem {
                id: curated.name.to_string(),
                display_name: curated.display_name.to_string(),
                description: curated.description.to_string(),
                category: curated.category.to_string(),
                status,
                startup_type,
                is_conditional: curated.is_conditional,
                conditional_note: curated.conditional_note.map(String::from),
                warning: curated.warning.map(String::from),
                cpu_vendor: curated.cpu_vendor.map(String::from),
                has_backup: has_applied_backup(&bid),
            });
        }

        Ok(items)
    })
    .await
    .map_err(|e| e.to_string())?
}

/// Desabilita os serviços selecionados, salvando o tipo de startup original em backup.
///
/// Para cada ID fornecido:
/// 1. Verifica se está na lista curada (rejeita IDs desconhecidos)
/// 2. Consulta o estado atual via PowerShell
/// 3. Cria backup do `StartType` original
/// 4. Define `StartupType = Disabled` e para o serviço
///
/// Serviços já desabilitados são contabilizados como sucesso sem ação.
///
/// # Erros
/// Retorna `Err` apenas em falhas catastróficas. Falhas individuais são
/// reportadas em `BatchResult.failed` sem interromper o processamento.
#[tauri::command]
pub fn disable_services(ids: Vec<String>) -> Result<BatchResult, String> {
    let mut result = BatchResult {
        succeeded: Vec::new(),
        failed: Vec::new(),
    };

    for id in &ids {
        // Verifica se é um serviço curado
        if !CURATED_SERVICES.iter().any(|s| s.name == id.as_str()) {
            result.failed.push(BatchError {
                id: id.clone(),
                error: "Serviço não está na lista curada".into(),
            });
            continue;
        }

        // Consulta estado atual
        let query = format!(
            "try {{ $s = Get-Service -Name '{}' -EA Stop; \
             [PSCustomObject]@{{Status=$s.Status.ToString();StartType=$s.StartType.ToString()}} \
             | ConvertTo-Json -Compress }} catch {{ Write-Error $_.Exception.Message }}",
            id
        );
        let query_out = match run_powershell(&query) {
            Ok(o) => o,
            Err(e) => {
                result.failed.push(BatchError {
                    id: id.clone(),
                    error: format!("Erro ao consultar: {}", e),
                });
                continue;
            }
        };

        if !query_out.success {
            result.failed.push(BatchError {
                id: id.clone(),
                error: format!("Serviço não encontrado: {}", query_out.stderr.trim()),
            });
            continue;
        }

        let data: Value = match serde_json::from_str(query_out.stdout.trim()) {
            Ok(v) => v,
            Err(e) => {
                result.failed.push(BatchError {
                    id: id.clone(),
                    error: format!("Erro ao parsear: {}", e),
                });
                continue;
            }
        };

        let original_type = data["StartType"].as_str().unwrap_or("Manual").to_string();

        // Já está desabilitado? Sucesso sem ação.
        if original_type == "Disabled" {
            result.succeeded.push(id.clone());
            continue;
        }

        // Backup do tipo de startup original
        let bid = svc_backup_id(id);
        if let Err(e) = backup_before_apply(
            &bid,
            TweakCategory::Powershell,
            &format!("Serviço {} desabilitado (era {})", id, original_type),
            OriginalValue {
                path: format!("Service:{}", id),
                key: "StartType".to_string(),
                value: Some(json!(original_type)),
                value_type: "STRING".to_string(),
            },
            json!("Disabled"),
        ) {
            // Se já tem backup Applied, o serviço já foi gerenciado por nós
            if has_applied_backup(&bid) {
                result.succeeded.push(id.clone());
                continue;
            }
            result.failed.push(BatchError {
                id: id.clone(),
                error: format!("Erro no backup: {}", e),
            });
            continue;
        }

        // Desabilita e para o serviço
        let disable = format!(
            "Set-Service -Name '{}' -StartupType Disabled -EA Stop; \
             Stop-Service -Name '{}' -Force -EA SilentlyContinue",
            id, id
        );
        match run_powershell(&disable) {
            Ok(o) if o.success => result.succeeded.push(id.clone()),
            Ok(o) => result.failed.push(BatchError {
                id: id.clone(),
                error: format!("Falha ao desabilitar: {}", o.stderr.trim()),
            }),
            Err(e) => result.failed.push(BatchError {
                id: id.clone(),
                error: e,
            }),
        }
    }

    Ok(result)
}

/// Restaura serviços selecionados ao tipo de startup original salvo no backup.
///
/// Para cada ID, recupera o `StartType` original do backup e o reaplica via
/// `Set-Service`. O backup é marcado como `Reverted`.
///
/// # Erros
/// Retorna `Err` apenas em falhas catastróficas. Falhas individuais (ex: sem
/// backup, serviço não encontrado) são reportadas em `BatchResult.failed`.
#[tauri::command]
pub fn restore_services(ids: Vec<String>) -> Result<BatchResult, String> {
    let mut result = BatchResult {
        succeeded: Vec::new(),
        failed: Vec::new(),
    };

    for id in &ids {
        let bid = svc_backup_id(id);

        // Recupera backup e marca como Reverted
        let original = match restore_from_backup(&bid) {
            Ok(o) => o,
            Err(e) => {
                result.failed.push(BatchError {
                    id: id.clone(),
                    error: format!("Sem backup: {}", e),
                });
                continue;
            }
        };

        let original_type = match &original.value {
            Some(Value::String(s)) => s.clone(),
            _ => "Manual".to_string(),
        };

        // Restaura tipo de startup original
        let restore = format!(
            "Set-Service -Name '{}' -StartupType {} -EA Stop",
            id, original_type
        );
        match run_powershell(&restore) {
            Ok(o) if o.success => result.succeeded.push(id.clone()),
            Ok(o) => result.failed.push(BatchError {
                id: id.clone(),
                error: format!("Falha ao restaurar: {}", o.stderr.trim()),
            }),
            Err(e) => result.failed.push(BatchError {
                id: id.clone(),
                error: e,
            }),
        }
    }

    Ok(result)
}

// ── Comandos: Tarefas Agendadas ─────────────────────────────────────────────

/// Retorna o status atual de todas as tarefas agendadas curadas.
///
/// Executa uma única consulta PowerShell em batch para obter o estado
/// de todas as tarefas da lista curada.
///
/// # Erros
/// Retorna `Err` se o PowerShell falhar ao executar ou se o JSON retornado
/// não puder ser deserializado.
#[tauri::command]
pub async fn get_tasks_status() -> Result<Vec<TaskItem>, String> {
    tokio::task::spawn_blocking(|| {
        let script = build_tasks_query_script();
        let output = run_powershell(&script)?;

        if !output.success {
            return Err(format!(
                "Erro ao consultar tarefas agendadas: {}",
                output.stderr
            ));
        }

        let ps_items: Vec<PsTaskInfo> = parse_json_array(&output.stdout)?;

        let mut items = Vec::with_capacity(CURATED_TASKS.len());
        for curated in CURATED_TASKS {
            let ps = ps_items
                .iter()
                .find(|p| p.path == curated.path && p.name == curated.name);
            let state = match ps {
                Some(p) => p.state.clone(),
                None => "NotFound".into(),
            };

            let bid = task_backup_id(curated.name);
            items.push(TaskItem {
                id: curated.name.to_string(),
                display_name: curated.display_name.to_string(),
                description: curated.description.to_string(),
                category: curated.category.to_string(),
                state,
                task_path: curated.path.to_string(),
                task_name: curated.name.to_string(),
                has_backup: has_applied_backup(&bid),
            });
        }

        Ok(items)
    })
    .await
    .map_err(|e| e.to_string())?
}

/// Desabilita as tarefas agendadas selecionadas, salvando o estado original em backup.
///
/// Para cada ID fornecido:
/// 1. Localiza a tarefa na lista curada (rejeita IDs desconhecidos)
/// 2. Consulta o estado atual via `Get-ScheduledTask`
/// 3. Cria backup do estado original
/// 4. Desabilita via `Disable-ScheduledTask`
///
/// Tarefas já desabilitadas são contabilizadas como sucesso sem ação.
///
/// # Erros
/// Retorna `Err` apenas em falhas catastróficas. Falhas individuais são
/// reportadas em `BatchResult.failed`.
#[tauri::command]
pub fn disable_tasks(ids: Vec<String>) -> Result<BatchResult, String> {
    let mut result = BatchResult {
        succeeded: Vec::new(),
        failed: Vec::new(),
    };

    for id in &ids {
        // Encontra a tarefa curada pelo nome
        let curated = match CURATED_TASKS.iter().find(|t| t.name == id.as_str()) {
            Some(t) => t,
            None => {
                result.failed.push(BatchError {
                    id: id.clone(),
                    error: "Tarefa não está na lista curada".into(),
                });
                continue;
            }
        };

        // Consulta estado atual
        let query = format!(
            "try {{ $t = Get-ScheduledTask -TaskPath '{}' -TaskName '{}' -EA Stop; \
             $t.State.ToString() }} catch {{ 'NotFound' }}",
            curated.path, curated.name
        );
        let query_out = match run_powershell(&query) {
            Ok(o) => o,
            Err(e) => {
                result.failed.push(BatchError {
                    id: id.clone(),
                    error: format!("Erro ao consultar: {}", e),
                });
                continue;
            }
        };

        let current_state = query_out.stdout.trim().to_string();

        // Já desabilitada?
        if current_state == "Disabled" {
            result.succeeded.push(id.clone());
            continue;
        }

        if current_state == "NotFound" {
            result.failed.push(BatchError {
                id: id.clone(),
                error: "Tarefa não encontrada no sistema".into(),
            });
            continue;
        }

        // Backup
        let bid = task_backup_id(curated.name);
        if let Err(e) = backup_before_apply(
            &bid,
            TweakCategory::Powershell,
            &format!("Tarefa {} desabilitada", curated.display_name),
            OriginalValue {
                path: format!("Task:{}", curated.path),
                key: curated.name.to_string(),
                value: Some(json!(current_state)),
                value_type: "STRING".to_string(),
            },
            json!("Disabled"),
        ) {
            if has_applied_backup(&bid) {
                result.succeeded.push(id.clone());
                continue;
            }
            result.failed.push(BatchError {
                id: id.clone(),
                error: format!("Erro no backup: {}", e),
            });
            continue;
        }

        // Desabilita a tarefa
        let disable = format!(
            "Disable-ScheduledTask -TaskPath '{}' -TaskName '{}' -EA Stop | Out-Null",
            curated.path, curated.name
        );
        match run_powershell(&disable) {
            Ok(o) if o.success => result.succeeded.push(id.clone()),
            Ok(o) => result.failed.push(BatchError {
                id: id.clone(),
                error: format!("Falha ao desabilitar: {}", o.stderr.trim()),
            }),
            Err(e) => result.failed.push(BatchError {
                id: id.clone(),
                error: e,
            }),
        }
    }

    Ok(result)
}

/// Restaura as tarefas agendadas selecionadas, reabilitando-as.
///
/// Para cada ID, marca o backup como `Reverted` e reabilita a tarefa via
/// `Enable-ScheduledTask`.
///
/// # Erros
/// Retorna `Err` apenas em falhas catastróficas. Falhas individuais (ex: sem
/// backup, tarefa não encontrada) são reportadas em `BatchResult.failed`.
#[tauri::command]
pub fn restore_tasks(ids: Vec<String>) -> Result<BatchResult, String> {
    let mut result = BatchResult {
        succeeded: Vec::new(),
        failed: Vec::new(),
    };

    for id in &ids {
        let curated = match CURATED_TASKS.iter().find(|t| t.name == id.as_str()) {
            Some(t) => t,
            None => {
                result.failed.push(BatchError {
                    id: id.clone(),
                    error: "Tarefa não encontrada na lista curada".into(),
                });
                continue;
            }
        };

        let bid = task_backup_id(curated.name);

        // Marca backup como Reverted
        if let Err(e) = restore_from_backup(&bid) {
            result.failed.push(BatchError {
                id: id.clone(),
                error: format!("Sem backup: {}", e),
            });
            continue;
        }

        // Reabilita a tarefa
        let enable = format!(
            "Enable-ScheduledTask -TaskPath '{}' -TaskName '{}' -EA Stop | Out-Null",
            curated.path, curated.name
        );
        match run_powershell(&enable) {
            Ok(o) if o.success => result.succeeded.push(id.clone()),
            Ok(o) => result.failed.push(BatchError {
                id: id.clone(),
                error: format!("Falha ao restaurar: {}", o.stderr.trim()),
            }),
            Err(e) => result.failed.push(BatchError {
                id: id.clone(),
                error: e,
            }),
        }
    }

    Ok(result)
}
