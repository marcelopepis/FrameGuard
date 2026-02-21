// Página de Limpeza do FrameGuard.
//
// Define as ações de limpeza pontuais do sistema (Flush DNS e Temporários)
// e delega toda a lógica de execução e renderização aos componentes compartilhados.

import { Globe, Trash2 } from 'lucide-react';
import { ActionCard } from '../components/ActionCard/ActionCard';
import { useActionRunner } from '../hooks/useActionRunner';
import type { ActionMeta, Section } from '../types/health';
import styles from '../components/ActionCard/ActionCard.module.css';

// ── Metadados das ações ─────────────────────────────────────────────────────

const SECTIONS: Section[] = [
  {
    id: 'limpeza',
    title: 'Limpeza e Manutenção',
    subtitle: 'Limpeza rápida de cache e arquivos temporários do sistema',
  },
];

const ACTIONS: ActionMeta[] = [
  {
    id: 'flush_dns',
    name: 'Flush DNS',
    Icon: Globe,
    description: 'Limpa o cache DNS local. Resolve problemas de conectividade causados por entradas desatualizadas ou corrompidas.',
    technicalDetails:
`Executa: ipconfig.exe /flushdns

O cache DNS local armazena resoluções de nomes recentes (ex: "google.com → 142.250.x.x") para acelerar conexões. Pode ficar desatualizado após mudanças de DNS ou conter entradas corrompidas que causam falhas de conexão.

O flush força o Windows a consultar os servidores DNS configurados na próxima requisição, garantindo endereços atualizados.

Útil para: sites que não carregam após mudança de DNS, troca de provedor, ou alterações no arquivo hosts.`,
    estimatedDuration: '< 1 segundo',
    eventChannel: 'dns_flush_progress',
    command: 'flush_dns',
    category: 'limpeza',
  },
  {
    id: 'temp_cleanup',
    name: 'Limpeza de Temporários',
    Icon: Trash2,
    description: 'Remove arquivos temporários de %TEMP%, Windows\\Temp e do cache do Windows Update. Arquivos em uso são ignorados.',
    technicalDetails:
`Remove arquivos de três locais:

• %TEMP%                                   — Temporários do usuário atual (instaladores, extrações, caches)
• C:\\Windows\\Temp                         — Temporários do sistema e serviços Windows
• C:\\Windows\\SoftwareDistribution\\Download — Cache do Windows Update (atualizações já instaladas)

Arquivos em uso são pulados silenciosamente. A pasta SoftwareDistribution\\Download é recriada automaticamente pelo Windows Update quando necessário.

O espaço liberado é calculado com precisão comparando o tamanho antes e depois da remoção.`,
    estimatedDuration: '30 segundos–3 minutos',
    eventChannel: 'temp_cleanup_progress',
    command: 'run_temp_cleanup',
    category: 'limpeza',
  },
];

// ── Componente ──────────────────────────────────────────────────────────────

export default function Cleanup() {
  const { states, handleRun, toggleLog, toggleDetails, isRunning } =
    useActionRunner(ACTIONS, 'frameguard:cleanup');

  return (
    <div className={styles.page}>
      <div className={styles.header}>
        <div>
          <h1 className={styles.title}>Limpeza</h1>
          <p className={styles.subtitle}>Limpeza e manutenção do sistema</p>
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
