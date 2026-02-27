// Hook compartilhado para scroll + highlight de itens encontrados pela busca global.
//
// Lê os search params ?section= e ?highlight= da URL, expande a seção
// do accordion (se aplicável), scrolla até o item e aplica um outline
// cyan temporário de 2 segundos.

import { useEffect } from 'react';
import { useSearchParams } from 'react-router-dom';

interface UseSearchHighlightOptions {
  /** Atributo data-* usado nos wrappers dos itens (ex: 'data-tweak-id') */
  dataAttribute: string;
  /** Se true, adia a execução até o loading terminar */
  pageLoading: boolean;
  /** Callback para expandir uma seção do accordion pelo ID */
  expandSection?: (sectionId: string) => void;
}

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
