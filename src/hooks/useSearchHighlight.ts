/**
 * Hook compartilhado para scroll + highlight de itens encontrados pela busca global.
 *
 * Lê os search params `?section=` e `?highlight=` da URL, expande a seção
 * do accordion (se aplicável), scrolla até o item e aplica um outline
 * cyan temporário de 2 segundos.
 *
 * @module useSearchHighlight
 */

import { useEffect } from 'react';
import { useSearchParams } from 'react-router-dom';

/** Opções de configuração para o hook `useSearchHighlight`. */
interface UseSearchHighlightOptions {
  /** Atributo data-* usado nos wrappers dos itens (ex: `"data-tweak-id"`) */
  dataAttribute: string;
  /** Se `true`, adia a execução até o loading terminar */
  pageLoading: boolean;
  /** Callback para expandir uma seção do accordion pelo ID */
  expandSection?: (sectionId: string) => void;
}

/**
 * Lê os search params da URL e aplica scroll + highlight no item correspondente.
 *
 * Após processar os params, limpa-os da URL (replace) para evitar re-disparo
 * ao navegar de volta para a página.
 *
 * @param options - Configuração com atributo de dados, estado de loading e callback de expansão
 *
 * @example
 * ```tsx
 * useSearchHighlight({
 *   dataAttribute: 'data-tweak-id',
 *   pageLoading,
 *   expandSection: (id) => setExpanded(prev => ({ ...prev, [id]: true })),
 * });
 * ```
 */
export function useSearchHighlight({
  dataAttribute,
  pageLoading,
  expandSection,
}: UseSearchHighlightOptions) {
  const [searchParams, setSearchParams] = useSearchParams();

  useEffect(() => {
    const section = searchParams.get('section');
    const highlight = searchParams.get('highlight');
    if (!section && !highlight) return;
    if (pageLoading) return;

    if (section && expandSection) {
      expandSection(section);
    }

    // Limpa params para não re-disparar ao voltar
    setSearchParams({}, { replace: true });

    if (highlight) {
      // Aguarda a expansão CSS do accordion (300ms) + margem
      const delay = section && expandSection ? 350 : 50;

      requestAnimationFrame(() => {
        setTimeout(() => {
          const el = document.querySelector(`[${dataAttribute}="${highlight}"]`);
          if (!el) return;

          el.scrollIntoView({ behavior: 'smooth', block: 'center' });
          el.classList.add('searchHighlight');
          setTimeout(() => el.classList.remove('searchHighlight'), 2000);
        }, delay);
      });
    }
  }, [searchParams, setSearchParams, pageLoading, dataAttribute, expandSection]);
}
