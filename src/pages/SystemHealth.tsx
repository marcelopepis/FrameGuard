// Página de Saúde do Sistema do FrameGuard.
//
// Define as ações de manutenção e verificação do Windows (DISM, SFC, ChkDsk,
// TRIM) e delega toda a lógica de execução e renderização aos componentes
// compartilhados.

import {
  ShieldCheck, Search, Wrench, Package,
  FileCheck, HardDrive, Zap,
} from 'lucide-react';
import { ActionCard } from '../components/ActionCard/ActionCard';
import { useActionRunner } from '../hooks/useActionRunner';
import type { ActionMeta, Section } from '../types/health';
import styles from '../components/ActionCard/ActionCard.module.css';

// ── Metadados estáticos das ações ───────────────────────────────────────────

const SECTIONS: Section[] = [
  {
    id: 'dism',
    title: 'DISM — Component Store',
    subtitle: 'Integridade e reparo do repositório de componentes do Windows',
  },
  {
    id: 'verificacao',
    title: 'Verificação de Disco',
    subtitle: 'Integridade do sistema de arquivos e otimização de SSDs',
  },
];

const ACTIONS: ActionMeta[] = [
  // ── DISM ──────────────────────────────────────────────────────────────────
  {
    id: 'dism_checkhealth',
    name: 'DISM CheckHealth',
    Icon: ShieldCheck,
    description: 'Verificação rápida de integridade do Component Store. Consulta apenas metadados locais — sem downloads, sem reparos.',
    technicalDetails:
`Executa: DISM /Online /Cleanup-Image /CheckHealth

Consulta somente os metadados do Component Store (WinSxS), sem examinar os arquivos reais. É a verificação mais rápida e ideal para diagnóstico inicial.

Saídas possíveis:
• "No component store corruption detected" → Saudável
• "The component store is repairable"       → Corrupção detectada, use RestoreHealth
• "The component store is corrupted"        → Corrupção grave, reparo urgente`,
    estimatedDuration: '< 30 segundos',
    eventChannel: 'dism_checkhealth_progress',
    command: 'run_dism_checkhealth',
    category: 'dism',
  },
  {
    id: 'dism_scanhealth',
    name: 'DISM ScanHealth',
    Icon: Search,
    description: 'Varredura profunda do Component Store. Examina todos os arquivos em busca de corrupção, sem realizar reparos.',
    technicalDetails:
`Executa: DISM /Online /Cleanup-Image /ScanHealth

Mais abrangente que o CheckHealth: verifica os arquivos reais do WinSxS comparando com os manifestos do sistema. Pode levar vários minutos.

Não realiza reparos — apenas documenta os problemas. Se detectar corrupção, execute o RestoreHealth a seguir.`,
    estimatedDuration: '2–15 minutos',
    eventChannel: 'dism_scanhealth_progress',
    command: 'run_dism_scanhealth',
    category: 'dism',
  },
  {
    id: 'dism_restorehealth',
    name: 'DISM RestoreHealth',
    Icon: Wrench,
    description: 'Repara o Component Store baixando arquivos limpos do Windows Update. Substitui componentes corrompidos por versões íntegras.',
    technicalDetails:
`Executa: DISM /Online /Cleanup-Image /RestoreHealth

Baixa versões íntegras dos componentes corrompidos diretamente dos servidores da Microsoft via Windows Update, substituindo os arquivos danificados.

Recomendação: execute ScanHealth antes para confirmar a corrupção. Após o RestoreHealth, execute SFC /scannow para reparar arquivos de sistema individuais.

Requer conexão ativa com a internet.`,
    estimatedDuration: '5–30 minutos',
    eventChannel: 'dism_restorehealth_progress',
    command: 'run_dism_restorehealth',
    requiresInternet: true,
    category: 'dism',
  },
  {
    id: 'dism_cleanup',
    name: 'DISM StartComponentCleanup',
    Icon: Package,
    description: 'Remove componentes obsoletos de atualizações anteriores da pasta WinSxS, liberando espaço em disco.',
    technicalDetails:
`Executa: DISM /Online /Cleanup-Image /StartComponentCleanup

O Windows mantém cópias antigas dos componentes do sistema para permitir rollback de atualizações. Com o tempo, esse acúmulo pode ocupar vários GB em C:\\Windows\\WinSxS.

O StartComponentCleanup remove versões que não são mais necessárias, reduzindo o tamanho do Component Store.

O Windows 10/11 faz isso automaticamente via agendamento — este comando força a limpeza imediata.`,
    estimatedDuration: '1–10 minutos',
    eventChannel: 'dism_cleanup_progress',
    command: 'run_dism_cleanup',
    category: 'dism',
  },

  // ── Verificação ───────────────────────────────────────────────────────────
  {
    id: 'sfc_scannow',
    name: 'SFC /scannow',
    Icon: FileCheck,
    description: 'Verifica e repara arquivos protegidos do Windows usando o cache local. Não requer conexão com a internet.',
    technicalDetails:
`Executa: sfc.exe /scannow (System File Checker)

Verifica a integridade de todos os arquivos protegidos do sistema e repara automaticamente os corrompidos usando o cache local (C:\\Windows\\System32\\dllcache).

Diferença entre SFC e DISM RestoreHealth:
• SFC usa cache local — mais rápido, sem internet, mas limitado ao cache disponível
• DISM usa Windows Update — mais abrangente, requer internet

Recomendação: execute DISM RestoreHealth primeiro para reconstruir o cache, depois SFC para reparar arquivos individuais.

O log completo fica em: C:\\Windows\\Logs\\CBS\\CBS.log`,
    estimatedDuration: '10–30 minutos',
    eventChannel: 'sfc_progress',
    command: 'run_sfc',
    category: 'verificacao',
  },
  {
    id: 'chkdsk',
    name: 'Check Disk (C:)',
    Icon: HardDrive,
    description: 'Verifica e corrige erros lógicos e físicos no disco C:. Se o disco estiver em uso, agenda a verificação para o próximo boot.',
    technicalDetails:
`Executa: chkdsk.exe C: /r

O flag /r implica /f (corrigir erros) e adiciona verificação de setores físicos defeituosos.

Comportamento no disco do sistema (C: em uso):
• O volume está bloqueado pelo Windows — chkdsk não consegue acessá-lo diretamente
• Uma confirmação "Y" é enviada automaticamente para agendar no próximo boot
• A verificação ocorre antes do Windows iniciar na próxima reinicialização

Exit codes: 0=sem erros, 1=erros corrigidos, 2=limpeza sugerida, 3=falha grave.`,
    estimatedDuration: 'Agendamento imediato / varia no boot',
    eventChannel: 'chkdsk_progress',
    command: 'run_chkdsk',
    invokeArgs: { driveLetter: null },
    requiresRestart: true,
    category: 'verificacao',
  },
  {
    id: 'ssd_trim',
    name: 'TRIM de SSDs',
    Icon: Zap,
    description: 'Executa TRIM em todos os SSDs detectados para manter performance de escrita e prolongar a vida útil do dispositivo.',
    technicalDetails:
`Usa PowerShell: Get-PhysicalDisk (SSD) + Optimize-Volume -ReTrim

O TRIM instrui o SSD a apagar internamente blocos marcados como não utilizados pelo sistema de arquivos. Sem TRIM, blocos "sujos" se acumulam e degradam a performance de escrita progressivamente.

O Windows executa TRIM automaticamente via Scheduled Tasks, mas executar manualmente garante que todos os SSDs estejam otimizados agora.

Apenas SSDs são processados — HDDs são detectados e ignorados automaticamente.`,
    estimatedDuration: '30 segundos–2 minutos',
    eventChannel: 'trim_progress',
    command: 'run_ssd_trim',
    category: 'verificacao',
  },
];

// ── Componente ──────────────────────────────────────────────────────────────

export default function SystemHealth() {
  const { states, handleRun, toggleLog, toggleDetails, isRunning } =
    useActionRunner(ACTIONS, 'frameguard:health');

  return (
    <div className={styles.page}>
      <div className={styles.header}>
        <div>
          <h1 className={styles.title}>Saúde do Sistema</h1>
          <p className={styles.subtitle}>Manutenção e integridade do Windows</p>
        </div>
      </div>

      <div className={styles.sections}>
        {SECTIONS.map(section => {
          const sectionActions = ACTIONS.filter(a => a.category === section.id);
          return (
            <div key={section.id} className={styles.section}>
              <div className={styles.sectionHeader}>
                <span className={styles.sectionTitle}>{section.title}</span>
                <span className={styles.sectionSubtitle}>{section.subtitle}</span>
              </div>
              <div className={styles.actionList}>
                {sectionActions.map(meta => (
                  <ActionCard
                    key={meta.id}
                    meta={meta}
                    state={states[meta.id]}
                    onRun={() => handleRun(meta)}
                    onToggleLog={() => toggleLog(meta.id)}
                    onToggleDetails={() => toggleDetails(meta.id)}
                    disabled={isRunning && !states[meta.id].running}
                  />
                ))}
              </div>
            </div>
          );
        })}
      </div>
    </div>
  );
}
