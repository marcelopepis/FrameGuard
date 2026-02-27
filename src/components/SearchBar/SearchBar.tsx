// Barra de busca global da sidebar.
//
// Filtra o índice estático de tweaks, ações e planos em tempo real,
// agrupando os resultados por página. Suporta Ctrl+K, keyboard nav
// (ArrowUp/Down/Enter/Escape) e navegação por clique.

import { useState, useRef, useEffect, useMemo } from 'react';
import { useNavigate } from 'react-router-dom';
import { Search, X } from 'lucide-react';
import { SEARCH_INDEX, type SearchItem } from '../../data/searchIndex';
import styles from './SearchBar.module.css';

// ── Helpers ──────────────────────────────────────────────────────────────────

/** Remove acentos e converte para minúsculas. */
function normalize(text: string): string {
  return text.normalize('NFD').replace(/[\u0300-\u036f]/g, '').toLowerCase();
}

/** Labels legíveis para o tipo de item. */
const TYPE_LABELS: Record<string, string> = {
  tweak: 'Tweak',
  action: 'Ação',
  plan: 'Plano',
};

// ── Componente ───────────────────────────────────────────────────────────────

export default function SearchBar() {
  const [query, setQuery] = useState('');
  const [isOpen, setIsOpen] = useState(false);
  const [selectedIndex, setSelectedIndex] = useState(0);

  const inputRef = useRef<HTMLInputElement>(null);
  const wrapperRef = useRef<HTMLDivElement>(null);
  const navigate = useNavigate();

  // ── Filtragem ──

  const results = useMemo(() => {
    const q = query.trim();
    if (!q) return [];

    const tokens = normalize(q).split(/\s+/);

    const scored = SEARCH_INDEX
      .map(item => {
        const haystack = normalize(`${item.name} ${item.description} ${item.tags.join(' ')}`);

        // Todos os tokens devem estar presentes (AND)
        if (!tokens.every(t => haystack.includes(t))) return null;

        // Score: nome > tags > description
        const nq = normalize(q);
        const nameScore = normalize(item.name).includes(nq) ? 2 : 0;
        const tagScore = item.tags.some(t => normalize(t).includes(nq)) ? 1 : 0;
        return { item, score: nameScore + tagScore };
      })
      .filter(Boolean) as { item: SearchItem; score: number }[];

    scored.sort((a, b) => b.score - a.score);
    return scored.slice(0, 10).map(r => r.item);
  }, [query]);

  // ── Agrupamento por página ──

  const grouped = useMemo(() => {
    const map = new Map<string, SearchItem[]>();
    for (const item of results) {
      const list = map.get(item.pageLabel) ?? [];
      list.push(item);
      map.set(item.pageLabel, list);
    }
    return map;
  }, [results]);

  // ── Navegação para o resultado ──

  function handleSelect(item: SearchItem) {
    setQuery('');
    setIsOpen(false);
    inputRef.current?.blur();

    // Planos usam o padrão ?viewPlan= já existente
    if (item.type === 'plan') {
      navigate(`/plans?viewPlan=${item.id}`);
      return;
    }

    const params = new URLSearchParams();
    if (item.section) params.set('section', item.section);
    params.set('highlight', item.id);
    navigate(`${item.page}?${params.toString()}`);
  }

  // ── Keyboard ──

  function handleKeyDown(e: React.KeyboardEvent) {
    if (e.key === 'Escape') {
      setIsOpen(false);
      setQuery('');
      inputRef.current?.blur();
      return;
    }

    if (e.key === 'ArrowDown') {
      e.preventDefault();
      setSelectedIndex(prev => Math.min(prev + 1, results.length - 1));
      return;
    }

    if (e.key === 'ArrowUp') {
      e.preventDefault();
      setSelectedIndex(prev => Math.max(prev - 1, 0));
      return;
    }

    if (e.key === 'Enter' && results[selectedIndex]) {
      e.preventDefault();
      handleSelect(results[selectedIndex]);
    }
  }

  // ── Click outside → fechar ──

  useEffect(() => {
    function onMouseDown(e: MouseEvent) {
      if (wrapperRef.current && !wrapperRef.current.contains(e.target as Node)) {
        setIsOpen(false);
      }
    }
    document.addEventListener('mousedown', onMouseDown);
    return () => document.removeEventListener('mousedown', onMouseDown);
  }, []);

  // ── Ctrl+K global ──

  useEffect(() => {
    function onKeyDown(e: KeyboardEvent) {
      if ((e.ctrlKey || e.metaKey) && e.key === 'k') {
        e.preventDefault();
        inputRef.current?.focus();
        setIsOpen(true);
      }
    }
    document.addEventListener('keydown', onKeyDown);
    return () => document.removeEventListener('keydown', onKeyDown);
  }, []);

  // ── Render ──

  const showDropdown = isOpen && query.trim().length > 0;

  return (
    <div className={styles.searchWrapper} ref={wrapperRef}>
      <div className={styles.searchInputWrap}>
        <Search size={14} strokeWidth={2} className={styles.searchIcon} />
        <input
          ref={inputRef}
          type="text"
          className={styles.searchInput}
          placeholder="Buscar..."
          value={query}
          onChange={e => {
            setQuery(e.target.value);
            setIsOpen(true);
            setSelectedIndex(0);
          }}
          onFocus={() => setIsOpen(true)}
          onKeyDown={handleKeyDown}
        />
        {query ? (
          <button
            className={styles.clearBtn}
            onClick={() => { setQuery(''); setSelectedIndex(0); }}
            tabIndex={-1}
          >
            <X size={12} strokeWidth={2} />
          </button>
        ) : (
          <span className={styles.shortcutHint}>
            <kbd className={styles.shortcutKey}>Ctrl</kbd>
            <kbd className={styles.shortcutKey}>K</kbd>
          </span>
        )}
      </div>

      {/* Dropdown de resultados */}
      {showDropdown && results.length > 0 && (
        <div className={styles.dropdown}>
          {Array.from(grouped.entries()).map(([pageLabel, items]) => (
            <div key={pageLabel} className={styles.group}>
              <div className={styles.groupLabel}>{pageLabel}</div>
              {items.map(item => {
                const flatIdx = results.indexOf(item);
                return (
                  <button
                    key={item.id}
                    className={`${styles.resultItem} ${flatIdx === selectedIndex ? styles.resultItemActive : ''}`}
                    onClick={() => handleSelect(item)}
                    onMouseEnter={() => setSelectedIndex(flatIdx)}
                  >
                    <Search size={13} strokeWidth={2} className={styles.resultIcon} />
                    <div className={styles.resultInfo}>
                      <span className={styles.resultName}>{item.name}</span>
                      <span className={styles.resultType}>{TYPE_LABELS[item.type] ?? item.type}</span>
                    </div>
                  </button>
                );
              })}
            </div>
          ))}
        </div>
      )}

      {/* Nenhum resultado */}
      {showDropdown && results.length === 0 && (
        <div className={styles.dropdown}>
          <div className={styles.noResults}>Nenhum resultado encontrado</div>
        </div>
      )}
    </div>
  );
}
