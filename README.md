<p align="center">
  <img src="src/midia/frameguard_icon_500px.png" alt="FrameGuard" width="120">
</p>

<h1 align="center">FrameGuard</h1>

<p align="center">
  Utilitário de manutenção e otimização para <strong>Windows 11</strong>, feito por gamers, para gamers.<br>
  Interface moderna, backend nativo em Rust e zero telemetria.
</p>

<p align="center">
  <img src="https://img.shields.io/badge/Windows_11-0078D6?style=for-the-badge&logo=windows&logoColor=white" alt="Windows 11">
  <img src="https://img.shields.io/badge/Tauri_v2-FFC131?style=for-the-badge&logo=tauri&logoColor=white" alt="Tauri v2">
  <img src="https://img.shields.io/badge/React_19-61DAFB?style=for-the-badge&logo=react&logoColor=black" alt="React 19">
  <img src="https://img.shields.io/badge/Rust-000000?style=for-the-badge&logo=rust&logoColor=white" alt="Rust">
  <img src="https://img.shields.io/badge/License-GPL_v3-blue?style=for-the-badge" alt="GPL v3">
</p>

<p align="center">
  <a href="https://github.com/marcelopepis/FrameGuard/releases">Download</a> ·
  <a href="#screenshots">Screenshots</a> ·
  <a href="#features">Features</a> ·
  <a href="#instalação">Instalação</a> ·
  <a href="#roadmap">Roadmap</a>
</p>

---

## O que é o FrameGuard?

O FrameGuard é uma ferramenta **100% gratuita e open-source** que reúne otimizações, manutenção e limpeza do Windows 11 em um único lugar.

Sem versão PRO, sem funcionalidade trancada atrás de paywall, sem telemetria, sem ads. Todo o código é aberto, auditável, e sempre será.

Cada tweak tem classificação de evidência (comprovado, plausível ou não comprovado), detalhes técnicos completos e reversão com um clique. Você decide o que aplica, sabendo exatamente o que vai acontecer no seu sistema.

## Screenshots

<p align="center">
  <img src="src/midia/dashboard.png" alt="Dashboard" width="800"><br>
  <em>Dashboard — Visão geral do hardware, status do sistema e planos rápidos</em>
</p>

<p align="center">
  <img src="src/midia/otimizacoes.png" alt="Otimizações" width="800"><br>
  <em>Otimizações — 21 tweaks organizados por categoria com níveis de risco e evidência</em>
</p>

<p align="center">
  <img src="src/midia/privacidade.png" alt="Privacidade" width="800"><br>
  <em>Privacidade — Controle de telemetria, remoção de bloatware UWP e apps em segundo plano</em>
</p>

<p align="center">
  <img src="src/midia/manutencao.png" alt="Manutenção" width="800"><br>
  <em>Manutenção — DISM, SFC, chkdsk, TRIM e flush DNS com progresso em tempo real</em>
</p>

## Features

**Otimizações**
- 21 tweaks de GPU, CPU, rede, armazenamento e timers com classificação de evidência
- Filtro automático por hardware — detecta GPU (NVIDIA/AMD/Intel) e CPU, exibe apenas tweaks compatíveis
- Backup automático do valor original antes de cada alteração; reversão com um clique

**Privacidade**
- 4 tweaks de privacidade e telemetria (registro + política de grupo)
- Remoção de bloatware UWP em batch com lista curada de 41 apps
- Controle de apps em segundo plano

**Manutenção**
- DISM (CheckHealth, ScanHealth, RestoreHealth), SFC, chkdsk, SSD TRIM, flush DNS
- Streaming de progresso em tempo real para cada operação

**Limpeza**
- Scan categorizado: temporários do sistema, GPU shader cache, browser cache, cache de apps, avançado (WinSxS)
- Seleção granular por item com detecção de file locks (Restart Manager API)

**Serviços**
- 33 serviços e 8 tarefas agendadas curados para gaming
- Desabilitar/restaurar com um clique

**Planos de Execução**
- Combine múltiplos tweaks em rotinas reutilizáveis
- 4 planos oficiais incluídos: Manutenção Básica, Saúde Completa, Otimização Gaming, Privacidade e Debloat
- Criação de planos personalizados

**Mais**
- Busca global (`Ctrl+K`) para encontrar qualquer tweak, ação ou plano
- Ponto de restauração automático antes de planos (configurável)
- Export/import de configurações em arquivo `.fg` (JSON legível)
- Página educacional desmistificando otimizações "snake oil"
- Verificação de atualizações via GitHub Releases

## Requisitos

- Windows 11 (x64)
- Privilégios de administrador (elevação via UAC automática)

## Instalação

### Download direto

Baixe o instalador `.exe` mais recente na página de [Releases](https://github.com/marcelopepis/FrameGuard/releases).

### Build local

```bash
# Pré-requisitos: Node.js 20+, Rust toolchain, Visual Studio Build Tools
git clone https://github.com/marcelopepis/FrameGuard.git
cd FrameGuard
npm install
npm run tauri build
```

O instalador NSIS será gerado em `src-tauri/target/release/bundle/nsis/`.

## Roadmap

O FrameGuard está em desenvolvimento ativo. Próximos passos planejados:

**v1.1**
- Tema claro sincronizado com a preferência do Windows
- Revisão de tweaks: adicionar novos com evidência e remover os que se provarem ineficazes
- Animações e transições refinadas

**v1.2**
- Refactoring de módulos grandes (optimizations.rs, cleanup.rs, Plans.tsx)
- Mais mitos e explicações na página educacional
- Recomendações por perfil de hardware

**v2.0**
- Publicação na Microsoft Store
- Auto-updater nativo
- Perfis de otimização compartilháveis pela comunidade

Sugestões são bem-vindas — abra uma [Issue](https://github.com/marcelopepis/FrameGuard/issues/new/choose).

## Desenvolvimento

```bash
npm run dev          # Vite dev server + Tauri dev (hot reload)
npm run build        # Build de produção (tsc + vite + cargo)
npm run tauri build  # Gera instalador NSIS
```

### Stack

| Camada | Tecnologia | Versão |
|--------|-----------|--------|
| Frontend | React + TypeScript (Vite) | React 19, Vite 7, TS 5.8 |
| Backend | Tauri v2 + Rust | Tauri 2, Edition 2021 |
| Ícones | lucide-react | 0.564+ |
| Roteamento | react-router-dom | 7.13+ |
| Registro | winreg | 0.55 |
| Sistema | sysinfo | 0.33 |

### Estrutura do projeto

```
FrameGuard/
├── src/                    # Frontend React/TypeScript
│   ├── components/         # ActionCard, Layout, SearchBar, Toast, WelcomeModal
│   ├── contexts/           # RunningContext, ToastContext
│   ├── hooks/              # useActionRunner, useHardwareFilter, usePlanExecution
│   ├── pages/              # 10 páginas (Dashboard, Optimizations, Privacy, ...)
│   └── midia/              # Screenshots e assets visuais
├── src-tauri/              # Backend Rust
│   ├── src/commands/       # Comandos Tauri (system_info, optimizations, cleanup, ...)
│   └── src/utils/          # Backup, plan_manager, activity_log, command_runner, ...
└── CLAUDE.md               # Guia de arquitetura e desenvolvimento
```

## Segurança

- Elevação de administrador via `manifest.xml` (UAC nativo do Windows)
- Backup automático de valores originais antes de qualquer modificação
- Detecção de file locks via Restart Manager API
- **Zero telemetria** — nenhuma conexão externa exceto verificação de updates no GitHub
- Configurações salvas em JSON legível em `%APPDATA%\FrameGuard`

## Contribuindo

Pull requests são bem-vindos. Consulte o [CLAUDE.md](CLAUDE.md) para detalhes sobre arquitetura e convenções.

Para sugerir um novo tweak, use o [template de sugestão](https://github.com/marcelopepis/FrameGuard/issues/new?template=tweak_suggestion.yml). O FrameGuard só inclui tweaks com evidência real de impacto.

## Licença

Este projeto está licenciado sob a [GNU General Public License v3.0](LICENSE).

---

<p align="center">
  O FrameGuard é e sempre será <strong>100% gratuito</strong> — sem versão PRO, sem paywall, sem pegadinha.<br>
  Se o projeto te ajudou e você quiser contribuir com qualquer valor, o PIX está aberto.<br>
  Totalmente opcional — o app funciona igual pra todo mundo.
</p>

<p align="center">
  <img src="src/midia/qrcode.png" alt="PIX" width="160">
</p>

<p align="center">
  Feito por <a href="https://github.com/marcelopepis"><strong>Marcelo Pepis</strong></a>
</p>

<p align="center">
  <a href="https://github.com/marcelopepis"><img src="https://img.shields.io/badge/GitHub-181717?style=for-the-badge&logo=github&logoColor=white" alt="GitHub"></a>
  <a href="https://www.linkedin.com/in/marcelo-pepis/"><img src="https://img.shields.io/badge/LinkedIn-0A66C2?style=for-the-badge&logo=linkedin&logoColor=white" alt="LinkedIn"></a>
  <a href="https://www.instagram.com/marcelo_pepis/"><img src="https://img.shields.io/badge/Instagram-E4405F?style=for-the-badge&logo=instagram&logoColor=white" alt="Instagram"></a>
</p>
