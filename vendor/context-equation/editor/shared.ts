// What the components share: the theme's variables, the icons, the words in each language, and how a
// component finds the field it works for.

import type { ContextEquationField } from "./field.ts";

/** The theme: every color and size the components use. Set `--context-equation-<name>` on an element or any
 * ancestor to change it (it reaches the components inside `<context-equation-editor>` too); the defaults use
 * `light-dark()`, which follows the page's `color-scheme`. */
export const TOKENS = `
:host {
  --_ink: var(--context-equation-ink, light-dark(#1e2224, #ece9e1));
  --_muted: var(--context-equation-muted, light-dark(#5c6062, #a8abaa));
  --_background: var(--context-equation-background, light-dark(#ffffff, #202326));
  --_surface: var(--context-equation-surface, light-dark(#f4f3ef, #2a2e31));
  --_border: var(--context-equation-border, light-dark(#d4d1c7, #42474b));
  --_focus: var(--context-equation-focus, light-dark(#1f5f99, #9fd3f2));
  --_caret: var(--context-equation-caret, light-dark(#1f5f99, #9fd3f2));
  --_selection: var(--context-equation-selection, light-dark(rgb(31 95 153 / 0.18), rgb(159 211 242 / 0.25)));
  --_slot: var(--context-equation-slot, light-dark(#a5713f, #d9a873));
  --_key: var(--context-equation-key, light-dark(#ffffff, #2f3336));
  --_key-active: var(--context-equation-key-active, light-dark(#ecebe6, #3a3f43));
  --_accent: var(--context-equation-accent, light-dark(#1f5f99, #9fd3f2));
  --_radius: var(--context-equation-radius, 6px);
  --_font: var(--context-equation-font, system-ui, -apple-system, "Segoe UI", sans-serif);
  --_mono: var(--context-equation-mono, ui-monospace, "SF Mono", Menlo, Consolas, monospace);
  --_page: var(--context-equation-page, light-dark(#f4f3ef, #18191b));
  font-family: var(--_font);
}
`;

/** Icons (24×24, stroked, in the style of Lucide). */
const ICON_PATHS: Record<string, string> = {
  left: "M15 18l-6-6 6-6",
  right: "M9 18l6-6-6-6",
  up: "M18 15l-6-6-6 6",
  down: "M6 9l6 6 6-6",
  backspace: "M21 4H8l-7 8 7 8h13a2 2 0 0 0 2-2V6a2 2 0 0 0-2-2z M18 9l-6 6 M12 9l6 6",
  enter: "M9 10l-5 5 5 5 M20 4v7a4 4 0 0 1-4 4H4",
  copy: "M8 8h12v12H8z M16 8V4H4v12h4",
  check: "M20 6L9 17l-5-5",
  download: "M12 3v12 M7 10l5 5 5-5 M5 21h14",
  undo: "M9 14L4 9l5-5 M4 9h10.5a5.5 5.5 0 0 1 0 11H11",
  redo: "M15 14l5-5-5-5 M20 9H9.5a5.5 5.5 0 0 0 0 11H13",
  shift: "M12 3l9 9h-5v8H8v-8H3z",
  tab: "M21 4v16 M3 12h14 M13 8l4 4-4 4",
  rowAdd: "M3 5h18 M3 11h18 M12 15v6 M9 18h6",
  rowRemove: "M3 5h18 M3 11h18 M9 18h6",
  columnAdd: "M5 3v18 M11 3v18 M18 9v6 M15 12h6",
  table: "M3 3h18v18H3z M3 9h18 M3 15h18 M9 3v18 M15 3v18",
  columnRemove: "M5 3v18 M11 3v18 M15 12h6",
  close: "M18 6L6 18 M6 6l12 12",
  keyboard: "M3 6h18v12H3z M7 10h0 M11 10h0 M15 10h0 M8 14h8",
  clear: "M3 6h18 M8 6V4h8v2 M6 6l1 14h10l1-14",
  search: "M11 4a7 7 0 1 0 0 14a7 7 0 1 0 0-14 M21 21l-4.35-4.35",
  globe: "M12 3a9 9 0 1 0 0 18a9 9 0 1 0 0-18 M3 12h18 M12 3c2.5 2.7 3.8 5.7 3.8 9s-1.3 6.3-3.8 9 M12 3c-2.5 2.7-3.8 5.7-3.8 9s1.3 6.3 3.8 9",
  sun: "M12 8a4 4 0 1 0 0 8a4 4 0 1 0 0-8 M12 2v2 M12 20v2 M4.9 4.9l1.4 1.4 M17.7 17.7l1.4 1.4 M2 12h2 M20 12h2 M4.9 19.1l1.4-1.4 M17.7 6.3l1.4-1.4",
  moon: "M20 14.5A8 8 0 0 1 9.5 4a8 8 0 1 0 10.5 10.5z",
  monitor: "M3 4h18v12H3z M8 20h8 M12 16v4",
  panel: "M3 4h18v16H3z M15 4v16",
  zoomIn: "M11 4a7 7 0 1 0 0 14a7 7 0 1 0 0-14 M21 21l-4.35-4.35 M11 8v6 M8 11h6",
  zoomOut: "M11 4a7 7 0 1 0 0 14a7 7 0 1 0 0-14 M21 21l-4.35-4.35 M8 11h6",
  chevron: "M6 9l6 6 6-6",
  gallery: "M3 3h7v7H3z M14 3h7v7h-7z M14 14h7v7h-7z M3 14h7v7H3z",
  options: "M4 21v-7 M4 10V3 M12 21v-9 M12 8V3 M20 21v-5 M20 12V3 M1 14h6 M9 8h6 M17 16h6",
  play: "M6 4l14 8-14 8z",
};

/** An icon as inline SVG; the button around it carries the name (`title` and `aria-label`). */
export function icon(name: string): string {
  const d = ICON_PATHS[name] ?? "";
  return `<svg viewBox="0 0 24 24" width="18" height="18" fill="none" stroke="currentColor" stroke-width="2" stroke-linecap="round" stroke-linejoin="round" aria-hidden="true">${d
    .split(" M")
    .map((p, i) => `<path d="${i ? "M" : ""}${p}"/>`)
    .join("")}</svg>`;
}

/** A word in the field's language: an object by locale (falls back to its language, then English) or one string. */
export type Words = string | Record<string, string>;

export function say(words: Words | undefined, locale: string | null | undefined): string {
  if (words === undefined) return "";
  if (typeof words === "string") return words;
  const l = locale ?? "en";
  return words[l] ?? words[l.split("-")[0]] ?? words.en ?? Object.values(words)[0] ?? "";
}

/** The words of the components' own interface. */
export const UI = {
  formula: { en: "Formula", "pt-BR": "Fórmula" },
  copy: { en: "Copy", "pt-BR": "Copiar" },
  copied: { en: "Copied", "pt-BR": "Copiado" },
  download: { en: "Download", "pt-BR": "Baixar" },
  insert: { en: "Insert", "pt-BR": "Inserir" },
  commands: { en: "Commands", "pt-BR": "Comandos" },
  noCommand: { en: "No command with this name", "pt-BR": "Nenhum comando com esse nome" },
  keyboard: { en: "Keyboard", "pt-BR": "Teclado" },
  undo: { en: "Undo", "pt-BR": "Desfazer" },
  redo: { en: "Redo", "pt-BR": "Refazer" },
  clear: { en: "Clear", "pt-BR": "Limpar" },
  left: { en: "Move left", "pt-BR": "Mover para a esquerda" },
  right: { en: "Move right", "pt-BR": "Mover para a direita" },
  up: { en: "Move up", "pt-BR": "Mover para cima" },
  down: { en: "Move down", "pt-BR": "Mover para baixo" },
  backspace: { en: "Delete back", "pt-BR": "Apagar" },
  enter: { en: "Enter", "pt-BR": "Confirmar" },
  tab: { en: "Next empty place", "pt-BR": "Próximo lugar vazio" },
  shift: { en: "Capitals", "pt-BR": "Maiúsculas" },
  latex: { en: "LaTeX", "pt-BR": "LaTeX" },
  mathml: { en: "MathML", "pt-BR": "MathML" },
  svg: { en: "SVG", "pt-BR": "SVG" },
  png: { en: "PNG", "pt-BR": "PNG" },
  speech: { en: "Speech", "pt-BR": "Fala" },
  braille: { en: "Braille", "pt-BR": "Braille" },
  text: { en: "Text", "pt-BR": "Texto" },
  empty: { en: "Nothing to show yet", "pt-BR": "Nada para mostrar ainda" },
  outputs: { en: "Outputs", "pt-BR": "Saídas" },
  palettes: { en: "Palettes", "pt-BR": "Paletas" },
  search: { en: "Search symbols and structures", "pt-BR": "Buscar símbolos e estruturas" },
  results: { en: "Results", "pt-BR": "Resultados" },
  noResults: { en: "Nothing found", "pt-BR": "Nada encontrado" },
  language: { en: "Language", "pt-BR": "Idioma" },
  theme: { en: "Theme", "pt-BR": "Tema" },
  themeAuto: { en: "Follow the system", "pt-BR": "Seguir o sistema" },
  themeLight: { en: "Light", "pt-BR": "Claro" },
  themeDark: { en: "Dark", "pt-BR": "Escuro" },
  zoomIn: { en: "Larger", "pt-BR": "Aumentar" },
  zoomOut: { en: "Smaller", "pt-BR": "Diminuir" },
  showOutputs: { en: "Show the outputs", "pt-BR": "Mostrar as saídas" },
  rules: { en: "Rules and warnings", "pt-BR": "Regras e avisos" },
  noRules: { en: "No rule changed this formula, and nothing to warn about.", "pt-BR": "Nenhuma regra mudou esta fórmula, e não há avisos." },
  applied: { en: "Rules contextEquation applied", "pt-BR": "Regras que o contextEquation aplicou" },
  warnings: { en: "Warnings", "pt-BR": "Avisos" },
  refused: { en: "contextEquation refused the formula", "pt-BR": "O contextEquation recusou a fórmula" },
  replay: { en: "Write it again", "pt-BR": "Escrever de novo" },
  galleryTitle: { en: "Gallery", "pt-BR": "Galeria" },
  galleryHint: { en: "contextEquation's gallery: choose an example to open it in the editor.", "pt-BR": "A galeria do contextEquation: escolha um exemplo para abri-lo no editor." },
  gallerySearch: { en: "Search the gallery", "pt-BR": "Buscar na galeria" },
  settingsTitle: { en: "contextEquation's options", "pt-BR": "Opções do contextEquation" },
  close: { en: "Close", "pt-BR": "Fechar" },
  font: { en: "Font", "pt-BR": "Fonte" },
  fontDefault: { en: "STIX Two Math (built in)", "pt-BR": "STIX Two Math (embutida)" },
  fontLoading: { en: "Downloading…", "pt-BR": "Baixando…" },
  fontCatalogFailed: { en: "The font catalog could not be read (offline?).", "pt-BR": "Não foi possível ler o catálogo de fontes (sem internet?)." },
  coverage: { en: "of the symbols", "pt-BR": "dos símbolos" },
  style: { en: "Style", "pt-BR": "Estilo" },
  display: { en: "Display", "pt-BR": "Destaque" },
  inline: { en: "Inline", "pt-BR": "Em linha" },
  lineBreaks: { en: "Break long formulas (outputs)", "pt-BR": "Quebrar fórmulas longas (saídas)" },
  maxWidth: { en: "Maximum width", "pt-BR": "Largura máxima" },
  brailleAuto: { en: "Language's", "pt-BR": "Do idioma" },
  animate: { en: "Written stroke by stroke (SVG output)", "pt-BR": "Escrita traço a traço (saída SVG)" },
  strict: { en: "Strict mode: refuse invalid LaTeX, as TeX does", "pt-BR": "Modo estrito: recusar LaTeX inválido, como o TeX" },
  pngSize: { en: "PNG size", "pt-BR": "Tamanho do PNG" },
  blockTitle: { en: "LaTeX of this block", "pt-BR": "LaTeX deste bloco" },
  blockHint: { en: "Ctrl+Enter or ⌘+Enter to finish · Esc to cancel", "pt-BR": "Ctrl+Enter ou ⌘+Enter conclui · Esc cancela" },
  blockDone: { en: "Done", "pt-BR": "Pronto" },
  blockCancel: { en: "Cancel", "pt-BR": "Cancelar" },
  blockEdit: { en: "Double-click to edit its LaTeX", "pt-BR": "Clique duas vezes para editar o LaTeX" },
  blockOk: { en: "contextEquation reads it without a warning", "pt-BR": "O contextEquation lê sem avisos" },
  rowAbove: { en: "Line above", "pt-BR": "Linha acima" },
  rowBelow: { en: "Line below", "pt-BR": "Linha abaixo" },
  colLeft: { en: "Column to the left", "pt-BR": "Coluna à esquerda" },
  colRight: { en: "Column to the right", "pt-BR": "Coluna à direita" },
  rowRemove: { en: "Remove this line", "pt-BR": "Remover esta linha" },
  colRemove: { en: "Remove this column", "pt-BR": "Remover esta coluna" },
  cellHint: { en: "Drag from one cell to another, or Alt+arrow, to draw an arrow", "pt-BR": "Arraste de uma célula a outra, ou Alt+seta, para criar uma seta" },
  arrowHint: { en: "Type the label · Backspace in an empty label removes the arrow", "pt-BR": "Digite o rótulo · Backspace no rótulo vazio apaga a seta" },
  editLatex: { en: "Edit as LaTeX", "pt-BR": "Editar como LaTeX" },
  blockBarHint: { en: "Kept as written · double click or Enter beside it also opens it", "pt-BR": "Mantido como escrito · clique duplo ou Enter ao lado também abre" },
  chVertex: { en: "Carbon at a vertex (no label)", "pt-BR": "Carbono no vértice (sem rótulo)" },
  chElement: { en: "Element", "pt-BR": "Elemento" },
  chLabel: { en: "Atom's label: anything (OH, CH_3, R')", "pt-BR": "Rótulo do átomo: qualquer um (OH, CH_3, R')" },
  chLabelPlaceholder: { en: "Label", "pt-BR": "Rótulo" },
  chBondTo: { en: "Bond towards", "pt-BR": "Ligação para" },
  chSingle: { en: "Single bond", "pt-BR": "Ligação simples" },
  chDouble: { en: "Double bond", "pt-BR": "Ligação dupla" },
  chTriple: { en: "Triple bond", "pt-BR": "Ligação tripla" },
  chWedge: { en: "Wedge (towards you)", "pt-BR": "Cunha (para frente)" },
  chHash: { en: "Hashed wedge (away from you)", "pt-BR": "Cunha tracejada (para trás)" },
  chBenzene: { en: "Benzene ring", "pt-BR": "Anel benzênico" },
  chHexagon: { en: "Six-membered ring", "pt-BR": "Anel de seis" },
  chPentagon: { en: "Five-membered ring", "pt-BR": "Anel de cinco" },
  chRemove: { en: "Remove", "pt-BR": "Apagar" },
  chAtomHint: { en: "Type the label, here or in the formula · Alt+arrow or drag adds a bond · arrows move between atoms", "pt-BR": "Digite o rótulo, aqui ou na fórmula · Alt+seta ou arrastar cria ligação · setas andam entre átomos" },
  chBondHint: { en: "Pick the bond's kind · Backspace removes the atom at its end", "pt-BR": "Escolha o tipo de ligação · Backspace apaga o átomo da ponta" },
  rxStructure: { en: "Add a structure", "pt-BR": "Adicionar estrutura" },
  rxNoReaction: { en: "Arrow of a reaction that does not happen", "pt-BR": "Seta de reação que não ocorre" },
  rxSchemeHint: { en: "Type a structure, a space, -> and the next one · [ gives the conditions", "pt-BR": "Digite uma estrutura, espaço, -> e a próxima · [ dá as condições" },
  rxForward: { en: "Reaction", "pt-BR": "Reação" },
  rxEquilibrium: { en: "Equilibrium", "pt-BR": "Equilíbrio" },
  rxEquilibriumRight: { en: "Equilibrium to the right", "pt-BR": "Equilíbrio deslocado à direita" },
  rxEquilibriumLeft: { en: "Equilibrium to the left", "pt-BR": "Equilíbrio deslocado à esquerda" },
  rxResonance: { en: "Resonance", "pt-BR": "Ressonância" },
  rxBackward: { en: "Reverse reaction", "pt-BR": "Reação inversa" },
  rxAbove: { en: "Condition above the arrow", "pt-BR": "Condição acima da seta" },
  rxBelow: { en: "Condition below the arrow", "pt-BR": "Condição abaixo da seta" },
  rxRemove: { en: "Remove this piece", "pt-BR": "Apagar esta peça" },
  rxPlus: { en: "Add a species (+)", "pt-BR": "Adicionar espécie (+)" },
  rxArrow: { en: "Add an arrow", "pt-BR": "Adicionar seta" },
  rxGas: { en: "Gas released (↑)", "pt-BR": "Gás liberado (↑)" },
  rxPrecipitate: { en: "Precipitate (↓)", "pt-BR": "Precipitado (↓)" },
  rxHint: { en: "Type species as math (H2O, SO4^2-), + and -> · [ the conditions · space moves on", "pt-BR": "Digite as espécies como matemática (H2O, SO4^2-), + e -> · [ as condições · espaço avança" },
  arrowStyle: { en: "Style", "pt-BR": "Estilo" },
  stylePlain: { en: "Plain", "pt-BR": "Comum" },
  styleDashed: { en: "Dashed", "pt-BR": "Tracejada" },
  styleDotted: { en: "Dotted", "pt-BR": "Pontilhada" },
  styleHook: { en: "Injection (hook)", "pt-BR": "Injeção (gancho)" },
  styleTwohead: { en: "Surjection (two heads)", "pt-BR": "Sobrejeção (duas pontas)" },
  styleDouble: { en: "Double (implies)", "pt-BR": "Dupla (implica)" },
  styleEqual: { en: "Equal", "pt-BR": "Igual" },
  styleMapsto: { en: "Maps to", "pt-BR": "Leva em" },
  styleSquiggly: { en: "Squiggly", "pt-BR": "Ondulada" },
  bendNone: { en: "Straight", "pt-BR": "Reta" },
  bendLeft: { en: "Bend left", "pt-BR": "Curva à esquerda" },
  bendRight: { en: "Bend right", "pt-BR": "Curva à direita" },
  swapLabel: { en: "Label on the other side", "pt-BR": "Rótulo do outro lado" },
  removeArrow: { en: "Remove the arrow", "pt-BR": "Apagar a seta" },
} satisfies Record<string, Words>;

/** Escapes text for HTML. */
export function html(s: string): string {
  return s.replace(/[&<>"']/g, (c) => ({ "&": "&amp;", "<": "&lt;", ">": "&gt;", '"': "&quot;", "'": "&#39;" })[c]!);
}

/** A component that works for a field: by `for="id"` (in the same document or shadow root), by the
 * `field` property, or inside a `<context-equation-editor>`. Re-attaches when the field appears later. */
export class FieldClient extends HTMLElement {
  private _field: ContextEquationField | null = null;
  private detach: (() => void) | null = null;
  private observer: MutationObserver | null = null;

  /** The field this component works for. */
  get field(): ContextEquationField | null {
    return this._field;
  }

  set field(f: ContextEquationField | null) {
    if (f === this._field) return;
    this.detach?.();
    this.detach = null;
    this._field = f;
    if (f) this.detach = this.attach(f);
    this.fieldChanged();
  }

  /** The field's locale, or this element's. */
  get locale(): string {
    return this.getAttribute("locale") ?? this._field?.getAttribute("locale") ?? "en";
  }

  connectedCallback(): void {
    this.findField();
    if (!this._field && this.getAttribute("for")) {
      // the field may come later in the page
      this.observer = new MutationObserver(() => {
        this.findField();
        if (this._field) this.observer?.disconnect();
      });
      this.observer.observe(this.getRootNode() as Node, { childList: true, subtree: true });
    }
  }

  disconnectedCallback(): void {
    this.observer?.disconnect();
  }

  private findField(): void {
    if (this._field) return;
    const id = this.getAttribute("for");
    if (!id) return;
    const root = this.getRootNode() as Document | ShadowRoot;
    const el = root.getElementById?.(id) ?? null;
    if (el && el.localName === "context-equation-field") this.field = el as ContextEquationField;
  }

  /** Listens to the field; returns what undoes it. */
  protected attach(_field: ContextEquationField): () => void {
    return () => {};
  }

  /** Called when the field is set or changes. */
  protected fieldChanged(): void {}
}

/** Keeps focus in the field when a button is pressed, so typing goes on where it was. */
export function keepFocus(el: HTMLElement): void {
  el.addEventListener("pointerdown", (e) => {
    if ((e.target as HTMLElement).closest("button")) e.preventDefault();
  });
}
