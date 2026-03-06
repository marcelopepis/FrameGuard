// Remoção de apps UWP (bloatware) pré-instalados do Windows 11.
//
// Lista curada de ~35 apps com recomendações de remoção. Apps marcados como
// "Keep" não podem ser removidos. O scan usa Get-AppxPackage para detectar
// quais apps estão instalados no sistema atual.

use serde::{Deserialize, Serialize};

use crate::utils::command_runner::run_powershell;

// ── Tipos expostos ao frontend ───────────────────────────────────────────────

#[derive(Debug, Clone, Serialize)]
pub struct UwpApp {
    pub name: String,
    pub package_full_name: String,
    pub display_name: String,
    pub description: String,
    pub category: String,
    pub recommended_action: RecommendedAction,
    pub is_installed: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "lowercase")]
pub enum RecommendedAction {
    Remove,
    Optional,
    Keep,
}

#[derive(Debug, Serialize)]
pub struct RemovalResult {
    pub succeeded: Vec<String>,
    pub failed: Vec<RemovalError>,
    pub total_requested: usize,
}

#[derive(Debug, Serialize)]
pub struct RemovalError {
    pub name: String,
    pub display_name: String,
    pub error: String,
}

// ── Tipo interno para desserialização do PowerShell ──────────────────────────

#[derive(Deserialize)]
struct PsAppxInfo {
    #[serde(rename = "Name")]
    name: String,
    #[serde(rename = "PackageFullName")]
    package_full_name: String,
}

// ── Lista curada de apps ─────────────────────────────────────────────────────

struct CuratedApp {
    name: &'static str,
    display_name: &'static str,
    description: &'static str,
    category: &'static str,
    recommended_action: RecommendedAction,
}

const CURATED_APPS: &[CuratedApp] = &[
    // ── Microsoft Bloatware ──
    CuratedApp {
        name: "Microsoft.BingNews",
        display_name: "Microsoft Notícias",
        description: "App de notícias com anúncios e telemetria.",
        category: "microsoft_bloatware",
        recommended_action: RecommendedAction::Remove,
    },
    CuratedApp {
        name: "Microsoft.BingWeather",
        display_name: "Microsoft Clima",
        description: "App de previsão do tempo. Substitua pelo navegador se precisar.",
        category: "microsoft_bloatware",
        recommended_action: RecommendedAction::Remove,
    },
    CuratedApp {
        name: "Microsoft.BingFinance",
        display_name: "Microsoft Finanças",
        description: "App de finanças e cotações. Desnecessário para gaming.",
        category: "microsoft_bloatware",
        recommended_action: RecommendedAction::Remove,
    },
    CuratedApp {
        name: "Microsoft.GetHelp",
        display_name: "Obter Ajuda",
        description: "Assistente de suporte da Microsoft. Raramente útil para usuários avançados.",
        category: "microsoft_bloatware",
        recommended_action: RecommendedAction::Remove,
    },
    CuratedApp {
        name: "Microsoft.Getstarted",
        display_name: "Dicas",
        description: "App de dicas e tutoriais do Windows. Desnecessário após setup inicial.",
        category: "microsoft_bloatware",
        recommended_action: RecommendedAction::Remove,
    },
    CuratedApp {
        name: "Microsoft.MicrosoftOfficeHub",
        display_name: "Office Hub",
        description: "Redirecionador para assinatura do Microsoft 365. Não é o Office em si.",
        category: "microsoft_bloatware",
        recommended_action: RecommendedAction::Remove,
    },
    CuratedApp {
        name: "Microsoft.MicrosoftSolitaireCollection",
        display_name: "Solitaire Collection",
        description: "Coleção de jogos de paciência com anúncios e compras in-app.",
        category: "microsoft_bloatware",
        recommended_action: RecommendedAction::Remove,
    },
    CuratedApp {
        name: "Microsoft.People",
        display_name: "Contatos",
        description: "App de contatos do Windows. Raramente usado em PCs de gaming.",
        category: "microsoft_bloatware",
        recommended_action: RecommendedAction::Remove,
    },
    CuratedApp {
        name: "Microsoft.PowerAutomateDesktop",
        display_name: "Power Automate Desktop",
        description: "Ferramenta de automação enterprise. Desnecessária para gaming.",
        category: "microsoft_bloatware",
        recommended_action: RecommendedAction::Remove,
    },
    CuratedApp {
        name: "Microsoft.Todos",
        display_name: "Microsoft To Do",
        description: "App de lista de tarefas. Substituível por alternativas mais leves.",
        category: "microsoft_bloatware",
        recommended_action: RecommendedAction::Optional,
    },
    CuratedApp {
        name: "Microsoft.WindowsAlarms",
        display_name: "Alarmes e Relógio",
        description: "Alarmes, cronômetro e timer. Pouco usado em desktops de gaming.",
        category: "microsoft_bloatware",
        recommended_action: RecommendedAction::Optional,
    },
    CuratedApp {
        name: "Microsoft.WindowsFeedbackHub",
        display_name: "Feedback Hub",
        description: "Coleta feedback para a Microsoft. Canal de telemetria adicional.",
        category: "microsoft_bloatware",
        recommended_action: RecommendedAction::Remove,
    },
    CuratedApp {
        name: "Microsoft.WindowsMaps",
        display_name: "Mapas",
        description: "App de mapas offline. Consome espaço e desnecessário em desktops.",
        category: "microsoft_bloatware",
        recommended_action: RecommendedAction::Remove,
    },
    CuratedApp {
        name: "Microsoft.WindowsSoundRecorder",
        display_name: "Gravador de Voz",
        description: "Gravador de áudio simples. Substituível por alternativas melhores.",
        category: "microsoft_bloatware",
        recommended_action: RecommendedAction::Optional,
    },
    CuratedApp {
        name: "Microsoft.YourPhone",
        display_name: "Celular (Phone Link)",
        description: "Integração celular-PC. Uso contínuo de recursos em background.",
        category: "microsoft_bloatware",
        recommended_action: RecommendedAction::Remove,
    },
    CuratedApp {
        name: "Microsoft.ZuneMusic",
        display_name: "Groove Music / Media Player",
        description: "Player de música pré-instalado. Substituível por VLC, foobar2000, etc.",
        category: "microsoft_bloatware",
        recommended_action: RecommendedAction::Optional,
    },
    CuratedApp {
        name: "Microsoft.ZuneVideo",
        display_name: "Filmes e TV",
        description: "Player de vídeo com loja integrada. VLC é alternativa superior.",
        category: "microsoft_bloatware",
        recommended_action: RecommendedAction::Optional,
    },
    CuratedApp {
        name: "MicrosoftCorporationII.QuickAssist",
        display_name: "Quick Assist",
        description: "Assistência remota da Microsoft. Risco de segurança se não utilizado.",
        category: "microsoft_bloatware",
        recommended_action: RecommendedAction::Remove,
    },
    CuratedApp {
        name: "Microsoft.549981C3F5F10",
        display_name: "Cortana",
        description: "Assistente virtual da Microsoft. Consome recursos em background.",
        category: "microsoft_bloatware",
        recommended_action: RecommendedAction::Remove,
    },
    CuratedApp {
        name: "Clipchamp.Clipchamp",
        display_name: "Clipchamp",
        description:
            "Editor de vídeo da Microsoft. Pré-instalado, substituível por DaVinci Resolve.",
        category: "microsoft_bloatware",
        recommended_action: RecommendedAction::Remove,
    },
    CuratedApp {
        name: "Microsoft.OutlookForWindows",
        display_name: "Outlook (novo)",
        description:
            "Nova versão do Outlook como app UWP. Instalado automaticamente em Win11 24H2+.",
        category: "microsoft_bloatware",
        recommended_action: RecommendedAction::Optional,
    },
    CuratedApp {
        name: "MicrosoftTeams",
        display_name: "Microsoft Teams (pré-instalado)",
        description: "Chat e reuniões enterprise. Versão pré-instalada consome recursos.",
        category: "microsoft_bloatware",
        recommended_action: RecommendedAction::Remove,
    },
    // ── Jogos e Xbox pré-instalados ──
    CuratedApp {
        name: "Microsoft.XboxGamingOverlay",
        display_name: "Xbox Game Bar",
        description: "Overlay para gaming (FPS, captura). Desabilite se usar Steam Overlay.",
        category: "games_preinstalled",
        recommended_action: RecommendedAction::Optional,
    },
    CuratedApp {
        name: "Microsoft.GamingApp",
        display_name: "Xbox App",
        description: "App principal do Xbox. Necessário para Game Pass. Remova se não usa.",
        category: "games_preinstalled",
        recommended_action: RecommendedAction::Optional,
    },
    CuratedApp {
        name: "Microsoft.Xbox.TCUI",
        display_name: "Xbox TCUI",
        description: "Interface de conversação do Xbox. Complementar ao Xbox App.",
        category: "games_preinstalled",
        recommended_action: RecommendedAction::Optional,
    },
    CuratedApp {
        name: "Microsoft.XboxSpeechToTextOverlay",
        display_name: "Xbox Speech to Text",
        description: "Conversão voz-texto para acessibilidade no Xbox. Raramente usado.",
        category: "games_preinstalled",
        recommended_action: RecommendedAction::Remove,
    },
    // ── Apps de terceiros pré-instalados ──
    CuratedApp {
        name: "SpotifyAB.SpotifyMusic",
        display_name: "Spotify (pré-instalado)",
        description: "Player de música pré-instalado por parceria. Reinstale da Store se quiser.",
        category: "third_party",
        recommended_action: RecommendedAction::Optional,
    },
    CuratedApp {
        name: "Disney.37853FC22B2CE",
        display_name: "Disney+",
        description: "App de streaming pré-instalado por parceria.",
        category: "third_party",
        recommended_action: RecommendedAction::Remove,
    },
    CuratedApp {
        name: "5A894077.McAfeeSecurity",
        display_name: "McAfee Security",
        description: "Antivírus pré-instalado por OEM. Windows Defender é suficiente.",
        category: "third_party",
        recommended_action: RecommendedAction::Remove,
    },
    CuratedApp {
        name: "4DF9E0F8.Netflix",
        display_name: "Netflix",
        description: "App de streaming pré-instalado. Use o navegador como alternativa.",
        category: "third_party",
        recommended_action: RecommendedAction::Optional,
    },
    CuratedApp {
        name: "AmazonVideo.PrimeVideo",
        display_name: "Prime Video",
        description: "App de streaming da Amazon pré-instalado.",
        category: "third_party",
        recommended_action: RecommendedAction::Optional,
    },
    CuratedApp {
        name: "Facebook.Facebook",
        display_name: "Facebook",
        description: "App do Facebook pré-instalado por parceria.",
        category: "third_party",
        recommended_action: RecommendedAction::Remove,
    },
    CuratedApp {
        name: "Facebook.Instagram",
        display_name: "Instagram",
        description: "App do Instagram pré-instalado por parceria.",
        category: "third_party",
        recommended_action: RecommendedAction::Remove,
    },
    // ── Úteis (opcionais) ──
    CuratedApp {
        name: "Microsoft.Paint",
        display_name: "Paint",
        description: "Editor de imagem clássico. Leve e ocasionalmente útil.",
        category: "useful",
        recommended_action: RecommendedAction::Optional,
    },
    CuratedApp {
        name: "Microsoft.ScreenSketch",
        display_name: "Ferramenta de Captura",
        description: "Captura de tela do Windows (Win+Shift+S). Muito útil.",
        category: "useful",
        recommended_action: RecommendedAction::Keep,
    },
    CuratedApp {
        name: "Microsoft.WindowsCamera",
        display_name: "Câmera",
        description: "App de câmera do Windows. Útil se tem webcam.",
        category: "useful",
        recommended_action: RecommendedAction::Optional,
    },
    // ── Sistema (não remover) ──
    CuratedApp {
        name: "Microsoft.WindowsStore",
        display_name: "Microsoft Store",
        description: "Loja de apps. Necessária para instalar/atualizar apps UWP.",
        category: "system",
        recommended_action: RecommendedAction::Keep,
    },
    CuratedApp {
        name: "Microsoft.WindowsCalculator",
        display_name: "Calculadora",
        description: "Calculadora do Windows. Essencial.",
        category: "system",
        recommended_action: RecommendedAction::Keep,
    },
    CuratedApp {
        name: "Microsoft.WindowsTerminal",
        display_name: "Windows Terminal",
        description: "Terminal moderno. Essencial para uso avançado.",
        category: "system",
        recommended_action: RecommendedAction::Keep,
    },
    CuratedApp {
        name: "Microsoft.DesktopAppInstaller",
        display_name: "App Installer (winget)",
        description: "Gerenciador de pacotes winget. Necessário para instalações via CLI.",
        category: "system",
        recommended_action: RecommendedAction::Keep,
    },
    CuratedApp {
        name: "Microsoft.WindowsNotepad",
        display_name: "Bloco de Notas",
        description: "Editor de texto básico. Essencial para edição rápida.",
        category: "system",
        recommended_action: RecommendedAction::Keep,
    },
];

// ── Helpers ──────────────────────────────────────────────────────────────────

/// Parseia JSON que pode ser array ou objeto único (PowerShell retorna
/// objeto quando só tem 1 resultado).
fn parse_json_array<T: for<'de> Deserialize<'de>>(json_str: &str) -> Result<Vec<T>, String> {
    let trimmed = json_str.trim();
    if trimmed.is_empty() || trimmed == "null" {
        return Ok(Vec::new());
    }
    if trimmed.starts_with('[') {
        serde_json::from_str(trimmed).map_err(|e| format!("Erro ao parsear JSON array: {}", e))
    } else if trimmed.starts_with('{') {
        let single: T = serde_json::from_str(trimmed)
            .map_err(|e| format!("Erro ao parsear JSON objeto: {}", e))?;
        Ok(vec![single])
    } else {
        Err(format!(
            "Saída inesperada do PowerShell: {}",
            &trimmed[..trimmed.len().min(200)]
        ))
    }
}

// ── Comandos Tauri ───────────────────────────────────────────────────────────

/// Escaneia apps UWP instalados e cruza com a lista curada.
#[tauri::command]
pub async fn get_installed_uwp_apps() -> Result<Vec<UwpApp>, String> {
    tokio::task::spawn_blocking(|| {
        let script =
            "Get-AppxPackage | Select-Object Name, PackageFullName | ConvertTo-Json -Compress";

        let output = run_powershell(script)?;
        if !output.success {
            return Err(format!("Erro ao listar apps UWP: {}", output.stderr.trim()));
        }

        let ps_apps: Vec<PsAppxInfo> = parse_json_array(&output.stdout)?;

        let mut apps: Vec<UwpApp> = Vec::with_capacity(CURATED_APPS.len());

        for curated in CURATED_APPS {
            let installed = ps_apps.iter().find(|a| a.name == curated.name);
            apps.push(UwpApp {
                name: curated.name.to_string(),
                package_full_name: installed
                    .map(|a| a.package_full_name.clone())
                    .unwrap_or_default(),
                display_name: curated.display_name.to_string(),
                description: curated.description.to_string(),
                category: curated.category.to_string(),
                recommended_action: match curated.recommended_action {
                    RecommendedAction::Remove => RecommendedAction::Remove,
                    RecommendedAction::Optional => RecommendedAction::Optional,
                    RecommendedAction::Keep => RecommendedAction::Keep,
                },
                is_installed: installed.is_some(),
            });
        }

        Ok(apps)
    })
    .await
    .map_err(|e| format!("Erro interno: {}", e))?
}

/// Remove apps UWP selecionados. Valida contra lista curada e bloqueia apps "Keep".
#[tauri::command]
pub async fn remove_uwp_apps(names: Vec<String>) -> Result<RemovalResult, String> {
    let total_requested = names.len();

    tokio::task::spawn_blocking(move || {
        let mut result = RemovalResult {
            succeeded: Vec::new(),
            failed: Vec::new(),
            total_requested,
        };

        for name in &names {
            // Validar: deve estar na lista curada
            let curated = match CURATED_APPS.iter().find(|a| a.name == name.as_str()) {
                Some(c) => c,
                None => {
                    result.failed.push(RemovalError {
                        name: name.clone(),
                        display_name: name.clone(),
                        error: "App não está na lista curada".into(),
                    });
                    continue;
                }
            };

            // Validar: não pode ser "Keep"
            if curated.recommended_action == RecommendedAction::Keep {
                result.failed.push(RemovalError {
                    name: name.clone(),
                    display_name: curated.display_name.to_string(),
                    error: "App protegido — não pode ser removido".into(),
                });
                continue;
            }

            // Remover via PowerShell
            let script = format!(
                "Get-AppxPackage -Name '{}' | Remove-AppxPackage -ErrorAction Stop",
                name
            );

            match run_powershell(&script) {
                Ok(o) if o.success => {
                    result.succeeded.push(curated.display_name.to_string());
                }
                Ok(o) => {
                    let stderr = o.stderr.trim().to_string();
                    // App já removido não é erro
                    if stderr.contains("not found")
                        || stderr.contains("No installed package")
                        || o.stdout.trim().is_empty() && stderr.is_empty()
                    {
                        result.succeeded.push(curated.display_name.to_string());
                    } else {
                        result.failed.push(RemovalError {
                            name: name.clone(),
                            display_name: curated.display_name.to_string(),
                            error: format!("Falha: {}", stderr),
                        });
                    }
                }
                Err(e) => {
                    result.failed.push(RemovalError {
                        name: name.clone(),
                        display_name: curated.display_name.to_string(),
                        error: e,
                    });
                }
            }
        }

        Ok(result)
    })
    .await
    .map_err(|e| format!("Erro interno: {}", e))?
}
