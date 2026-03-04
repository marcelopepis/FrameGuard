<p align="center">
  <img src="https://img.shields.io/badge/Windows_11-0078D6?style=for-the-badge&logo=windows&logoColor=white" alt="Windows 11">
  <img src="https://img.shields.io/badge/Tauri_v2-FFC131?style=for-the-badge&logo=tauri&logoColor=white" alt="Tauri v2">
  <img src="https://img.shields.io/badge/React_19-61DAFB?style=for-the-badge&logo=react&logoColor=black" alt="React 19">
  <img src="https://img.shields.io/badge/Rust-000000?style=for-the-badge&logo=rust&logoColor=white" alt="Rust">
  <img src="https://img.shields.io/badge/License-GPL_v3-blue?style=for-the-badge" alt="GPL v3">
</p>

# FrameGuard

Utilitario de manutencao e otimizacao para **Windows 11**, voltado para gamers. Interface moderna com tema escuro, backend nativo em Rust e zero dependencias de runtime.

## Features

- **21 tweaks de otimizacao** -- GPU, CPU, rede, armazenamento, timers, com niveis de risco e evidencia
- **Filtro por hardware** -- detecta GPU (NVIDIA/AMD/Intel) e CPU automaticamente, mostra apenas tweaks compativeis
- **4 tweaks de privacidade** + remocao de bloatware UWP em batch
- **Manutencao do sistema** -- DISM, SFC, chkdsk, SSD TRIM, flush DNS com streaming de progresso em tempo real
- **Limpeza categorizada** -- temporarios, shader cache, browser cache, cache de apps
- **Gerenciamento de servicos** -- 33 servicos e 8 tarefas agendadas curados para gaming
- **Planos de execucao** -- combine multiplos tweaks em rotinas reutilizaveis (4 planos oficiais incluidos)
- **Backup automatico** -- todo tweak salva o valor original antes de alterar; reversao com um clique
- **Ponto de restauracao** -- criacao automatica antes de tweaks/planos (configuravel)
- **Export/Import** -- salve e restaure todas as configuracoes em arquivo `.fg`
- **Busca global** -- `Ctrl+K` para encontrar qualquer tweak, acao ou plano
- **Pagina educacional** -- desmistifica otimizacoes "snake oil" com explicacoes baseadas em evidencias
- **Verificacao de atualizacoes** -- consulta GitHub Releases automaticamente

## Screenshots

> _Em breve_

## Requisitos

- Windows 11 (x64)
- Privilegios de administrador (elevacao via UAC automatica)

## Instalacao

### Download

Baixe o instalador `.exe` mais recente na pagina de [Releases](https://github.com/marcelopepis/FrameGuard/releases).

### Build local

```bash
# Pre-requisitos: Node.js 20+, Rust toolchain, Visual Studio Build Tools
git clone https://github.com/marcelopepis/FrameGuard.git
cd FrameGuard
npm install
npm run tauri build
```

O instalador NSIS sera gerado em `src-tauri/target/release/bundle/nsis/`.

## Desenvolvimento

```bash
npm run dev          # Vite dev server + Tauri dev (hot reload)
npm run build        # Build de producao (tsc + vite + cargo)
npm run tauri build  # Gera instalador NSIS
```

### Stack

| Camada | Tecnologia | Versao |
|--------|-----------|--------|
| Frontend | React + TypeScript (Vite) | React 19, Vite 7, TS 5.8 |
| Backend | Tauri v2 + Rust | Tauri 2, Edition 2021 |
| Icones | lucide-react | 0.564+ |
| Roteamento | react-router-dom | 7.13+ |
| Registro | winreg | 0.55 |
| Sistema | sysinfo | 0.33 |

### Estrutura

```
FrameGuard/
├── src/                    # Frontend React/TypeScript
│   ├── components/         # ActionCard, Layout, SearchBar, Toast, WelcomeModal
│   ├── contexts/           # RunningContext, ToastContext
│   ├── hooks/              # useActionRunner, useHardwareFilter, usePlanExecution
│   ├── pages/              # 10 paginas (Dashboard, Optimizations, Privacy, ...)
│   └── styles/             # CSS vars, tema escuro, glassmorphism
├── src-tauri/              # Backend Rust
│   ├── src/commands/       # Comandos Tauri (system_info, optimizations, health_check, ...)
│   └── src/utils/          # Registry, backup, plan_manager, activity_log, ...
└── CLAUDE.md               # Guia de desenvolvimento
```

## Seguranca

- Elevacao de administrador via `manifest.xml` (UAC nativo do Windows)
- Backup automatico de valores originais antes de qualquer modificacao no registro
- Deteccao de file locks via Restart Manager API
- Sem credenciais hardcoded, sem telemetria, sem conexoes externas (exceto GitHub Releases)

## Contribuindo

1. Fork o repositorio
2. Crie uma branch (`git checkout -b feature/minha-feature`)
3. Commit suas alteracoes (`git commit -m 'feat: minha feature'`)
4. Push para a branch (`git push origin feature/minha-feature`)
5. Abra um Pull Request

Consulte o [CLAUDE.md](CLAUDE.md) para detalhes sobre arquitetura, convencoes e checklists de implementacao.

## Licenca

Este projeto esta licenciado sob a [GNU General Public License v3.0](LICENSE).

## Autor

**Marcelo Pepis** -- [@marcelopepis](https://github.com/marcelopepis)
