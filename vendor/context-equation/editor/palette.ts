// <context-equation-palette> and <context-equation-toolbar>: buttons that insert into a field, each drawn by contextEquation from
// what it inserts. A palette is one grid; a toolbar is palettes in tabs.

import { init, render } from "./renderer.ts";
import { FieldClient, TOKENS, html, icon, keepFocus, say, UI } from "./shared.ts";
import { PALETTES, previewLatex, searchPalettes, type PaletteItem, type PaletteSet } from "./sets.ts";
import type { ContextEquationField } from "./field.ts";
import { openMatrixPicker } from "./matrix-picker.ts";

const drawn = new Map<string, string>();

/** A formula as inline SVG, drawn once and kept (the palettes draw the same buttons often). */
export function preview(latex: string): string {
  let svg = drawn.get(latex);
  if (svg === undefined) {
    try {
      svg = render(latex, { display: true }).svg.replace("<svg ", '<svg aria-hidden="true" ');
      drawn.set(latex, svg);
    } catch {
      // not kept (contextEquation may not be ready yet): drawn again next time
      svg = `<span>${html(latex)}</span>`;
    }
  }
  return svg;
}

/** Waits for contextEquation's WebAssembly. */
export function contextEquationReady(): Promise<unknown> {
  return init();
}

/** What a button shows: an icon, a label, or the formula it inserts. */
export function face(item: PaletteItem): string {
  if (item.icon && item.label) return `${icon(item.icon)}<span class="label">${html(item.label)}</span>`;
  if (item.icon) return icon(item.icon);
  if (item.label) return `<span class="label">${html(item.label)}</span>`;
  return preview(previewLatex(item));
}

/** Runs a palette item on a field (`button`: where a picker opens). */
export function apply(field: ContextEquationField, item: PaletteItem, button?: HTMLElement): void {
  if (item.picker && button) return void openMatrixPicker(button, field, field.locale);
  // the focus first: what the item opens (a block's LaTeX panel) takes it from there
  field.focus();
  if (item.action) field.run(item.action);
  else if (item.insert !== undefined) field.run(`latex:${item.insert}`);
}

/** A palette button's HTML. */
function buttonHtml(item: PaletteItem, i: number, locale: string): string {
  const name = html(say(item.title, locale));
  return `<button type="button" part="button" data-i="${i}" title="${name}" aria-label="${name}"${item.wide ? ' class="wide"' : ""}>${face(item)}</button>`;
}

export const BUTTON_STYLE = `
.grid { display: grid; grid-template-columns: repeat(auto-fill, minmax(var(--context-equation-button-size, 44px), 1fr)); gap: 4px; padding: 6px; }
.grid button, .keys button {
  display: grid; place-items: center; min-height: var(--context-equation-button-size, 44px); padding: 4px;
  color: var(--_ink); background: var(--_key); border: 1px solid var(--_border); border-radius: var(--_radius);
  cursor: pointer; font: inherit; transition: border-color 0.12s, transform 0.12s, background 0.12s;
}
.grid button:hover, .keys button:hover { border-color: var(--_accent); }
.grid button:active, .keys button:active { transform: translateY(1px); background: var(--_key-active); }
.grid button:focus-visible, .keys button:focus-visible, .tabs button:focus-visible { outline: 2px solid var(--_focus); outline-offset: 1px; }
.grid button.wide { grid-column: span 2; }
.grid button:has(> svg + .label) { display: flex; justify-content: center; gap: 6px; }
.grid button svg, .keys button svg { max-width: 100%; height: auto; max-height: calc(var(--context-equation-button-size, 44px) - 14px); width: auto; font-size: 15px; }
.grid g[data-empty], .keys g[data-empty], .menu g[data-empty] { fill: var(--_slot); }
.label { font-size: 13px; }
@media (prefers-reduced-motion: reduce) { .grid button, .keys button { transition: none; } }
`;

const PALETTE_STYLE = `
${TOKENS}
:host { display: block; color: var(--_ink); }
:host([hidden]) { display: none; }
${BUTTON_STYLE}
`;

/** One grid of buttons. `set="greek"` picks a palette that comes with the editor; `items` (or `set`
 * as an object) gives your own. */
export class ContextEquationPalette extends FieldClient {
  static observedAttributes = ["set", "locale"];
  private _items: PaletteItem[] | null = null;
  private grid: HTMLDivElement;

  constructor() {
    super();
    const root = this.attachShadow({ mode: "open" });
    root.innerHTML = `<style>${PALETTE_STYLE}</style><div class="grid" part="grid" role="toolbar"></div>`;
    this.grid = root.querySelector(".grid")!;
    keepFocus(this.grid);
    this.grid.addEventListener("click", (e) => {
      const b = (e.target as HTMLElement).closest("button");
      if (!b || !this.field) return;
      apply(this.field, this.items[Number(b.dataset.i)], b);
    });
  }

  /** The buttons: your own, or those of the palette named by `set`. */
  get items(): PaletteItem[] {
    return this._items ?? PALETTES.find((p) => p.id === this.getAttribute("set"))?.items ?? PALETTES[0].items;
  }

  set items(items: PaletteItem[]) {
    this._items = items;
    this.paint();
  }

  override connectedCallback(): void {
    super.connectedCallback();
    contextEquationReady().then(() => this.paint());
  }

  attributeChangedCallback(): void {
    contextEquationReady().then(() => this.paint());
  }

  protected override fieldChanged(): void {
    contextEquationReady().then(() => this.paint());
  }

  private paint(): void {
    const locale = this.locale;
    this.grid.setAttribute("aria-label", say(PALETTES.find((p) => p.id === this.getAttribute("set"))?.title ?? UI.palettes, locale));
    this.grid.innerHTML = this.items.map((item, i) => buttonHtml(item, i, locale)).join("");
  }
}

const TOOLBAR_STYLE = `
${TOKENS}
:host { display: block; container-type: inline-size; color: var(--_ink); background: var(--_surface); border: 1px solid var(--_border); border-radius: var(--_radius); overflow: hidden; }
:host([hidden]) { display: none; }
.head { display: flex; align-items: flex-end; gap: 8px; padding: 6px 6px 0; border-bottom: 1px solid var(--_border); }
.tabs { display: flex; gap: 2px; flex: 1; min-width: 0; overflow-x: auto; scrollbar-width: thin; }
:host([wrap]) .tabs { flex-wrap: wrap; overflow: visible; }
.tabs[data-dim] button { opacity: 0.55; }
.search { box-sizing: border-box; flex: 0 1 min(260px, 40vw); display: flex; align-items: center; gap: 6px; margin-bottom: 6px; padding: 0 10px; height: 32px; width: min(260px, 40vw); max-width: 100%; min-width: 0; color: var(--_muted); background: var(--_background); border: 1px solid var(--_border); border-radius: 999px; transition: border-color 0.15s, width 0.2s; }
.search:focus-within { border-color: var(--_focus); color: var(--_ink); }
.search input { flex: 1; min-width: 0; border: 0; outline: 0; background: none; color: var(--_ink); font: 13px var(--_font); }
.search input::placeholder { color: var(--_muted); }
:host([nosearch]) .search { display: none; }
.panel { max-height: var(--context-equation-palette-height, none); overflow-y: auto; }
@container (max-width: 1000px) {
  .head { flex-wrap: wrap; }
  .tabs { flex: 1 1 100%; }
  .search { flex: 1 1 100%; width: 100%; order: -1; margin: 0 0 4px; }
}
.empty { padding: 14px; color: var(--_muted); font-size: 13px; }
.tabs button { flex: none; font: 600 13px var(--_font); color: var(--_muted); background: none; border: 0; padding: 7px 10px 8px; border-radius: var(--_radius) var(--_radius) 0 0; cursor: pointer; transition: color 0.12s, background 0.12s; }
.tabs button[aria-selected="true"] { color: var(--_ink); background: var(--_background); box-shadow: inset 0 -2px 0 var(--_accent); }
.panel { background: var(--_background); }
.panel.fresh .grid { animation: fade 0.18s ease-out; }
@keyframes fade { from { opacity: 0; transform: translateY(-2px); } }
@media (prefers-reduced-motion: reduce) { .panel.fresh .grid { animation: none; } .tabs button { transition: none; } }
${BUTTON_STYLE}
`;

/** Palettes in tabs. `palettes="structures calculus greek"` picks and orders those that come with the
 * editor; the `palettes` property takes your own. */
export class ContextEquationToolbar extends FieldClient {
  static observedAttributes = ["palettes", "locale"];
  private _sets: PaletteSet[] | null = null;
  private current = 0;
  private tabs: HTMLDivElement;
  private panel: HTMLDivElement;

  constructor() {
    super();
    const root = this.attachShadow({ mode: "open" });
    root.innerHTML = `<style>${TOOLBAR_STYLE}</style><div class="head" part="head"><div class="tabs" part="tabs" role="tablist"></div><label class="search" part="search">${icon("search")}<input type="search" id="q" autocomplete="off" spellcheck="false"></label></div><div class="panel" part="panel" role="tabpanel"><div class="grid" part="grid"></div></div>`;
    this.tabs = root.querySelector(".tabs")!;
    this.panel = root.querySelector(".panel")!;
    this.query = root.querySelector("input")!;
    this.query.addEventListener("input", () => this.paint());
    this.query.addEventListener("keydown", (e) => {
      if (e.key === "Escape") {
        this.query.value = "";
        this.paint();
        this.field?.focus();
      } else if (e.key === "Enter" && this.shownItems[0] && this.field) {
        e.preventDefault();
        apply(this.field, this.shownItems[0], this.panel.querySelector("button") ?? undefined);
      }
    });
    keepFocus(this.tabs);
    keepFocus(this.panel);
    this.tabs.addEventListener("click", (e) => {
      const b = (e.target as HTMLElement).closest("button");
      if (!b) return;
      this.query.value = "";
      this.current = Number(b.dataset.i);
      this.panel.classList.add("fresh");
      this.paint();
    });
    this.tabs.addEventListener("keydown", (e) => {
      if (e.key !== "ArrowLeft" && e.key !== "ArrowRight") return;
      const n = this.sets.length;
      this.current = (this.current + (e.key === "ArrowRight" ? 1 : n - 1)) % n;
      this.paint();
      (this.tabs.children[this.current] as HTMLElement).focus();
    });
    this.panel.addEventListener("click", (e) => {
      const b = (e.target as HTMLElement).closest("button");
      if (!b || !this.field) return;
      apply(this.field, this.shownItems[Number(b.dataset.i)], b);
    });
  }

  /** The buttons shown: the current palette's, or what the search found. */
  private shownItems: PaletteItem[] = [];
  private query: HTMLInputElement;

  /** The palettes in the tabs. */
  get palettes(): PaletteSet[] {
    return this.sets;
  }

  set palettes(sets: PaletteSet[]) {
    this._sets = sets;
    this.current = 0;
    this.paint();
  }

  private get sets(): PaletteSet[] {
    if (this._sets) return this._sets;
    const names = this.getAttribute("palettes")?.split(/[\s,]+/).filter(Boolean);
    return names ? names.map((n) => PALETTES.find((p) => p.id === n)).filter((p): p is PaletteSet => !!p) : PALETTES;
  }

  override connectedCallback(): void {
    super.connectedCallback();
    contextEquationReady().then(() => this.paint());
  }

  attributeChangedCallback(): void {
    contextEquationReady().then(() => this.paint());
  }

  protected override fieldChanged(): void {
    contextEquationReady().then(() => this.paint());
  }

  private paint(): void {
    const locale = this.locale;
    const sets = this.sets;
    this.current = Math.min(this.current, Math.max(0, sets.length - 1));
    this.tabs.setAttribute("aria-label", say(UI.palettes, locale));
    this.tabs.innerHTML = sets
      .map((s, i) => `<button type="button" role="tab" part="tab" data-i="${i}" aria-selected="${i === this.current}" tabindex="${i === this.current ? 0 : -1}">${html(say(s.title, locale))}</button>`)
      .join("");
    this.query.placeholder = say(UI.search, locale);
    this.query.setAttribute("aria-label", say(UI.search, locale));
    const q = this.query.value.trim();
    if (q) {
      this.tabs.setAttribute("data-dim", "");
      this.shownItems = searchPalettes(q, sets);
      this.panel.setAttribute("aria-label", say(UI.results, locale));
      this.panel.innerHTML = this.shownItems.length
        ? `<div class="grid" part="grid">${this.shownItems.map((item, i) => buttonHtml(item, i, locale)).join("")}</div>`
        : `<div class="empty">${html(say(UI.noResults, locale))}</div>`;
      return;
    }
    this.tabs.removeAttribute("data-dim");
    const set = sets[this.current];
    if (!set) return;
    this.shownItems = set.items;
    this.panel.setAttribute("aria-label", say(set.title, locale));
    this.panel.innerHTML = `<div class="grid" part="grid">${set.items.map((item, i) => buttonHtml(item, i, locale)).join("")}</div>`;
  }
}

if (typeof customElements !== "undefined") {
  if (!customElements.get("context-equation-palette")) customElements.define("context-equation-palette", ContextEquationPalette);
  if (!customElements.get("context-equation-toolbar")) customElements.define("context-equation-toolbar", ContextEquationToolbar);
}
