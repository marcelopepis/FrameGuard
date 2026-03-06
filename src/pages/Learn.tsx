// Página educacional "Aprenda" — Mitos e Snake Oil.
//
// Explica tweaks populares de otimização Windows que NÃO funcionam
// ou podem prejudicar o sistema. Cards colapsáveis com badges por categoria.

import { useState } from 'react';
import {
  ChevronDown,
  ExternalLink,
  Info,
  Ban,
  AlertTriangle,
  Clock,
  FlaskConical,
} from 'lucide-react';
import styles from './Learn.module.css';

// ── Tipos ────────────────────────────────────────────────────────────────────

type BadgeType = 'mito' | 'perigoso' | 'obsoleto' | 'snake_oil';

interface MythEntry {
  id: string;
  title: string;
  badge: { type: BadgeType; label: string };
  paragraphs: string[];
  source?: { url: string; label: string };
}

// ── Badge config ─────────────────────────────────────────────────────────────

const BADGE_CONFIG: Record<BadgeType, { Icon: typeof Ban; className: string }> = {
  mito: { Icon: Ban, className: 'badgeMito' },
  perigoso: { Icon: AlertTriangle, className: 'badgePerigoso' },
  obsoleto: { Icon: Clock, className: 'badgeObsoleto' },
  snake_oil: { Icon: FlaskConical, className: 'badgeSnakeOil' },
};

// ── Dados dos mitos ──────────────────────────────────────────────────────────

const MYTHS: MythEntry[] = [
  {
    id: 'visual_effects',
    title: '"Desabilitar efeitos visuais do Windows melhora performance"',
    badge: { type: 'mito', label: 'Mito' },
    paragraphs: [
      'Desde o Windows Vista, os efeitos visuais (transparência, animações, sombras) usam aceleração de hardware via GPU. Desabilitar eles REMOVE a aceleração e força rendering por CPU/software, o que na maioria dos PCs modernos PIORA o desempenho da interface.',
      'Raymond Chen, engenheiro sênior da Microsoft, documentou isso no blog "The Old New Thing": desabilitar composição de desktop pode aumentar carga da CPU. A Microsoft documentou oficialmente que o DWM (Desktop Window Manager) usa GPU para toda a composição visual.',
      'O único cenário onde desabilitar ajuda é em PCs muito antigos sem GPU dedicada — que não servem para gaming de qualquer forma. Em PCs com GPU dedicada, o impacto é literalmente zero nos jogos.',
    ],
    source: {
      url: 'https://devblogs.microsoft.com/oldnewthing/',
      label: 'The Old New Thing — Microsoft DevBlogs',
    },
  },
  {
    id: 'windows_update',
    title: '"Desabilitar Windows Update melhora performance"',
    badge: { type: 'perigoso', label: 'Perigoso' },
    paragraphs: [
      'Windows Update consome recursos apenas durante atualizações — não há impacto contínuo na performance. Desabilitar permanentemente deixa seu sistema vulnerável a exploits críticos que são descobertos e corrigidos mensalmente.',
      'Anti-cheats modernos (EAC, BattlEye, Vanguard) exigem patches recentes do Windows para funcionar. Sem updates, componentes como .NET e Visual C++ Redistributables ficam desatualizados, quebrando jogos que dependem de versões recentes.',
      'Em vez de desabilitar, configure horários ativos no Windows Update para evitar updates durante sessões de jogo. O FrameGuard nunca desabilita updates permanentemente — a atualização contínua é essencial para segurança e compatibilidade.',
    ],
  },
  {
    id: 'qos_bandwidth',
    title: '"O Windows rouba 20% da sua internet"',
    badge: { type: 'mito', label: 'Mito' },
    paragraphs: [
      'O QoS Packet Scheduler reserva 20% de banda APENAS para pacotes QoS-tagged (VoIP, streaming corporativo). Se nenhum aplicativo está usando QoS, 100% da banda está disponível para o seu uso normal — incluindo jogos.',
      'A chave NonBestEffortLimit no registro controla priorização de pacotes, não reserva fixa de banda. Nenhum jogo ou aplicativo comum faz solicitação QoS. Mudar esse valor no gpedit ou registro não muda absolutamente nada.',
      'Todos os testes de bandwidth confiáveis mostram resultados idênticos antes e depois da alteração. Este mito circula desde o Windows XP e nunca foi verdade.',
    ],
  },
  {
    id: 'prefetch_sysmain',
    title: '"Desabilitar Prefetch/SysMain melhora SSD"',
    badge: { type: 'obsoleto', label: 'Obsoleto' },
    paragraphs: [
      'Este conselho era relevante na era dos HDDs lentos, onde o Prefetch/Superfetch causava I/O excessivo. Em SSDs modernos, o SysMain (antigo Superfetch) é inteligente o suficiente para não causar overhead significativo.',
      'O serviço pré-carrega aplicativos frequentes na RAM ociosa — memória que não está sendo usada de qualquer forma. Desabilitar pode AUMENTAR tempos de carregamento de apps usados frequentemente, já que eles não estarão pré-carregados.',
      'O impacto no desgaste do SSD é insignificante considerando a vida útil de SSDs modernos (centenas de TBW). Só faz sentido desabilitar em PCs com menos de 8 GB de RAM e problemas específicos de uso excessivo de memória.',
    ],
  },
  {
    id: 'spectre_meltdown',
    title: '"Desabilitar Spectre/Meltdown patches dobra o FPS"',
    badge: { type: 'perigoso', label: 'Perigoso' },
    paragraphs: [
      'Em CPUs modernas (Intel 10ª gen+, Ryzen 3000+), o impacto das mitigações Spectre/Meltdown é imperceptível em games — menos de 1% de diferença no FPS. As mitigações afetam principalmente workloads de I/O intensivo (servidores, databases), não renderização 3D.',
      "Desabilitar as mitigações expõe o sistema a ataques JavaScript em navegadores e outros vetores de exploração. Tom's Hardware testou 10 CPUs diferentes e não encontrou diferença significativa em nenhum jogo testado.",
      'O "dobrar FPS" era exagero mesmo em CPUs antigas. Na época do lançamento dos patches (2018), a diferença real era de 2-5% em cenários específicos de I/O — nunca em rendering de jogos.',
    ],
    source: {
      url: 'https://www.tomshardware.com/news/meltdown-spectre-cpu-performance-impacts,36585.html',
      label: "Tom's Hardware — Meltdown/Spectre Performance Impact",
    },
  },
  {
    id: 'irq_priority',
    title: '"Mudar IRQ Priority da GPU melhora latência"',
    badge: { type: 'obsoleto', label: 'Obsoleto' },
    paragraphs: [
      'Este tweak era relevante na era PCI, quando dispositivos compartilhavam IRQs (Interrupt Request Lines) e conflitos podiam causar latência. GPUs modernas usam MSI/MSI-X (Message Signaled Interrupts) que não usam IRQ compartilhado.',
      'Com MSI-X, cada dispositivo tem seus próprios vetores de interrupção dedicados — não há conflito para resolver. Alterar prioridade de IRQ no registro não tem efeito algum quando o dispositivo usa MSI.',
      'O FrameGuard já oferece habilitação de MSI Mode para GPU, que é a abordagem correta e moderna para otimizar o handling de interrupções da placa de vídeo.',
    ],
  },
  {
    id: 'network_throttling',
    title: '"NetworkThrottlingIndex = FFFFFFFF melhora rede"',
    badge: { type: 'snake_oil', label: 'Snake Oil' },
    paragraphs: [
      'O NetworkThrottlingIndex do Windows Vista limitava o processamento de rede a 10 pacotes por milissegundo para evitar que tráfego de rede consumisse toda a CPU. Em CPUs multi-core modernas com NICs gigabit, esse throttle nunca é atingido.',
      'Nenhum benchmark confiável mostra diferença mensurável em latência, download ou upload após alterar este valor. A limitação foi projetada para CPUs single-core da era Vista — hardware que ninguém mais usa para gaming.',
      'O tweak é inofensivo mas completamente inútil. É o exemplo clássico de "snake oil" — parece técnico e impressionante, mas não faz nada em hardware moderno.',
    ],
  },
];

// ── Componente ───────────────────────────────────────────────────────────────

export default function Learn() {
  const [expanded, setExpanded] = useState<Set<string>>(new Set());

  function toggle(id: string) {
    setExpanded((prev) => {
      const s = new Set(prev);
      if (s.has(id)) s.delete(id);
      else s.add(id);
      return s;
    });
  }

  return (
    <div className={styles.page}>
      {/* Header */}
      <div className={styles.header}>
        <div>
          <h1 className={styles.title}>Aprenda</h1>
          <p className={styles.subtitle}>
            Mitos comuns sobre otimização do Windows — e o que a ciência diz
          </p>
        </div>
      </div>

      {/* Banner informativo */}
      <div className={styles.infoBanner}>
        <Info size={15} />
        <span>
          Muitos "tweaks" populares na internet não têm base técnica ou são obsoletos. Aqui
          explicamos por que o FrameGuard não inclui esses ajustes — e por que você também não
          deveria aplicá-los manualmente.
        </span>
      </div>

      {/* Lista de mitos */}
      <div className={styles.mythList}>
        {MYTHS.map((myth) => {
          const isOpen = expanded.has(myth.id);
          const cfg = BADGE_CONFIG[myth.badge.type];
          const BIcon = cfg.Icon;

          return (
            <div
              key={myth.id}
              className={`${styles.mythCard} ${isOpen ? styles.mythCardOpen : ''}`}
            >
              <button className={styles.mythToggle} onClick={() => toggle(myth.id)}>
                <div className={styles.mythToggleLeft}>
                  <span className={`${styles.badge} ${styles[cfg.className]}`}>
                    <BIcon size={11} strokeWidth={2.5} />
                    {myth.badge.label}
                  </span>
                  <span className={styles.mythName}>{myth.title}</span>
                </div>
                <ChevronDown
                  size={14}
                  strokeWidth={2}
                  className={`${styles.mythChevron} ${isOpen ? styles.mythChevronOpen : ''}`}
                />
              </button>

              {isOpen && (
                <div className={styles.mythContent}>
                  {myth.paragraphs.map((p, i) => (
                    <p key={i} className={styles.mythParagraph}>
                      {p}
                    </p>
                  ))}
                  {myth.source && (
                    <a
                      href={myth.source.url}
                      target="_blank"
                      rel="noopener noreferrer"
                      className={styles.sourceLink}
                    >
                      <ExternalLink size={12} />
                      {myth.source.label}
                    </a>
                  )}
                </div>
              )}
            </div>
          );
        })}
      </div>
    </div>
  );
}
