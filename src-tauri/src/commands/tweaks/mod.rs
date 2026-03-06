//! Submódulos de tweaks organizados por categoria.
//!
//! Cada módulo agrupa tweaks relacionados (constantes, status, get_info, apply, revert)
//! e re-exporta tudo publicamente para manter compatibilidade com o resto do codebase.

pub mod gaming;
pub mod gpu;
pub mod network;
pub mod power;
pub mod storage;
pub mod visual;

pub use gaming::*;
pub use gpu::*;
pub use network::*;
pub use power::*;
pub use storage::*;
pub use visual::*;
