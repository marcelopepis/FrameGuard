# FrameGuard — Roadmap

> Documento de referência interno. Atualizar a cada release concluído.
> Filosofia de versionamento: `0.2.x` = melhorias/adições ao modelo atual | `0.3.x` = nova UX/paradigma | `0.4.x` = features avançadas

---

## ✅ v0.1.0 — Release inicial
- App funcional com Dashboard, Otimizações, Privacidade, Serviços, Limpeza
- Detecção de hardware (GPU/CPU vendor, Power Plan, Windows version)
- Backup e restore de tweaks
- GitHub Actions CI/CD com build automático

## ✅ v0.1.1 — Hotfix
- Fix detecção incorreta de Windows 11 (usava `ProductName` → migrado para `CurrentBuildNumber ≥ 22000`)

## ✅ v0.2.0 — Polish e infraestrutura
- Light mode / Dark mode funcional
- Busca global (Ctrl+K) com keyboard navigation
- Remoção de UWP/Bloatware (40 apps curados)
- Infraestrutura vendor-specific (`useHardwareFilter`, `TWEAK_HARDWARE_MAP`)
- Export/import de configuração `.fg`
- Issue templates no GitHub

## ✅ v0.2.1 — Qualidade e auto-update
- Fix de contraste em light mode (bloatware não instalado — WCAG 4.5:1)
- Auto-updater on-demand na página About (sem background process, sem serviço)
  - Nota técnica: `createUpdaterArtifacts: true` é obrigatório no objeto `bundle` do `tauri.conf.json`

## ✅ v0.2.2 — Privacidade expandida e debloat
- Windows Recall disable (`HKLM\...\WindowsAI`)
- Windows Error Reporting off
- Activity History / Timeline off
- Location Tracking global off
- Feedback Frequency off
- DiagTrack expandido (`AllowDeviceNameInTelemetry`, `DoNotShowFeedbackNotifications`, `dmwappushservice`)
- Edge Debloat (~15 policies: startup boost, background mode, sidebar Copilot, shopping, telemetria)
- Classic Right Click Menu (Win11 only — reinicia Explorer automaticamente)

---

## ✅ v0.2.3 — Serviços, CPU vendor tweaks e Docker
- 11 novos serviços Tier 1 (Fax, MapsBroker, RetailDemo, WerSvc, lfsvc, wisvc, RemoteRegistry, AJRouter, SEMgrSvc, TermService, Spooler)
- Warnings críticos e detalhados nos 4 serviços Xbox (XblAuthManager, XblGameSave, XboxNetApiSvc, XboxGipSvc) — explica o que quebra em cada um
- CPU Vendor Tweaks: AMD Ryzen Power Plan (GUID `9897998c-...`), Intel Power Throttling Off, Intel Turbo Boost Agressivo
- AMD fTPM detection — warning no Dashboard com link para FAQ da AMD
- Docker cleanup — containers parados, imagens dangling, build cache (volumes opt-in com warning)

## ✅ v0.2.4 — GPU vendor tweaks, tweaks de alto impacto e Mitos & Verdades

### Tweaks de alto impacto (todos os usuários)
- **HVCI/VBS disable** (`disable_hvci_vbs`) — 5-10% FPS médio, até 28% em CPUs sem MBEC. `risk_level: Medium`, `requires_restart: true`
- **Timer Resolution 1ms** (`timer_resolution_1ms`) — `GlobalTimerResolutionRequests=1`. Melhora 20-30% em 1% lows. `risk_level: Low`, `requires_restart: false`

### NVIDIA GPU (`hardware_filter: { gpu_vendor: "nvidia" }`)
- **PowerMizer** (`nvidia_power_mizer`) — enumera subkey GPU via `find_gpu_registry_subkey("NVIDIA")`, seta `PerfLevelSrc=0x2222`, `PowerMizerEnable=1`, `PowerMizerLevelAC=1`
- **Telemetria off** (`nvidia_telemetry_off`) — IFEO bloqueando `NvTelemetryContainer.exe` + desabilita scheduled tasks `NvTmMon`/`NvTmRep` via `schtasks`
- **Overlay off** (`nvidia_overlay_off`) — `HKCU\...\ShadowPlay\ShadowPlayOnSystemStart\Enable=0`
- **MSI Mode revisão** — agora cobre GPU Display + HD Audio Controller NVIDIA. Detecta estado atual (RTX 40+ já em MSI reporta "já aplicado"). Backup multi-entry com `restore_multi_entries`

### AMD GPU (`hardware_filter: { gpu_vendor: "amd" }`)
- **ULPS disable** (`amd_ulps_disable`) — `EnableUlps=0` na subkey GPU via `find_gpu_registry_subkey("AMD")`/`"Radeon"`. Elimina delays de wake e black screens
- **Shader Cache forçado** (`amd_shader_cache`) — `ShaderCache` (REG_BINARY) = `32 00` no subkey `UMD`. Funções `read_binary`/`write_binary` adicionadas a `registry.rs`

### Frontend
- 7 tweaks registrados em `tweakRegistry.ts` e `searchIndex.ts` (busca global)
- 5 tweaks vendor-specific mapeados em `TWEAK_HARDWARE_MAP` (`useHardwareFilter.ts`)

### Página Mitos e Verdades (Learn)
- 14 itens com veredicto técnico: 5 PERIGOSO (vermelho), 4 MITO (cyan), 5 CONDICIONAL (amarelo)
- Cada item: badge colorido, veredicto em uma linha, explicação em accordion, link para fonte
- Itens: pagefile, Realtime priority, Defender, Spectre/Meltdown, Xbox Services, Nagle, IRQ8Priority, SvcHostSplit, Core Parking, Memory Compression, Process Affinity, HAGS, HPET, SysMain

---

## 📋 v0.3.0 — Nova experiência de uso *(mudança de paradigma — justifica bump de minor)*

### Nova Home Page — "O que você quer fazer hoje?"
Substitui o Dashboard como tela inicial. 3 camadas progressivas:
1. **Plano Recomendado** — analisa estado atual (tweaks não aplicados, serviços rodando) e gera plano personalizado com 1 clique
2. **Planos por Intenção** — 4 cards:
   - 🎮 Quero mais FPS → HVCI, Timer Resolution, Game DVR, GPU vendor tweaks
   - 🔒 Quero mais privacidade → Telemetria, WER, Activity History, Recall, Edge Debloat
   - 🧹 Quero limpar → Cleanup, UWP Bloatware, serviços desnecessários
   - ⚡ Quero tudo → todos os tweaks safe combinados
3. **Modo Expert** → link para páginas específicas (comportamento atual)

### Contador Before/After Serviços
- WMI `Win32_Service WHERE State='Running'` antes e depois de executar qualquer plano
- Exibir inline: "Antes: 189 rodando → Depois: 177 rodando (-12 serviços)"
- Delay de 3-5s após execução para aguardar serviços terminarem

### Arquivos novos
- `src/pages/Home.tsx` + `Home.module.css`
- `src/hooks/useHomeRecommendations.ts`
- `src-tauri/src/commands/metrics.rs` (`get_running_services_count`)
- Atualizar `App.tsx`: rota `/` → Home. Dashboard continua acessível pela sidebar

---

## 📋 v0.4.0 — Raio-X do PC com IA + Educação

### Feature principal: Raio-X do PC com IA
Diagnóstico inteligente baseado no Event Viewer + IA generativa.

**Fluxo**:
1. Coletar eventos do Event Viewer — últimos 3 dias, apenas Error/Warning/Critical
2. Filtrar ruído conhecido (ex: `DistributedCOM 10016`, `Kernel-EventTracing`, `VSS` — eventos normais do Windows que poluiriam o relatório)
3. Sanitizar dados sensíveis (caminhos de arquivo, nomes de usuário, IPs) antes de enviar
4. Enviar para API do Claude com prompt de sistema contextualizado:
   - O que o FrameGuard pode fazer (tweaks disponíveis, limpezas, etc.)
   - Os eventos coletados
   - Instrução para gerar: diagnóstico de estabilidade + ações que o FrameGuard pode executar + ações manuais recomendadas
5. Exibir relatório formatado com plano de ação

**Arquitetura BYOK (Bring Your Own Key)**:
- Campo nas Settings para o usuário inserir sua chave da API Anthropic
- A chave é armazenada localmente (nunca enviada ao servidor do desenvolvedor)
- Todo o processamento ocorre diretamente entre o PC do usuário e a API Anthropic
- Feature gratuita — sem monetização planejada por enquanto
- Iniciar apenas com Claude/Anthropic. Outras IAs (GPT-4, Gemini) como expansão futura após validar qualidade do prompt

**Considerações importantes**:
- O prompt do sistema é o coração da feature — vai precisar de iteração
- Lista de exclusão de eventos ruidosos precisa ser curada (trabalho real)
- Sanitização de dados sensíveis é obrigatória para coerência com filosofia do projeto

~~### Feature secundária: Mitos e Verdades (página Learn)~~
*(Implementado em v0.2.4 — 14 itens com badges coloridos, accordion e fontes)*

---

## 🔮 Backlog / Futuro indefinido

- **Auto-exclusões Defender para jogos** — detectar Steam/Epic/GOG via registry, listar pastas, adicionar exclusões via `Add-MpPreference`. Sem desabilitar Defender
- **Memory Dump files** — arquivos gerados por BSODs podem acumular facilmente 20GB+. Verificar e oferecer limpeza de `C:\Windows\Minidump\` (minidumps individuais) e `C:\Windows\MEMORY.DMP` (dump completo). Exibir tamanho total antes de limpar
- **Pontos de Restauração do Windows** — podem acumular 40GB+. Oferecer limpeza com opção de manter apenas o ponto mais recente (nunca deletar tudo sem aviso). Usar `vssadmin delete shadows` ou `Checkpoint-Computer` via PowerShell
- **Microsoft Store** — publicação futura. Usar SignPath.org para certificado de código gratuito (open source)
- **Node.js upgrade** — Node 20.x EOL abril 2026. Migrar para LTS atual

---

## Referências técnicas importantes

### Padrões obrigatórios do projeto
- Comandos Tauri com I/O: sempre `pub async fn` + `tokio::task::spawn_blocking` (nunca `pub fn` síncrono — bloqueia main thread)
- Versão Windows: sempre `CurrentBuildNumber` (≥22000 = Win11). NUNCA `ProductName`
- Power Plan: detectar por GUID, nunca por nome (compatibilidade cross-locale)
- GPU info: ler direto de `HKLM\SYSTEM\ControlSet001\Control\Class\{4d36e968...}` via winreg
- Detecção de subkey GPU: `find_gpu_registry_subkey()` em `gpu.rs` — enumera `HKLM\SYSTEM\CurrentControlSet\Control\Class\{4d36e968-e325-11ce-bfc1-08002be10318}\` buscando `DriverDesc` contendo "NVIDIA" ou "AMD"/"Radeon". O índice (`0000`, `0001`...) varia por sistema
- Páginas: conditional rendering (não keep-alive/display:none) — evita disparo simultâneo de comandos na startup
- Auto-updater: `createUpdaterArtifacts: true` obrigatório no objeto `bundle` do `tauri.conf.json`

### GUIDs de Power Plans
| Plano | GUID |
|---|---|
| Balanceado (Windows) | `381b4222-f694-41f0-9685-ff5bb260df2e` |
| Alto Desempenho | `8c5e7fda-e8bf-4a96-9a85-a6e23a8c635c` |
| Máximo Desempenho | `e9a42b02-d5df-448d-aa00-03f14749eb61` |
| AMD Ryzen Balanced | `9897998c-92de-4669-853f-b7cd3ecb2790` |

### Serviços Xbox — o que cada um faz
| Serviço | Quebra se desabilitar |
|---|---|
| XblAuthManager | Login Xbox Live, Game Pass não lança, achievements param |
| XblGameSave | Cloud saves param de sincronizar |
| XboxNetApiSvc | Multiplayer Xbox Live, detecção NAT, party chat |
| XboxGipSvc | Firmware update e remapeamento de controles Xbox Elite/Series |

### Tweaks com evidência sólida (referência rápida)
| Tweak | Impacto | Risco |
|---|---|---|
| HVCI/VBS disable | 5-10% FPS médio, até 28% sem MBEC | Medium |
| Timer Resolution 1ms | 20-30% melhora em 1% lows | Low |
| AMD ULPS disable | Elimina black screens e wake delays | Low |
| PowerMizer NVIDIA | GPU sempre em P0 | Low |
| MSI Mode GPU | Reduz DPC latency | Low |
| AMD Ryzen Power Plan | Responsividade e boost | Low |
| Edge StartupBoost off | Melhora boot time | Low |
