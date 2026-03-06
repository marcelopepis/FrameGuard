# FrameGuard - Guia de Desenvolvimento

Utilitário de manutenção e otimização Windows 11 para gamers.

Informações que o Claude Code precisa saber ao executar qualquer prompt:

- Stack: Tauri (Rust backend) + React (TypeScript frontend)
- Estrutura: Backend em `src-tauri/src/`, Frontend em `src/`
- Padrões existentes: OnceLock<Mutex<T>>, comandos async com spawn_blocking, CSS Modules, ícones Lucide
- Renderização condicional: App.tsx renderiza APENAS a página ativa (sem keep-alive). Páginas carregam dados somente quando navegadas pela primeira vez (lazy loading via useRef)
- Planos built-in: IDs prefixados com "builtin_" e definidos em plan_manager.rs
- Persistência: JSON em %APPDATA%\FrameGuard/ (plans.json, backups.json, activity_log.json)
- O app roda em Windows PT-BR — detecção baseada em texto deve ser agnóstica de idioma
- Correções recentes: activity log, dashboard cache TTL/OnceLock, file locks (Restart Manager API), comandos async com spawn_blocking, detecção de GPU via registro direto (sem PowerShell), Power Plan por GUID (cross-locale)
- Features recentes: WelcomeModal (primeira execução), busca global na sidebar (Ctrl+K), filtro por hardware/vendor, remoção de bloatware UWP, ponto de restauração automático, página de cleanup categorizado, página educacional (Learn), tema dark/light
- Refatorações recentes: tweaks do backend divididos em submódulos por categoria (`commands/tweaks/`), health_check dividido em submódulos (`commands/health/`), `TweakMeta` builder (`utils/tweak_builder.rs`), `tweakRegistry.ts` centralizado no frontend, hooks extraídos (`useTweakPage`, `useDashboardData`), componentes Dashboard extraídos, CSS tokens extraídos em `tokens.css`

Ao gerar código novo:
- Rust: usar spawn_blocking para operações que envolvem PowerShell, WMI ou registry
- Rust: usar `TweakMeta` em `utils/tweak_builder.rs` para metadados estáticos de tweaks novos
- Rust: tweaks de otimização vão em `commands/tweaks/{categoria}.rs`, não em `optimizations.rs`
- React: CSS Modules (.module.css) para estilos, Lucide para ícones
- React: registrar tweaks novos no `tweakRegistry.ts` (fonte única de verdade para IDs/comandos)
- Garantir que qualquer novo comando Tauri seja registrado em src-tauri/src/lib.rs

## Stack Tecnológico

| Camada      | Tecnologia                          | Versão  |
|-------------|-------------------------------------|---------|
| Frontend    | React + TypeScript (Vite)           | React 19, Vite 7, TS 5.8 |
| Backend     | Tauri v2 + Rust                     | Tauri 2, Edition 2021 |
| UI Icons    | lucide-react                        | 0.564+  |
| Roteamento  | react-router-dom                    | 7.13+   |
| Diálogos    | @tauri-apps/plugin-dialog           | 2.6+    |
| Registro    | winreg                              | 0.55    |
| Sistema     | sysinfo                             | 0.33    |
| Async       | tokio (feature: rt)                 | 1       |
| Serialização| serde + serde_json                  | 1       |
| Datas       | chrono (features: std, clock, serde)| 0.4     |
| IDs         | uuid (features: v4, serde)          | 1       |

## Arquitetura Geral

```
┌─────────────────────────────────────────────────────┐
│  Frontend (React + TS)                              │
│  ┌──────────┐ ┌──────────┐ ┌──────────────────────┐│
│  │ Contexts  │ │  Hooks    │ │     10 Pages         ││
│  │(Running,  │ │(Action,   │ │ Dashboard, Optim,    ││
│  │ Toast)    │ │ Plan,     │ │ Privacy, Maint,      ││
│  └──────────┘ │ HwFilter, │ │ Cleanup, Services,   ││
│               │ Search,   │ │ Plans, Learn, About,  ││
│               │ TweakPage,│ │ Settings              ││
│               │ Dashboard)│ └──────────────────────┘│
│               └──────────┘                          │
│                             └──────────────────────┘│
│          invoke() ↕ listen()                        │
├─────────────────────────────────────────────────────┤
│  Backend (Rust)                                     │
│  ┌──────────────┐  ┌──────────────────────────────┐ │
│  │  Commands     │  │  Utils                       │ │
│  │  (system_info,│  │  (registry, command_runner,  │ │
│  │   tweaks/,    │  │   backup, plan_manager,      │ │
│  │   health/,    │  │   activity_log, file_locks,  │ │
│  │   privacy,    │  │   restore_point, wmi,        │ │
│  │   cleanup,    │  │   tweak_builder)             │ │
│  │   bloatware,  │  └──────────────────────────────┘ │
│  │   restore_pt, │                                   │
│  │   about)      │                                   │
│  └──────────────┘                                    │
│          ↕ Win32 API / PowerShell / Registry        │
├─────────────────────────────────────────────────────┤
│  Windows 11 (Elevação Admin via manifest.xml)       │
└─────────────────────────────────────────────────────┘
```

## Estrutura de Diretórios

```
FrameGuard/
├── src/                           # Frontend React/TS
│   ├── App.tsx                    # Router + renderização condicional (apenas página ativa montada)
│   ├── main.tsx                   # Entry point
│   ├── components/
│   │   ├── ActionCard/            # Card de ação com progresso, logs, resultado
│   │   ├── BloatwareSection/      # Seção de remoção de apps UWP (usado em Privacy)
│   │   ├── Dashboard/             # Subcomponentes extraídos do Dashboard
│   │   │   ├── ActivityItem.tsx   # Item de atividade recente
│   │   │   ├── QuickPlanCard.tsx  # Card de plano rápido
│   │   │   ├── StatusBadges.tsx   # Badges de status (Admin, GameDVR, PowerPlan)
│   │   │   └── QuickExecModal.tsx # Modal de execução rápida de plano
│   │   ├── Layout/                # Layout principal (Sidebar + content + SearchBar)
│   │   ├── SearchBar/             # Busca global na sidebar (Ctrl+K)
│   │   ├── Toast/                 # Notificações toast (portal)
│   │   ├── WelcomeModal/          # Modal de boas-vindas (primeira execução)
│   │   ├── TweakCard.tsx          # Card de tweak com apply/revert/restore
│   │   └── index.ts
│   ├── contexts/
│   │   ├── RunningContext.tsx      # Estado global de execução (Set<string>)
│   │   └── ToastContext.tsx        # Fila de toasts (max 3)
│   ├── data/
│   │   ├── searchIndex.ts         # Índice estático para busca global (tweaks, ações, planos)
│   │   └── tweakRegistry.ts       # Registro centralizado de tweaks (IDs, comandos, categorias)
│   ├── hooks/
│   │   ├── useActionRunner.ts     # Execução de ações com streaming
│   │   ├── useDashboardData.ts    # Estados e fetches do Dashboard (extraído)
│   │   ├── useHardwareFilter.ts   # Filtragem de tweaks por vendor (GPU/CPU)
│   │   ├── usePlanExecution.ts    # Execução de planos com progresso por item
│   │   ├── useSearchHighlight.ts  # Scroll + highlight de itens encontrados via busca
│   │   ├── useTweakPage.ts        # Lógica compartilhada de páginas de tweaks (Optim/Privacy)
│   │   └── index.ts
│   ├── pages/
│   │   ├── Dashboard.tsx          # Hardware, status, atividade recente, planos rápidos
│   │   ├── Optimizations.tsx      # 21 tweaks em 6 categorias (com filtro de hardware)
│   │   ├── Privacy.tsx            # 4 tweaks de privacidade + remoção de bloatware UWP
│   │   ├── Maintenance.tsx        # DISM, SFC, chkdsk, SSD trim, DNS
│   │   ├── Cleanup.tsx            # Limpeza categorizada (temp, shader, browser, apps)
│   │   ├── Services.tsx           # Serviços e tarefas agendadas
│   │   ├── Plans.tsx              # CRUD de planos de execução
│   │   ├── Learn.tsx              # Página educacional (mitos e snake oil)
│   │   ├── About.tsx              # Versão, créditos, verificação de atualizações
│   │   └── Settings.tsx           # Export/import, backups, config
│   ├── services/
│   │   └── systemInfo.ts          # Wrappers invoke() para system info + getDetectedVendors
│   ├── styles/
│   │   ├── tokens.css             # Design tokens (cores dark/light, accent, status, border)
│   │   └── globals.css            # Reset, scrollbar, utilitários globais
│   ├── types/
│   │   ├── health.ts              # Interfaces compartilhadas (HealthCheckResult, etc.)
│   │   ├── cleanup.ts             # Tipos do sistema de cleanup (CleanupItem, CleanupCategory)
│   │   └── index.ts
│   └── utils/
│       ├── formatters.ts          # formatDuration, formatDate, formatSpaceFreed
│       ├── restorePoint.ts        # Lógica centralizada de ponto de restauração (cache 24h)
│       └── theme.ts               # Gerenciamento de tema dark/light (localStorage)
│
├── src-tauri/                     # Backend Rust
│   ├── src/
│   │   ├── main.rs                # Entry point binário
│   │   ├── lib.rs                 # Setup Tauri + registro de 121 comandos
│   │   ├── commands/
│   │   │   ├── mod.rs             # Declaração de módulos
│   │   │   ├── system_info.rs     # HW info (cache), status (TTL 5s), usage, summary, vendors
│   │   │   ├── optimizations.rs   # Tipos TweakInfo, RiskLevel, EvidenceLevel + helpers
│   │   │   ├── tweaks/            # Tweaks organizados por categoria
│   │   │   │   ├── mod.rs         # Re-exporta todos os submódulos
│   │   │   │   ├── gaming.rs      # Game Mode, VBS, Timer Resolution, Mouse Accel, FSO
│   │   │   │   ├── gpu.rs         # HAGS, Game DVR, Xbox Overlay, MSI Mode, MPO, NVIDIA Telemetry
│   │   │   │   ├── network.rs     # Delivery Optimization, Nagle
│   │   │   │   ├── power.rs       # Ultimate Performance, Power Throttling, Hibernation
│   │   │   │   ├── storage.rs     # Reserved Storage, NTFS Last Access
│   │   │   │   └── visual.rs      # Wallpaper Compression, Sticky Keys, Bing Search
│   │   │   ├── privacy.rs         # 4 tweaks de privacidade
│   │   │   ├── health/            # Ações de saúde do sistema (split de health_check.rs)
│   │   │   │   ├── mod.rs         # Tipos (HealthCheckResult, CheckStatus) + helpers compartilhados
│   │   │   │   ├── dism.rs        # DISM Cleanup, CheckHealth, ScanHealth, RestoreHealth
│   │   │   │   ├── disk.rs        # SFC, chkdsk, SSD TRIM
│   │   │   │   └── maintenance.rs # Flush DNS, limpeza de temporários, kill process
│   │   │   ├── cleanup.rs         # scan_cleanup, execute_cleanup (categorizado)
│   │   │   ├── bloatware.rs       # get_installed_uwp_apps, remove_uwp_apps
│   │   │   ├── restore_point.rs   # create_restore_point (via PowerShell)
│   │   │   ├── plans.rs           # CRUD + execute_plan (emite plan_progress)
│   │   │   ├── services.rs        # 33 serviços + 8 tarefas curadas
│   │   │   ├── activity.rs        # log_tweak_activity, get_recent_activity
│   │   │   ├── about.rs           # check_for_updates (GitHub API)
│   │   │   └── export_import.rs   # Export/import .fg (JSON), validate_fg_file
│   │   └── utils/
│   │       ├── mod.rs
│   │       ├── registry.rs        # read/write DWORD/STRING, delete, key_exists (HKCU/HKLM)
│   │       ├── command_runner.rs   # run_command, run_command_with_progress, run_powershell
│   │       ├── backup.rs          # Backup/restore tweaks (%APPDATA%\FrameGuard\backups.json)
│   │       ├── plan_manager.rs    # CRUD planos + 4 built-in (%APPDATA%\FrameGuard\plans.json)
│   │       ├── activity_log.rs    # FIFO max 100 (%APPDATA%\FrameGuard\activity_log.json)
│   │       ├── file_locks.rs      # Restart Manager API — detecta processos travando arquivos
│   │       ├── restore_point.rs   # Criação de ponto de restauração Windows
│   │       ├── tweak_builder.rs   # TweakMeta — metadados estáticos de tweaks (reduz boilerplate)
│   │       ├── wmi.rs             # Queries WMI
│   │       └── elevated.rs        # is_elevated() via OpenProcessToken
│   ├── Cargo.toml
│   ├── tauri.conf.json            # Janela 1100x700 (min 900x600), tema Dark, NSIS perMachine
│   ├── manifest.xml               # requireAdministrator (UAC)
│   └── build.rs                   # tauri_build com manifest.xml
│
├── package.json                   # Scripts: dev, build, preview, tauri
├── vite.config.ts                 # Port 1420, HMR 1421
├── tsconfig.json                  # ES2020, strict, react-jsx
├── index.html                     # lang="pt-BR", mount #root
└── CLAUDE.md                      # Este arquivo
```

## Variáveis de Ambiente e Configuração

### Dev Server
- `TAURI_DEV_HOST` — hostname customizado para dev (opcional)
- Vite dev: `http://localhost:1420`
- HMR fallback: porta `1421`

### Persistência (Backend — `%APPDATA%\FrameGuard\`)
| Arquivo              | Conteúdo                                  | Limite       |
|----------------------|-------------------------------------------|--------------|
| `backups.json`       | Backup de tweaks (original_value, status)  | Ilimitado    |
| `plans.json`         | Planos user + 4 built-in (v2)             | Ilimitado    |
| `activity_log.json`  | Log de atividades recentes                | 100 entries  |

### localStorage (Frontend)
| Chave                      | Conteúdo                          |
|----------------------------|-----------------------------------|
| `frameguard:health:{id}`   | Último HealthCheckResult por ação |
| `frameguard:cleanup:{id}`  | Último resultado de cleanup       |
| `fg.firstRunSeen`          | Flag de primeira execução (WelcomeModal) |
| `fg.restorePoint`          | Preferência de ponto de restauração automático |
| `fg-theme`                 | Tema ativo: `"dark"` ou `"light"` (padrão: dark) |

### IDs dos Planos Built-in
- `builtin_manutencao_basica` — Manutenção básica
- `builtin_saude_completa` — Saúde completa
- `builtin_otimizacao_gaming` — Otimização gaming
- `builtin_privacidade_debloat` — Privacidade & debloat

## Convenções de Código

### Nomenclatura
- Comandos Rust: `snake_case` → Frontend: `camelCase`
- Comentários e textos UI: **português (pt-BR)**
- Erros: `Result<T, String>` (planejado refatorar para custom error types)

### Padrões Críticos

**Async obrigatório para comandos longos:**
```rust
// DISM, SFC, cleanup → SEMPRE async + spawn_blocking
#[tauri::command]
pub async fn run_dism_checkhealth(app: AppHandle) -> Result<HealthCheckResult, String> {
    tokio::task::spawn_blocking(move || { /* ... */ }).await
}
```

**Encoding PT-BR (DISM/SFC):**
```rust
// Executar via PowerShell para forçar UTF-8
powershell.exe -Command "[Console]::OutputEncoding = [System.Text.Encoding]::UTF8; dism.exe ..."
```

**Status DISM em português:**
- "reparável" = repairable
- "não está danificado" = ok
- "corrompido" / "danificado" = corrupted

**Streaming de progresso:**
```rust
// Emitir eventos em tempo real via Tauri
run_command_with_progress(&app, "event_channel", cmd, args, display_label)
// Frontend escuta: listen<CommandEvent>("event_channel", callback)
```

**Buffer de output no frontend:**
```typescript
// useActionRunner flush a cada 80ms para evitar freeze
// Máximo 500 linhas de log no DOM
```

### Padrão de Tweak (Backend)
Cada tweak segue o padrão:
1. Declarar `const META: TweakMeta` com metadados estáticos (usa `utils/tweak_builder.rs`)
2. `get_{tweak}_info()` → `META.build(is_applied)` retorna `TweakInfo` completo
3. `disable_{tweak}()` ou `enable_{tweak}()` → aplica + cria backup
4. `revert_{tweak}()` → restaura valor original do backup
5. (opcional) `restore_{tweak}_default()` → valor padrão Windows (sem backup)

Tweaks ficam em `commands/tweaks/{categoria}.rs` (gaming, gpu, network, power, storage, visual).

### Padrão de Tweak (Frontend)
```typescript
// TweakCard recebe:
interface TweakInfo {
  id: string;
  name: string;
  description: string;
  is_applied: boolean;
  requires_restart: boolean;
  has_backup: boolean;
  risk_level: 'low' | 'medium' | 'high';
  evidence_level: 'proven' | 'plausible' | 'unproven';
  default_value_description: string;
}
```

## Rotas do Frontend

| Rota             | Componente     | Descrição                                       |
|------------------|----------------|--------------------------------------------------|
| `/`              | Dashboard      | Info HW, status, atividade, planos rápidos       |
| `/optimizations` | Optimizations  | 21 tweaks gaming/GPU/CPU/storage/network/UX      |
| `/privacy`       | Privacy        | 4 tweaks de privacidade + remoção de bloatware   |
| `/maintenance`   | Maintenance    | DISM, SFC, chkdsk, SSD trim, DNS                |
| `/cleanup`       | Cleanup        | Limpeza categorizada (temp, shader, browser, apps)|
| `/services`      | Services       | 33 serviços + 8 tarefas agendadas               |
| `/plans`         | Plans          | CRUD + execução de planos                        |
| `/learn`         | Learn          | Página educacional (mitos e snake oil)           |
| `/about`         | About          | Versão, créditos, verificação de atualizações    |
| `/settings`      | Settings       | Export/import .fg, backups, config               |

**Renderização condicional:** Apenas a página ativa é montada (`pathname === path ? <Page /> : null`). Páginas com dados pesados usam `useRef` para carregar apenas na primeira visita.

## Eventos Tauri (Backend → Frontend)

| Evento                      | Emissor                    | Payload          |
|-----------------------------|----------------------------|------------------|
| `dns_flush_progress`        | flush_dns                  | CommandEvent     |
| `temp_cleanup_progress`     | run_temp_cleanup           | CommandEvent     |
| `dism_cleanup_progress`     | run_dism_cleanup           | CommandEvent     |
| `dism_checkhealth_progress` | run_dism_checkhealth       | CommandEvent     |
| `dism_scanhealth_progress`  | run_dism_scanhealth        | CommandEvent     |
| `dism_restorehealth_progress`| run_dism_restorehealth    | CommandEvent     |
| `sfc_progress`              | run_sfc                    | CommandEvent     |
| `chkdsk_progress`           | run_chkdsk                 | CommandEvent     |
| `ssd_trim_progress`         | run_ssd_trim               | CommandEvent     |
| `plan_progress`             | execute_plan               | PlanProgressEvent|
| `cleanup_progress`          | execute_cleanup            | CleanupProgressEvent|

```typescript
// CommandEvent
{ event_type: 'started' | 'stdout' | 'stderr' | 'completed' | 'error', data: string, timestamp: string }

// PlanProgressEvent
{ plan_id, current_item, current_item_index, total_items, item_status, item_result, overall_progress_percent }
```

## Tipos Principais

### Backend (Rust)

```rust
// Registro
enum Hive { CurrentUser, LocalMachine }

// Tweaks
struct TweakMeta { id, name, description, category, requires_restart, risk_level, evidence_level, default_value_description, hardware_filter }
struct TweakInfo { id, name, description, category, is_applied, requires_restart, has_backup, risk_level, evidence_level, ... }
enum RiskLevel { Low, Medium, High }
enum EvidenceLevel { Proven, Plausible, Unproven }

// Saúde
struct HealthCheckResult { id, name, status: CheckStatus, message, details, duration_seconds, space_freed_mb, locking_processes }
enum CheckStatus { Success, Warning, Error }

// Backup
struct BackupEntry { category: TweakCategory, description, original_value: OriginalValue, status: BackupStatus }
struct OriginalValue { path, key, value: Option<Value>, value_type }
enum BackupStatus { Applied, Reverted }

// Planos
struct Plan { id, name, description, created_at, last_executed, items: Vec<PlanItem>, builtin }
struct PlanItem { tweak_id, order: u32, enabled: bool }
struct PlanExecutionSummary { plan_id, plan_name, duration_seconds, total/completed/failed/skipped counts, results }

// Serviços
struct ServiceItem { id, display_name, description, category, status, startup_type, is_conditional, has_backup }
struct TaskItem { id, display_name, description, category, enabled, has_backup }
struct BatchResult { success_count, fail_count, results: Vec<ItemResult> }

// Atividade
struct ActivityEntry { timestamp, activity_type, name, result, duration_seconds, completed/failed/skipped counts }

// Export
struct FgExportFile { frameguard_export, version, app_version, exported_at, machine_info, backups, plans, settings, services_disabled, tasks_disabled }

// Cleanup
struct CleanupCategory { id, name, description, risk: CleanupRisk, items: Vec<CleanupItem> }
struct CleanupItem { path, display_name, size_bytes, item_type }
enum CleanupRisk { Safe, Moderate, Caution }

// Bloatware
struct UwpAppInfo { name, display_name, publisher, category, recommendation }

// Ponto de Restauração
enum RestorePointResult { Created, Skipped, Disabled, Failed(String) }
```

### Frontend (TypeScript)

```typescript
// Contextos
interface RunningCtx { isRunning: boolean; startTask(key: string): void; endTask(key: string): void }
interface ToastCtx { showToast(type, title, message?, duration?): void }

// Hooks
function useActionRunner(actions: ActionMeta[], lsKeyPrefix: string): { states, handleRun, toggleLog, toggleDetails, isRunning }
function usePlanExecution(): { executingPlan, execState, execute, closeModal, cleanup }
function useHardwareFilter(): { filterCompatible, getVendorBadge }
function useSearchHighlight(opts): void  // auto-scroll + highlight via URL params (?section=&highlight=)
function useDashboardData(cleanupPlanExec): DashboardData  // estados + fetches do Dashboard
function useTweakPage(config: TweakPageConfig): UseTweakPageReturn  // lógica compartilhada Optim/Privacy

// Ação
interface ActionMeta { id, name, Icon, description, technicalDetails, estimatedDuration, eventChannel, command, invokeArgs?, requiresInternet?, requiresRestart?, category }
interface ActionState { running, log: LogLine[], progress, showLog, showDetails, lastResult?: HealthCheckResult }
```

## Design System

### Design Tokens (`src/styles/tokens.css`)

Tokens definidos via `[data-theme]` no `:root`. Dark é o padrão.

```css
/* Fundos */
--bg-base / --bg-surface / --bg-elevated / --bg-modal
/* Accent */
--accent / --accent-hover / --accent-glow
/* Texto */
--text-primary / --text-secondary / --text-muted
/* Status */
--status-green / --status-amber / --status-red
/* Sidebar */
--sidebar-top / --sidebar-bottom
/* Borda */
--border
/* RGB triplets (para compor alpha) */
--accent-rgb / --surface-rgb / --muted-rgb
```

### Tema Dark/Light

- Tokens em `src/styles/tokens.css` com seletores `[data-theme="dark"]` e `[data-theme="light"]`
- Gerenciamento em `src/utils/theme.ts`: `initTheme()`, `applyTheme()`, `getStoredTheme()`
- `initTheme()` chamado em `main.tsx` antes do render (evita flash)
- Persistido em `localStorage` (`fg-theme`, padrão: `dark`)

### Princípios Visuais
- **Frutiger Aero moderno** — glassmorphism sutil, gradientes suaves
- Fonte: Inter, Segoe UI, system-ui
- Cards com `backdrop-filter: blur(12px)`
- Bordas arredondadas consistentes (6-16px)
- Suporte a tema escuro e claro com accent cyan
- CSS Modules para isolamento de estilos por componente

## Scripts de Build

```bash
npm run dev       # Vite dev (1420) + Tauri dev
npm run build     # tsc + vite build → dist/
npm run preview   # Preview local do build
npm run tauri     # CLI Tauri (e.g., npm run tauri build)
```

### Pipeline de Build
1. `tsc` — Type-check TypeScript
2. `vite build` — Bundle frontend → `dist/`
3. `cargo build --release` — Compila Rust
4. Tauri empacota tudo → instalador NSIS (perMachine)

## Segurança

- Elevação admin via `manifest.xml` (requireAdministrator)
- HKLM requer privilégios de admin
- CSP desabilitado (`"csp": null`) — app desktop confiável
- Sem credenciais hardcoded
- Backup protege valores originais antes de qualquer modificação
- File locks detection via Restart Manager API

## Features Recentes

### Busca Global (SearchBar)

- Ativada via `Ctrl+K` ou clique no ícone da sidebar
- Busca fuzzy no índice estático (`src/data/searchIndex.ts`) com tags bilíngues (PT + EN)
- Resultados agrupados por página/tipo com navegação por teclado (↑↓↵)
- Ao clicar, navega para a página e aplica highlight via `useSearchHighlight` (URL params `?section=&highlight=`)
- Highlight com borda cyan por 2 segundos, auto-expande accordion sections

### Filtro por Hardware/Vendor (useHardwareFilter)

- `get_detected_vendors()` detecta fabricantes de GPU/CPU via PowerShell
- `useHardwareFilter()` filtra tweaks incompatíveis (ex: `disable_nvidia_telemetry` só aparece para GPUs NVIDIA)
- Mapeamento estático em `TWEAK_HARDWARE_MAP` — fallback seguro: mostra todos se detecção falhar
- Usado em Optimizations e Plans

### Ponto de Restauração Automático (restorePoint.ts)

- Criação automática antes de tweaks/planos (se habilitado pelo usuário)
- Preferência salva em `localStorage` (`fg.restorePoint`, padrão: habilitado)
- Cache local de 24h para evitar duplicatas (`Mutex<Option<Instant>>` no backend)
- Tratamento gracioso: não bloqueia execução se feature estiver desabilitada ou em cooldown

### Remoção de Bloatware UWP (BloatwareSection)

- Scan e remoção de apps UWP pré-instalados
- Lista curada com ~35 apps em categorias: Microsoft Bloatware, Games/Xbox, OEM, Opcionais, Sistema (protegido)
- Recomendações por app: remover / opcional / manter
- Remoção em batch com tracking de erros
- Integrado na página Privacy

### WelcomeModal (primeira execução)

- Modal de boas-vindas exibido na primeira execução do app
- Apresenta os pilares do FrameGuard
- Controlado via `localStorage` (`fg.firstRunSeen`)

### Página Learn (educacional)

- Desmistifica otimizações comuns do Windows
- Badges: Mito, Perigoso, Obsoleto, Snake Oil
- Tópicos: efeitos visuais, Windows Update, QoS bandwidth, prefetch, etc.
- Explicações baseadas em evidências

### Verificação de Atualizações (About)

- `check_for_updates()` consulta GitHub Releases API
- Comparação semântica de versões
- Exibe release notes da versão mais recente

## Detecção e Localização

### Power Plan

- Detecção por GUID, NUNCA por nome (Windows PT-BR tem nomes diferentes)
- Ultimate Performance: `e9a42b02-d5df-448d-aa00-03f14749eb61`
- High Performance: `8c5e7fda-e8bf-4a96-9a85-a6e23a8c635c`
- Nome extraído do output de `powercfg /getactivescheme` entre parênteses

### Game DVR (3 estados)

- `disabled`: DVR desabilitado por política ou GameDVR_Enabled=0
- `available`: DVR ativo mas sem gravação em background (HistoricalCaptureEnabled=0)
- `recording`: DVR ativo COM gravação em background (HistoricalCaptureEnabled=1)
- Somente `recording` impacta performance de forma mensurável

### GPU

- Detecção via registro direto (sem PowerShell/WMI)
- Path: `HKLM\SYSTEM\ControlSet001\Control\Class\{4d36e968-e325-11ce-bfc1-08002be10318}\000X`
- VRAM: `HardwareInformation.qwMemorySize` (QWORD, 64-bit — necessário para GPUs >4GB)
- Vendor: inferido do nome (nvidia/geforce → nvidia, amd/radeon → amd)

## Cache e Performance

| Item                 | Estratégia                    | TTL      |
|----------------------|-------------------------------|----------|
| StaticHwInfo         | `OnceLock` (cache permanente) | Sessão   |
| GpuInfo              | `OnceLock` (pre-warm setup)   | Sessão   |
| SystemStatus         | `OnceLock<Mutex>` + TTL       | 5s       |
| SystemSummary        | `OnceLock` (cache permanente) | Sessão   |
| Backups/Plans/Log    | `OnceLock<Mutex>` + arquivo   | Persistente |
| Restore Point        | `Mutex<Option<Instant>>`      | 24h      |
| UI event buffer      | Flush a cada 80ms             | —        |
| Log DOM              | Max 500 linhas                | —        |
| CPU/RAM polling      | setInterval (delay até HW OK) | 3s       |
| Status/Activity poll | setInterval                   | 10s      |

## Performance e Inicialização

Princípios críticos para não degradar a experiência de abertura do app:

### Renderização condicional (não keep-alive)

- App.tsx renderiza apenas a página ativa com `pathname === path ? <Page /> : null`
- Páginas com dados pesados (Optimizations, Privacy, Services) usam `useRef` para carregar apenas na primeira vez que ficam visíveis
- Dashboard é a exceção: carrega no mount porque é sempre a primeira página

### Comandos async obrigatórios

- **Todo** comando `#[tauri::command]` que faz I/O (registro, PowerShell, filesystem) DEVE ser `pub async fn` com `tokio::task::spawn_blocking`
- Comandos `pub fn` (sync) rodam na main thread do Tauri e **congelam a WebView inteira**
- Template:

```rust
  #[tauri::command]
  pub async fn get_example_info() -> Result<TweakInfo, String> {
      tokio::task::spawn_blocking(|| {
          // lógica aqui
          Ok(TweakInfo { ... })
      })
      .await
      .map_err(|e| e.to_string())?
  }
```

### Evitar PowerShell quando possível

- Registro do Windows: usar `winreg` diretamente (~1ms vs ~2s via PowerShell)
- GPU info: ler de `HKLM\SYSTEM\ControlSet001\Control\Class\{4d36e968-e325-11ce-bfc1-08002be10318}` (~50ms vs ~3s via WMI)
- Power Plan: chamar `powercfg.exe` diretamente via `run_command`, sem wrapper PowerShell (~50ms vs ~2s)
- PowerShell reservado para: DISM, SFC, operações que genuinamente precisam de scripts

### Pre-warm de caches

- `pre_warm_all_caches()` chamado no setup do Tauri
- Popula GPU, CPU/RAM, SystemSummary e SystemStatus em background
- Dashboard mostra skeleton enquanto dados chegam

### Polling de CPU/RAM

- Inicia SOMENTE após dados de HW carregarem (`useEffect` depende de `hw`)
- Intervalo de 3s (suficiente para delta preciso)
- `try_lock` no Mutex evita enfileiramento de medições

## Toast e Feedback

- Toasts de sucesso/info: 6 segundos de duração
- Toasts de erro: 8 segundos de duração
- Toasts persistentes: `duration=0` (não fecha automaticamente)
- Implementação: `ToastContext` com `useCallback`, animação de dismiss com 300ms

## Adicionando um Novo Tweak (Checklist)

### Backend (Rust)
1. Criar arquivo ou adicionar ao submódulo correto em `commands/tweaks/{categoria}.rs`
2. Declarar `const META: TweakMeta` com metadados estáticos (usando `utils/tweak_builder.rs`)
3. Implementar `get_{tweak}_info()` → `META.build(is_applied)`
4. Implementar `disable_{tweak}()` / `enable_{tweak}()` + `revert_{tweak}()`
5. Usar `backup_before_apply()` antes de alterar registro/sistema
6. Registrar no `tauri::generate_handler![]` em `lib.rs`
7. OBRIGATÓRIO: usar `pub async fn` + `tokio::task::spawn_blocking` — comandos sync congelam a UI

### Frontend (React)
1. Adicionar entrada no `tweakRegistry.ts` (fonte única de verdade)
2. Adicionar o ID no array `tweakIds` da seção correspondente na página (Optimizations/Privacy)
3. `useTweakPage` + `TweakCard` já lidam com apply/revert/restore automaticamente
4. Adicionar `tweak_id` nos planos built-in se relevante (`plan_manager.rs`)

### Planos Built-in

- Se o tweak deve aparecer em planos oficiais, adicionar o ID em `plan_manager.rs`
- CURRENT_BUILTIN_VERSION deve ser incrementado para trigger auto-update dos planos do usuário
- Ordem nos planos: limpeza primeiro, depois otimizações, depois verificações

### Testes manuais (pré-release)

- [ ] Tweak aplica sem erro
- [ ] Tweak reverte sem erro
- [ ] Backup é criado corretamente (verificar em Configurações > Ver backups)
- [ ] Reaplicar tweak já aplicado não dá erro (backup update)
- [ ] Filtro de hardware funciona (tweak some se hardware incompatível)
- [ ] Busca global encontra o tweak (verificar searchIndex.ts e tweakRegistry.ts)

## Adicionando uma Nova Ação de Manutenção (Checklist)

### Backend
1. Criar função no submódulo correto em `commands/health/` (dism.rs, disk.rs ou maintenance.rs)
2. Usar `run_command_with_progress` com event channel dedicado
3. Registrar no `generate_handler![]` em `lib.rs`

### Frontend
1. Adicionar `ActionMeta` no array da página Maintenance
2. `useActionRunner` gerencia execução/streaming automaticamente
3. Definir `eventChannel` correspondente ao backend
