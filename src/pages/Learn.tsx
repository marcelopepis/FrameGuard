// Página educacional "Aprenda" — Mitos e Verdades.
//
// Explica tweaks populares de otimização Windows que NÃO funcionam
// ou podem prejudicar o sistema. Cards colapsáveis com badges de veredicto.

import { useState } from 'react';
import {
  ChevronDown,
  ExternalLink,
  ShieldAlert,
  ShieldCheck,
  ShieldQuestion,
} from 'lucide-react';
import styles from './Learn.module.css';

// ── Tipos ────────────────────────────────────────────────────────────────────

type VerdictType = 'perigoso' | 'mito' | 'condicional';

interface VerdictEntry {
  id: string;
  title: string;
  verdict: VerdictType;
  verdictLine: string;
  explanation: string;
  source?: { url: string; label: string };
}

// ── Badge config ─────────────────────────────────────────────────────────────

const VERDICT_CONFIG: Record<
  VerdictType,
  { Icon: typeof ShieldAlert; className: string; label: string }
> = {
  perigoso: { Icon: ShieldAlert, className: 'verdictPerigoso', label: 'PERIGOSO' },
  mito: { Icon: ShieldCheck, className: 'verdictMito', label: 'MITO' },
  condicional: { Icon: ShieldQuestion, className: 'verdictCondicional', label: 'CONDICIONAL' },
};

// ── Dados ────────────────────────────────────────────────────────────────────

const ENTRIES: VerdictEntry[] = [
  // ── PERIGOSO ────────────────────────────────────────────────────────────────
  {
    id: 'pagefile',
    title: 'Desabilitar o arquivo de paginação (pagefile) melhora FPS',
    verdict: 'perigoso',
    verdictLine: 'Causa crashes garantidos — mesmo com 16GB+ de RAM',
    explanation:
      'O Windows é um sistema operacional paginado. Programas reservam blocos de memória virtual muito maiores que o uso físico real. Sem pagefile, quando esse espaço virtual não cabe na RAM física, o resultado é crash imediato. Jogos como PUBG, Warzone e Watch Dogs 2 travam com pagefile desabilitado mesmo em sistemas com 16GB. Em SSDs modernos, o pagefile raramente é acessado e tem impacto zero em condições normais.',
    source: {
      url: 'https://forums.tomshardware.com/threads/should-i-disable-pagefile-if-i-have-16gb-ram.3744073/',
      label: "Tom's Hardware Forums",
    },
  },
  {
    id: 'realtime',
    title: 'Prioridade Realtime para o jogo melhora FPS',
    verdict: 'perigoso',
    verdictLine: 'Congela o sistema completamente — requer reinicialização forçada',
    explanation:
      'Prioridade Realtime significa que o processo não cede controle da CPU para nenhuma outra thread — incluindo mouse, teclado, drivers de GPU e processos críticos do Windows. O resultado documentado é freeze instantâneo do sistema inteiro. Prioridade High (um nível abaixo) é segura e pode ajudar marginalmente.',
    source: {
      url: 'https://forums.tomshardware.com/threads/setting-game-priority-to-realtime.3536073/',
      label: "Tom's Hardware Forums, Blur Busters Forums",
    },
  },
  {
    id: 'defender',
    title: 'Desabilitar o Windows Defender dá mais FPS',
    verdict: 'perigoso',
    verdictLine: 'Zero ganho mensurável em gaming — risco crítico de segurança',
    explanation:
      'Testes independentes do AV-Comparatives medem impacto em operações de arquivo (cópia, instalação), não em gaming ativo onde o jogo já está rodando e os arquivos já foram escaneados. O overhead real em gaming é mínimo. A solução correta é adicionar as pastas dos jogos às exclusões do Defender.',
    source: {
      url: 'https://www.av-comparatives.org/tests/performance-test-april-2024/',
      label: 'AV-Comparatives Performance Test 2024',
    },
  },
  {
    id: 'spectre',
    title: 'Desabilitar mitigações Spectre/Meltdown melhora gaming',
    verdict: 'perigoso',
    verdictLine: 'Ganho negligível em gaming — vulnerabilidades ativamente exploradas em 2024',
    explanation:
      'As penalidades pesadas de 15-30% afetam workloads I/O-intensivos de servidor. Em gaming, o CPU passa o tempo em física, IA e rendering — não em syscalls privilegiadas. CPUs modernas (Intel Skylake+, AMD Zen 2+) têm mitigações em hardware que tornam o overhead mínimo. Em março de 2024, pesquisadores publicaram novas técnicas de exploração de Spectre v2 — as vulnerabilidades continuam ativas.',
    source: {
      url: 'https://www.howtogeek.com/906483/should-you-disable-spectre-and-meltdown-patches/',
      label: 'How-To Geek, Phoronix, StarWind Software (2024)',
    },
  },
  {
    id: 'windows_update',
    title: 'Desabilitar Windows Update melhora performance',
    verdict: 'perigoso',
    verdictLine: 'Sem impacto contínuo em FPS — deixa o sistema vulnerável a exploits críticos',
    explanation:
      'Windows Update consome recursos apenas durante atualizações — não há impacto contínuo na performance. Desabilitar permanentemente deixa seu sistema vulnerável a exploits críticos que são descobertos e corrigidos mensalmente. Anti-cheats modernos (EAC, BattlEye, Vanguard) exigem patches recentes do Windows para funcionar. Em vez de desabilitar, configure horários ativos no Windows Update para evitar updates durante sessões de jogo.',
  },
  {
    id: 'xbox_services',
    title: 'Desabilitar Xbox Services elimina telemetria da Microsoft',
    verdict: 'perigoso',
    verdictLine: 'Quebra Game Pass, achievements e cloud saves — telemetria é de outros serviços',
    explanation:
      'XblAuthManager, XblGameSave, XboxNetApiSvc e XboxGipSvc têm funções reais: autenticação Xbox Live, saves na nuvem, networking multiplayer e firmware de controles. Todos já são Manual por padrão — só rodam quando necessários. A telemetria da Microsoft vem de DiagTrack e outros serviços independentes, não dos serviços Xbox. O FrameGuard desabilita a telemetria real e preserva os serviços Xbox com avisos claros.',
    source: {
      url: 'https://learn.microsoft.com/en-us/answers/questions/1226803/is-it-safe-to-disable-xbox-services',
      label: 'Microsoft Learn Q&A',
    },
  },

  // ── MITO ────────────────────────────────────────────────────────────────────
  {
    id: 'nagle',
    title: "Nagle's Algorithm desabilitado melhora ping em jogos",
    verdict: 'mito',
    verdictLine: 'Inútil — jogos modernos já ignoram essa configuração',
    explanation:
      'O algoritmo de Nagle de 1984 só afeta TCP. Jogos competitivos modernos (CS2, Valorant, Fortnite, Apex) usam UDP, onde Nagle não existe. Jogos que usam TCP (alguns MMOs) já setam TCP_NODELAY por socket internamente. A modificação via registro é system-wide, mas cada socket sobrescreve com sua própria configuração — o tweak não faz nada para nenhum jogo relevante.',
    source: {
      url: 'https://en.wikipedia.org/wiki/Nagle%27s_algorithm',
      label: "Wikipedia (Nagle's Algorithm), IT Hare, cFosSpeed Forum",
    },
  },
  {
    id: 'irq_priority',
    title: 'IRQ Priority / IRQ8Priority melhora latência de GPU ou áudio',
    verdict: 'mito',
    verdictLine: 'Completamente inútil — o kernel Windows não lê essas chaves',
    explanation:
      'Análise do binário do kernel NT via Sysinternals Strings provou que a string "IRQ8Priority" não existe em nenhum arquivo do sistema. O Windows não usa IRQ8 para timekeeping desde o Windows XP. GPUs modernas usam MSI/MSI-X (Message Signaled Interrupts) com vetores de interrupção dedicados — não há conflito de IRQ para resolver. Alterar prioridades de IRQ no registro não tem efeito algum em hardware moderno.',
    source: {
      url: 'https://github.com/djdallmann/GamingPCSetup',
      label: 'GamingPCSetup (djdallmann) — análise de kernel',
    },
  },
  {
    id: 'svchost',
    title: 'SvcHostSplitThresholdInKB economiza RAM',
    verdict: 'mito',
    verdictLine: 'Apenas cosmético — uso total de RAM idêntico ou maior',
    explanation:
      'O tweak apenas reagrupa visualmente os processos. Em vez de 20 processos svchost usando 100MB cada, você vê 5 usando 400MB cada. A Microsoft separou os serviços intencionalmente no Windows 10 1703 para melhorar isolamento e diagnóstico de crashes. Reverter apenas esconde a complexidade real.',
    source: {
      url: 'https://learn.microsoft.com/en-us/windows/application-management/svchost-service-refactoring',
      label: 'Microsoft — Windows 10 Creators Update',
    },
  },
  {
    id: 'coreparking',
    title: 'Core Parking manual melhora FPS',
    verdict: 'mito',
    verdictLine: 'Redundante — planos de energia modernos já fazem isso automaticamente',
    explanation:
      'Os planos High Performance e Ultimate Performance já desabilitam Core Parking. Intel Skylake+ moveu o controle para hardware on-chip. Modificar via registro não adiciona nada além do que o plano de energia já oferece. Não existe benchmark documentado mostrando ganho além do plano correto.',
  },
  {
    id: 'visual_effects',
    title: 'Desabilitar efeitos visuais do Windows melhora performance',
    verdict: 'mito',
    verdictLine: 'Remove aceleração de GPU e força rendering por CPU — pode piorar',
    explanation:
      'Desde o Windows Vista, os efeitos visuais (transparência, animações, sombras) usam aceleração de hardware via GPU. Desabilitar REMOVE a aceleração e força rendering por CPU/software, o que na maioria dos PCs modernos PIORA o desempenho da interface. Raymond Chen, engenheiro da Microsoft, documentou isso no blog "The Old New Thing". Em PCs com GPU dedicada, o impacto é literalmente zero nos jogos.',
    source: {
      url: 'https://devblogs.microsoft.com/oldnewthing/',
      label: 'The Old New Thing — Microsoft DevBlogs',
    },
  },
  {
    id: 'qos_bandwidth',
    title: 'O Windows rouba 20% da sua internet (QoS)',
    verdict: 'mito',
    verdictLine: 'QoS reserva banda apenas para pacotes tagueados — jogos não usam QoS',
    explanation:
      'O QoS Packet Scheduler reserva 20% de banda APENAS para pacotes QoS-tagged (VoIP, streaming corporativo). Se nenhum aplicativo está usando QoS, 100% da banda está disponível. Nenhum jogo faz solicitação QoS. Mudar NonBestEffortLimit no registro ou gpedit não muda absolutamente nada. Este mito circula desde o Windows XP e nunca foi verdade.',
  },
  {
    id: 'network_throttling',
    title: 'NetworkThrottlingIndex = FFFFFFFF melhora rede',
    verdict: 'mito',
    verdictLine: 'Snake oil — limitação da era Vista nunca atingida em hardware moderno',
    explanation:
      'O NetworkThrottlingIndex do Windows Vista limitava o processamento de rede a 10 pacotes por milissegundo para evitar que tráfego de rede consumisse toda a CPU. Em CPUs multi-core modernas com NICs gigabit, esse throttle nunca é atingido. Nenhum benchmark confiável mostra diferença mensurável. O tweak é inofensivo mas completamente inútil — parece técnico e impressionante, mas não faz nada em hardware moderno.',
  },

  // ── CONDICIONAL ─────────────────────────────────────────────────────────────
  {
    id: 'memcompression',
    title: 'Desabilitar Memory Compression (Disable-MMAgent -mc) melhora gaming',
    verdict: 'condicional',
    verdictLine: 'Inútil com 16GB+ — pode piorar com 8GB',
    explanation:
      'A compressão só ativa quando a RAM está sob pressão. Com 16GB+, raramente é acionada durante gaming — o overhead de CPU é próximo de zero. Desabilitar com 8GB força o Windows a usar o pagefile em disco quando a RAM encher, trocando overhead de CPU (mínimo) por leitura de disco (perceptível). Memory Compression está desabilitada no Windows Server por razões específicas de servidor — não se aplica a desktops gaming.',
    source: {
      url: 'https://learn.microsoft.com/en-us/answers/questions/313032/memory-compression',
      label: 'Microsoft Q&A, NinjaOne, guru3D Forum',
    },
  },
  {
    id: 'affinity',
    title: 'Process Affinity manual melhora FPS',
    verdict: 'condicional',
    verdictLine: 'Pode piorar em CPUs híbridas Intel — inútil no resto',
    explanation:
      'Em CPUs Intel 12th gen+ (P-cores + E-cores), setar affinity manualmente pode acidentalmente forçar threads do jogo para E-cores, causando degradação massiva de performance. O Thread Director do Windows já direciona automaticamente cargas de gaming para P-cores. Para AMD Ryzen e Intel pré-12th, o scheduler moderno é mais eficiente que qualquer configuração manual.',
  },
  {
    id: 'hags',
    title: 'HAGS (Hardware-Accelerated GPU Scheduling) sempre melhora FPS',
    verdict: 'condicional',
    verdictLine: 'Obrigatório para Frame Generation — neutro ou negativo fora disso',
    explanation:
      'Ganho médio medido: ~0.3% de FPS. Custo: ~1GB de VRAM (crítico para GPUs com 8GB). HAGS é obrigatório para DLSS 3/4 Frame Generation em RTX 40/50 series — sem ele, Frame Generation não funciona. Para todos os outros casos, desabilitar é neutro ou ligeiramente melhor. Windows 11 ativa por padrão.',
    source: {
      url: 'https://www.tomshardware.com/reviews/hardware-accelerated-gpu-scheduling-test',
      label: "Tom's Hardware, FrameSync Labs (jan/2025)",
    },
  },
  {
    id: 'hpet',
    title: 'HPET off (bcdedit) sempre melhora latência',
    verdict: 'condicional',
    verdictLine: 'Hardware-dependent — sem efeito na maioria dos sistemas modernos',
    explanation:
      'Sistemas modernos já usam TSC (Time Stamp Counter) como clock padrão, não HPET. O comando "bcdedit /set useplatformclock false" só tem efeito se algo tiver forçado HPET anteriormente. Aplicar em sistema onde TSC já é padrão não muda nada. Verificar com "bcdedit /enum" se "useplatformclock" aparece no output antes de modificar.',
  },
  {
    id: 'sysmain',
    title: 'SysMain/Superfetch é prejudicial em SSD',
    verdict: 'condicional',
    verdictLine: 'Windows já otimiza automaticamente para SSD — diferença mínima',
    explanation:
      'O Windows detecta SSDs e reduz automaticamente a atividade do SysMain. Em drives NVMe modernos, o serviço fica praticamente dormente. Benchmarks mostram diferença de milissegundos em lançamento de apps — zero impacto em gaming FPS. Desabilitar é seguro mas desnecessário com SSD e 16GB+ de RAM.',
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
            Mitos e Verdades — tweaks populares analisados com evidência técnica
          </p>
        </div>
      </div>

      {/* Banner informativo */}
      <div className={styles.infoBanner}>
        <ShieldAlert size={15} />
        <span>
          O FrameGuard nunca implementa tweaks perigosos ou sem evidência real. Esta página existe
          para proteger você da desinformação que circula na comunidade de otimização Windows.
        </span>
      </div>

      {/* Lista unificada */}
      <div className={styles.mythList}>
        {ENTRIES.map((entry) => {
          const isOpen = expanded.has(entry.id);
          const cfg = VERDICT_CONFIG[entry.verdict];
          const VIcon = cfg.Icon;

          return (
            <div
              key={entry.id}
              className={`${styles.mythCard} ${isOpen ? styles.mythCardOpen : ''}`}
            >
              <button className={styles.mythToggle} onClick={() => toggle(entry.id)}>
                <div className={styles.mythToggleLeft}>
                  <span className={`${styles.badge} ${styles[cfg.className]}`}>
                    <VIcon size={11} strokeWidth={2.5} />
                    {cfg.label}
                  </span>
                  <span className={styles.mythName}>{entry.title}</span>
                  <span className={styles.verdictLine}>{entry.verdictLine}</span>
                </div>
                <ChevronDown
                  size={14}
                  strokeWidth={2}
                  className={`${styles.mythChevron} ${isOpen ? styles.mythChevronOpen : ''}`}
                />
              </button>

              {isOpen && (
                <div className={styles.mythContent}>
                  <p className={styles.mythParagraph}>{entry.explanation}</p>
                  {entry.source && (
                    <a
                      href={entry.source.url}
                      target="_blank"
                      rel="noopener noreferrer"
                      className={styles.sourceLink}
                    >
                      <ExternalLink size={12} />
                      {entry.source.label}
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
