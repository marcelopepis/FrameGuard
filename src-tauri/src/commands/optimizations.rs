// Toggles de otimização (VBS, Game Mode, HAGS, etc.)
use serde::Serialize;

/// Estado atual de uma otimização
#[derive(Debug, Serialize)]
pub struct OptimizationStatus {
    pub id: String,
    pub name: String,
    pub description: String,
    pub enabled: bool,
}

/// Retorna o estado de todas as otimizações disponíveis
#[tauri::command]
pub fn get_optimizations() -> Result<Vec<OptimizationStatus>, String> {
    // TODO: implementar leitura real do registro
    let optimizations = vec![
        OptimizationStatus {
            id: "vbs".to_string(),
            name: "Virtualization Based Security".to_string(),
            description: "Desabilita VBS/HVCI para melhor performance em jogos".to_string(),
            enabled: false,
        },
        OptimizationStatus {
            id: "game_mode".to_string(),
            name: "Game Mode".to_string(),
            description: "Ativa o modo de jogo do Windows para priorizar jogos".to_string(),
            enabled: false,
        },
        OptimizationStatus {
            id: "hags".to_string(),
            name: "Hardware Accelerated GPU Scheduling".to_string(),
            description: "Ativa agendamento de GPU por hardware".to_string(),
            enabled: false,
        },
        OptimizationStatus {
            id: "fullscreen_optimizations".to_string(),
            name: "Fullscreen Optimizations".to_string(),
            description: "Desabilita otimizações de tela cheia do Windows".to_string(),
            enabled: false,
        },
    ];

    Ok(optimizations)
}

/// Alterna o estado de uma otimização específica
#[tauri::command]
pub fn toggle_optimization(id: String, enable: bool) -> Result<bool, String> {
    // TODO: implementar escrita real no registro
    println!("Toggling optimization '{}' to {}", id, enable);
    Ok(enable)
}
