# FrameGuard - Guia de Desenvolvimento

Utilitário de manutenção e otimização Windows 11 para gamers.

Informações que o Claude Code precisa saber ao executar qualquer prompt:

- Stack: Tauri (Rust backend) + React (TypeScript frontend)
- Estrutura: Backend em `src-tauri/src/`, Frontend em `src/`
- Padrões existentes: OnceLock<Mutex<T>>, comandos async com spawn_blocking, CSS Modules, ícones Lucide
- Keep-alive: App.tsx renderiza todas as páginas simultaneamente, escondendo inativas com display:none
- Planos built-in: IDs prefixados com "builtin_" e definidos em plan_manager.rs
- Persistência: JSON em %APPDATA%\FrameGuard/ (plans.json, backups.json, activity_log.json)
- O app roda em Windows PT-BR — detecção baseada em texto deve ser agnóstica de idioma
- Correções recentes já aplicadas: activity log integration, dashboard cache com TTL/OnceLock, file locks com Restart Manager API

Ao gerar código novo:
- Rust: usar spawn_blocking para operações que envolvem PowerShell, WMI ou registry
- React: CSS Modules (.module.css) para estilos, Lucide para ícones
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
│  │ Contexts  │ │  Hooks   │ │      7 Pages         ││
│  │(Running,  │ │(Action,  │ │ Dashboard, Optim,    ││
│  │ Toast)    │ │ Plan)    │ │ Privacy, Maint,      ││
│  └──────────┘ └──────────┘ │ Services, Plans, Set  ││
│                             └──────────────────────┘│
│          invoke() ↕ listen()                        │
├─────────────────────────────────────────────────────┤
│  Backend (Rust)                                     │
│  ┌──────────────┐  ┌──────────────────────────────┐ │
│  │  Commands     │  │  Utils                       │ │
│  │  (system_info,│  │  (registry, command_runner,  │ │
│  │   optim, priv,│  │   backup, plan_manager,      │ │
│  │   health, ...) │  │   activity_log, file_locks) │ │
│  └──────────────┘  └──────────────────────────────┘ │
│          ↕ Win32 API / PowerShell / Registry        │
├─────────────────────────────────────────────────────┤
│  Windows 11 (Elevação Admin via manifest.xml)       │
└─────────────────────────────────────────────────────┘
```

## Estrutura de Diretórios

```
FrameGuard/
├── src/                           # Frontend React/TS
│   ├── App.tsx                    # Router + keep-alive (todas as 7 páginas montadas)
│   ├── main.tsx                   # Entry point
│   ├── components/
│   │   ├── ActionCard/            # Card de ação com progresso, logs, resultado
│   │   ├── Layout/                # Layout principal (Sidebar + content)
│   │   ├── Toast/                 # Notificações toast (portal)
│   │   ├── TweakCard.tsx          # Card de tweak com apply/revert/restore
│   │   └── index.ts
│   ├── contexts/
│   │   ├── RunningContext.tsx      # Estado global de execução (Set<string>)
│   │   └── ToastContext.tsx        # Fila de toasts (max 3)
│   ├── hooks/
│   │   ├── useActionRunner.ts     # Execução de ações com streaming
│   │   └── usePlanExecution.ts    # Execução de planos com progresso por item
│   ├── pages/
│   │   ├── Dashboard.tsx          # Hardware, status, atividade recente, planos rápidos
│   │   ├── Optimizations.tsx      # 21 tweaks em 6 categorias
│   │   ├── Privacy.tsx            # 4 tweaks de privacidade
│   │   ├── Maintenance.tsx        # DISM, SFC, cleanup, disco
│   │   ├── Services.tsx           # Serviços e tarefas agendadas
│   │   ├── Plans.tsx              # CRUD de planos de execução
│   │   └── Settings.tsx           # Export/import, backups, sobre
│   ├── services/
│   │   └── systemInfo.ts          # Wrappers invoke() para system info
│   ├── styles/
│   │   └── globals.css            # CSS vars do tema, reset, scrollbar
│   ├── types/
│   │   └── health.ts              # Interfaces compartilhadas
│   └── utils/
│       └── formatters.ts          # formatDuration, formatDate, formatSpaceFreed
│
├── src-tauri/                     # Backend Rust
│   ├── src/
│   │   ├── main.rs                # Entry point binário
│   │   ├── lib.rs                 # Setup Tauri + registro de 153 comandos
│   │   ├── commands/
│   │   │   ├── mod.rs             # Declaração de módulos
│   │   │   ├── system_info.rs     # HW info (cache), status (TTL 5s), usage, summary
│   │   │   ├── optimizations.rs   # 21 tweaks (get_info, apply, revert, restore_default)
│   │   │   ├── privacy.rs         # 4 tweaks de privacidade
│   │   │   ├── health_check.rs    # DISM, SFC, chkdsk, SSD trim, DNS, temp cleanup
│   │   │   ├── cleanup.rs         # analyze_cleanup, run_cleanup
│   │   │   ├── plans.rs           # CRUD + execute_plan (emite plan_progress)
│   │   │   ├── services.rs        # 33 serviços + 8 tarefas curadas
│   │   │   ├── activity.rs        # log_tweak_activity, get_recent_activity
│   │   │   └── export_import.rs   # Export/import .fg (JSON), validate_fg_file
│   │   └── utils/
│   │       ├── mod.rs
│   │       ├── registry.rs        # read/write DWORD/STRING, delete, key_exists (HKCU/HKLM)
│   │       ├── command_runner.rs   # run_command, run_command_with_progress, run_powershell
│   │       ├── backup.rs          # Backup/restore tweaks (%APPDATA%\FrameGuard\backups.json)
│   │       ├── plan_manager.rs    # CRUD planos + 4 built-in (%APPDATA%\FrameGuard\plans.json)
│   │       ├── activity_log.rs    # FIFO max 100 (%APPDATA%\FrameGuard\activity_log.json)
│   │       ├── file_locks.rs      # Restart Manager API — detecta processos travando arquivos
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
1. `get_{tweak}_info()` → `TweakInfo` (status atual, risk, evidence, backup)
2. `disable_{tweak}()` ou `enable_{tweak}()` → aplica + cria backup
3. `revert_{tweak}()` → restaura valor original do backup
4. `restore_{tweak}_default()` → valor padrão Windows (sem backup)

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

| Rota             | Componente     | Descrição                                    |
|------------------|----------------|----------------------------------------------|
| `/`              | Dashboard      | Info HW, status, atividade, planos rápidos   |
| `/optimizations` | Optimizations  | 21 tweaks gaming/GPU/CPU/storage/network/UX  |
| `/privacy`       | Privacy        | 4 tweaks de privacidade/debloat              |
| `/maintenance`   | Maintenance    | Cleanup, DISM, SFC, disco                    |
| `/services`      | Services       | 33 serviços + 8 tarefas agendadas            |
| `/plans`         | Plans          | CRUD + execução de planos                    |
| `/settings`      | Settings       | Export/import .fg, backups, config            |

**Keep-alive:** Todas as páginas ficam sempre montadas (`display: none` quando inativas) para preservar estado React, listeners e execuções em andamento.

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
```

### Frontend (TypeScript)

```typescript
// Contextos
interface RunningCtx { isRunning: boolean; startTask(key: string): void; endTask(key: string): void }
interface ToastCtx { showToast(type, title, message?, duration?): void }

// Hooks
function useActionRunner(actions: ActionMeta[], lsKeyPrefix: string): { states, handleRun, toggleLog, toggleDetails, isRunning }
function usePlanExecution(): { executingPlan, execState, execute, closeModal, cleanup }

// Ação
interface ActionMeta { id, name, Icon, description, technicalDetails, estimatedDuration, eventChannel, command, invokeArgs?, requiresInternet?, requiresRestart?, category }
interface ActionState { running, log: LogLine[], progress, showLog, showDetails, lastResult?: HealthCheckResult }
```

## Design System

### Cores (CSS Custom Properties)
```css
--color-bg-primary:    #0a0e17;        /* Fundo principal */
--color-bg-secondary:  #111827;        /* Fundo secundário */
--color-bg-card:       rgba(17,24,39,0.6); /* Card glassmorphism */
--color-accent:        #22d3ee;        /* Cyan (ações primárias) */
--color-accent-hover:  #06b6d4;        /* Cyan hover */
--color-text-primary:  #f1f5f9;        /* Texto principal */
--color-text-secondary:#94a3b8;        /* Texto secundário */
--color-border:        rgba(148,163,184,0.12);
--color-success:       #34d399;        /* Verde */
--color-warning:       #fbbf24;        /* Amarelo */
--color-error:         #f87171;        /* Vermelho */
--glass-blur:          blur(12px);     /* Glassmorphism */
--radius-sm/md/lg:     6px/10px/16px;
--transition-fast:     150ms ease;
--transition-normal:   250ms ease;
```

### Princípios Visuais
- **Frutiger Aero moderno** — glassmorphism sutil, gradientes suaves
- Fonte: Inter, Segoe UI, system-ui
- Cards com `backdrop-filter: blur(12px)`
- Bordas arredondadas consistentes (6-16px)
- Tema escuro com accent cyan
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

## Cache e Performance

| Item                 | Estratégia                    | TTL      |
|----------------------|-------------------------------|----------|
| StaticHwInfo         | `OnceLock` (cache permanente) | Sessão   |
| SystemStatus         | `OnceLock<Mutex>` + TTL       | 5s       |
| Backups/Plans/Log    | `OnceLock<Mutex>` + arquivo   | Persistente |
| UI event buffer      | Flush a cada 80ms             | —        |
| Log DOM              | Max 500 linhas                | —        |
| CPU/RAM polling      | setInterval                   | 2s       |
| Status/Activity poll | setInterval                   | 10s      |

## Adicionando um Novo Tweak (Checklist)

### Backend (Rust)
1. Criar funções em `commands/optimizations.rs` ou `commands/privacy.rs`:
   - `get_{tweak}_info() -> Result<TweakInfo, String>`
   - `disable_{tweak}() -> Result<(), String>` (ou `enable_`)
   - `revert_{tweak}() -> Result<(), String>`
   - (opcional) `restore_{tweak}_default() -> Result<(), String>`
2. Registrar no `tauri::generate_handler![]` em `lib.rs`
3. Usar `backup_before_apply()` antes de alterar registro/sistema
4. Se async necessário: `tokio::task::spawn_blocking`

### Frontend (React)
1. Adicionar entrada no array de tweaks da página correspondente
2. TweakCard já lida com apply/revert/restore automaticamente
3. Adicionar `tweak_id` nos planos built-in se relevante (`plan_manager.rs`)

## Adicionando uma Nova Ação de Manutenção (Checklist)

### Backend
1. Criar função `async fn run_{action}(app: AppHandle) -> Result<HealthCheckResult, String>` em `health_check.rs`
2. Usar `run_command_with_progress` com event channel dedicado
3. Registrar no `generate_handler![]`

### Frontend
1. Adicionar `ActionMeta` no array da página Maintenance
2. `useActionRunner` gerencia execução/streaming automaticamente
3. Definir `eventChannel` correspondente ao backend
