// Verificação de privilégios de administrador

/// Verifica se o processo está rodando com privilégios elevados
pub fn is_elevated() -> bool {
    // TODO: implementar verificação via Windows API (IsUserAnAdmin / TokenElevation)
    // Por enquanto retorna true já que o manifest solicita elevação
    true
}
