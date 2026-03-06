use crate::commands::optimizations::{
    backup_info, EvidenceLevel, HardwareFilter, RiskLevel, TweakInfo,
};

/// Metadados estáticos de um tweak — tudo que é constante e não depende do estado do sistema.
///
/// Separa os campos invariantes (nome, descrição, risco, evidência) dos campos dinâmicos
/// (`is_applied`, `has_backup`, `last_applied`) que mudam conforme o estado atual do Windows.
///
/// Cada tweak declara um `const TweakMeta` e chama [`TweakMeta::build`] passando apenas
/// o booleano `is_applied`, eliminando o boilerplate repetido em cada `get_X_info()`.
pub struct TweakMeta {
    /// Identificador único em snake_case (ex: `"disable_wallpaper_compression"`)
    pub id: &'static str,
    /// Nome legível exibido na UI
    pub name: &'static str,
    /// Descrição detalhada do efeito para o usuário final
    pub description: &'static str,
    /// Categoria para agrupamento na UI (ex: `"optimization"`, `"privacy"`)
    pub category: &'static str,
    /// `true` se a mudança só tem efeito após reinicialização do Windows
    pub requires_restart: bool,
    /// Nível de risco do tweak
    pub risk_level: RiskLevel,
    /// Grau de evidência técnica que sustenta o benefício declarado
    pub evidence_level: EvidenceLevel,
    /// Descrição do valor padrão do Windows para exibição no botão "Restaurar Padrão"
    pub default_value_description: &'static str,
    /// Filtro de hardware: `None` = universal, `Some(...)` = vendor-specific
    pub hardware_filter: Option<HardwareFilter>,
}

impl TweakMeta {
    /// Constrói um [`TweakInfo`] completo combinando os metadados estáticos com o estado dinâmico.
    ///
    /// O único parâmetro dinâmico obrigatório é `is_applied` (o tweak está ativo no sistema?).
    /// Os campos `has_backup` e `last_applied` são resolvidos automaticamente via
    /// [`backup_info`], consultando `backups.json`.
    pub fn build(&self, is_applied: bool) -> TweakInfo {
        let (has_backup, last_applied) = backup_info(self.id);

        TweakInfo {
            id: self.id.to_string(),
            name: self.name.to_string(),
            description: self.description.to_string(),
            category: self.category.to_string(),
            is_applied,
            requires_restart: self.requires_restart,
            last_applied,
            has_backup,
            risk_level: self.risk_level.clone(),
            evidence_level: self.evidence_level.clone(),
            default_value_description: self.default_value_description.to_string(),
            hardware_filter: self.hardware_filter.clone(),
        }
    }
}
