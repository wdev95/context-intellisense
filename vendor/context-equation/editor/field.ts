// <context-equation-field>: the box where a formula is built. contextEquation draws it at every change; the boxes it
// returns place the cursor and the selection and read the clicks.

import { cleanLabel, labelText } from "./structure.ts";
import { init, render, type Options, type Result } from "./renderer.ts";
import { Editor, classOfTree, type ChangeKind } from "./model.ts";
import { arrowAt, arrowPath, atomRect, blockAt, boxesById, caretRect, cellAt, cellRect, cursorAt, selectionRect, type Box, type Rect } from "./geometry.ts";
import type { ArrowStyle, BlockSession, ChemSelection, Diagram, Raw, Reaction } from "./model.ts";
import { icon } from "./shared.ts";
import { TOKENS, UI, say } from "./shared.ts";

const STYLE = `
${TOKENS}
:host {
  display: block;
  font-size: 1.5em;
  color: var(--_ink);
  -webkit-tap-highlight-color: transparent;
}
:host { position: relative; }
:host([inline]) { display: inline-block; font-size: inherit; vertical-align: middle; }
:host([hidden]) { display: none; }
.box {
  position: relative;
  box-sizing: border-box;
  height: 100%;
  display: flex;
  /* "safe": a formula taller or wider than the box starts at its top or left edge, where it can scroll from */
  align-items: safe center;
  justify-content: var(--context-equation-align, safe center);
  min-height: 1.6em;
  padding: var(--context-equation-padding, 0.6em 0.8em);
  background: var(--_background);
  border: 1px solid var(--_border);
  border-radius: var(--_radius);
  cursor: text;
  overflow: auto;
  touch-action: manipulation;
  transition: border-color 0.15s, box-shadow 0.15s;
}
:host([inline]) .box { min-height: 1.3em; padding: var(--context-equation-padding, 0.15em 0.35em); min-width: 3em; }
:host(:focus-within) .box { border-color: var(--_focus); box-shadow: 0 0 0 3px color-mix(in srgb, var(--_focus) 22%, transparent); }
:host([readonly]) .box { cursor: default; }
.drawing { position: relative; line-height: 0; flex: none; }
.drawing svg { display: block; overflow: visible; }
:host([fit]) .box { overflow: hidden; }
:host([fit]) .drawing, :host([fit]) .drawing svg { max-width: 100%; }
:host([fit]) .drawing svg { height: auto; }
g[data-empty] { fill: var(--_slot); }
.overlay { position: absolute; pointer-events: none; }
.caret { background: var(--_caret); border-radius: 1px; animation: blink 1.05s steps(1) infinite; }
.selection { background: var(--_selection); border-radius: 2px; }
:host(:not(:focus-within)) .caret, :host([readonly]) .caret { display: none; }
@keyframes blink { 50% { opacity: 0; } }
@media (prefers-reduced-motion: reduce) { .caret { animation: none; } .box { transition: none; } }
:host([data-empty]:not(:focus-within)) .drawing { visibility: hidden; }
.placeholder { position: absolute; left: var(--context-equation-placeholder-left, 0.8em); color: var(--_muted); font-size: 0.6em; pointer-events: none; font-family: var(--_font); }
textarea { position: absolute; opacity: 0; width: 1px; height: 1px; padding: 0; border: 0; margin: 0; resize: none; overflow: hidden; left: 0; top: 0; pointer-events: none; }
.hover { border: 1.5px dashed var(--_accent); border-radius: 4px; background: color-mix(in srgb, var(--_accent) 6%, transparent); }
.box.over-block { cursor: pointer; }
.block { position: absolute; z-index: 5; display: grid; gap: 8px; width: min(560px, calc(100% - 16px)); padding: 12px; color: var(--_ink); background: var(--_background); border: 1px solid var(--_border); border-radius: calc(var(--_radius) + 2px); box-shadow: 0 14px 36px rgb(0 0 0 / 0.2); font: 13px var(--_font); animation: drop 0.14s ease-out; }
.block[hidden] { display: none; }
@keyframes drop { from { opacity: 0; transform: translateY(-4px); } }
.block .title { display: flex; justify-content: space-between; gap: 10px; color: var(--_muted); font-size: 12px; }
.block .title b { color: var(--_ink); font-weight: 600; }
.block textarea.src { position: static; opacity: 1; width: 100%; height: auto; min-height: 4.5em; max-height: 14em; box-sizing: border-box; padding: 8px 10px; resize: vertical; overflow: auto; pointer-events: auto; color: var(--_ink); background: var(--_surface); border: 1px solid var(--_border); border-radius: var(--_radius); font: 14px/1.5 var(--_mono); }
.block textarea.src:focus { outline: 2px solid var(--_focus); outline-offset: 0; }
.block .status { min-height: 1.4em; color: var(--_muted); font-size: 12px; }
.block .status.warn { color: light-dark(#8a4b00, #f0c27a); }
.block .actions { display: flex; justify-content: flex-end; gap: 8px; }
.block button { height: 30px; padding: 0 14px; border-radius: var(--_radius); font: 600 13px var(--_font); cursor: pointer; border: 1px solid var(--_border); background: var(--_key); color: var(--_ink); }
.block button.primary { background: var(--_accent); border-color: var(--_accent); color: var(--_background); }
.block button:focus-visible { outline: 2px solid var(--_focus); outline-offset: 1px; }
@media (prefers-reduced-motion: reduce) { .block { animation: none; } }
.dgov { position: absolute; pointer-events: none; overflow: visible; }
.dgov .cell { fill: none; stroke: var(--_border); stroke-width: 1.2; stroke-dasharray: 4 3; vector-effect: non-scaling-stroke; }
.dgov .cell.here { stroke: var(--_accent); stroke-dasharray: none; fill: color-mix(in srgb, var(--_accent) 7%, transparent); }
.dgov .cell.target { stroke: var(--_slot); stroke-width: 2; stroke-dasharray: none; fill: color-mix(in srgb, var(--_slot) 12%, transparent); }
.dgov .arrow { fill: none; stroke: var(--_accent); stroke-width: 9; stroke-linecap: round; opacity: 0.28; vector-effect: non-scaling-stroke; }
.dgov .atom { fill: color-mix(in srgb, var(--_accent) 4%, transparent); stroke: var(--_border); stroke-width: 1.2; stroke-dasharray: 3 3; vector-effect: non-scaling-stroke; }
.dgov .atom.here { stroke: var(--_accent); stroke-width: 2; stroke-dasharray: none; fill: color-mix(in srgb, var(--_accent) 12%, transparent); }
.box.chem .caret { visibility: hidden; }
.dgov .ghost { fill: none; stroke: var(--_slot); stroke-width: 2; stroke-dasharray: 6 4; vector-effect: non-scaling-stroke; }
.dbar { position: absolute; z-index: 4; display: flex; flex-wrap: wrap; align-items: center; gap: 4px; max-width: calc(100% - 16px); padding: 6px; background: var(--_background); border: 1px solid var(--_border); border-radius: calc(var(--_radius) + 2px); box-shadow: 0 8px 24px rgb(0 0 0 / 0.14); font: 12px var(--_font); animation: drop 0.14s ease-out; }
.dbar[hidden] { display: none; }
/* the bar is for editing: it goes with the caret when the field loses the focus, so it never sits over the page */
:host(:not(:focus-within)) .dbar { display: none; }
.dbar button { display: inline-grid; place-items: center; min-width: 30px; height: 30px; padding: 0 8px; color: var(--_ink); background: var(--_key); border: 1px solid var(--_border); border-radius: var(--_radius); font: 15px var(--_font); cursor: pointer; }
.dbar button.text { font-size: 12px; }
.dbar button[aria-pressed="true"] { border-color: var(--_accent); color: var(--_accent); background: color-mix(in srgb, var(--_accent) 10%, transparent); }
.dbar button:hover { border-color: var(--_accent); }
.dbar button:focus-visible { outline: 2px solid var(--_focus); outline-offset: 1px; }
.dbar .sep { width: 1px; height: 22px; background: var(--_border); margin: 0 4px; }
.dbar .hint { color: var(--_muted); padding: 0 6px; }
.dbar.compact .hint, .dbar.compact .line:has(> .hint:only-child) { display: none; }
.dbar:has(> .line) { flex-direction: column; align-items: flex-start; }
.dbar .line { display: flex; flex-wrap: wrap; align-items: center; gap: 4px; }
.dbar input.label { width: 9em; height: 30px; box-sizing: border-box; padding: 0 8px; color: var(--_ink); background: var(--_background); border: 1px solid var(--_border); border-radius: var(--_radius); font: 15px var(--_font); }
.dbar input.label:focus { outline: 2px solid var(--_focus); outline-offset: 1px; border-color: var(--_focus); }
.box.dragging-arrow { cursor: crosshair; }
.live { position: absolute; width: 1px; height: 1px; overflow: hidden; clip: rect(0 0 0 0); white-space: nowrap; }
`;

let ready: Promise<unknown> | null = null;
let loaded = false;

export interface InputDetail {
  /** The formula as plain LaTeX. */
  latex: string;
}

/** Sent while a command is typed after `\` (`active`), and when it ends: what a command menu follows. */
export interface CommandDetail {
  /** The letters typed so far. */
  name: string;
  active: boolean;
  /** Where the command is on the screen (viewport pixels). */
  rect: DOMRect | null;
}

/** The time contextEquation and the cursor took for the last change, in milliseconds. */
export interface Timing {
  render: number;
  total: number;
}

/** A key combination (`Mod+B`, `Shift+Enter`, `Alt+ArrowUp`; `Mod` is Ctrl, or ⌘ on a Mac) and the action it runs (see `run`). */
export type Keymap = Record<string, string>;

const IS_MAC = typeof navigator !== "undefined" && /Mac|iPhone|iPad/.test(navigator.platform ?? "");

/** What the keys do, unless `keymap` says otherwise. */
export const DEFAULT_KEYMAP: Keymap = {
  ArrowLeft: "moveLeft",
  ArrowRight: "moveRight",
  ArrowUp: "moveUp",
  ArrowDown: "moveDown",
  "Shift+ArrowLeft": "selectLeft",
  "Shift+ArrowRight": "selectRight",
  Home: "home",
  End: "end",
  "Shift+Home": "selectHome",
  "Shift+End": "selectEnd",
  Backspace: "backspace",
  Delete: "deleteForward",
  Enter: "enter",
  Tab: "tab",
  "Shift+Tab": "tabBack",
  "Mod+A": "selectAll",
  "Mod+Z": "undo",
  "Mod+Shift+Z": "redo",
  "Mod+Y": "redo",
  "Mod+Enter": "addRow",
  "Mod+Shift+Enter": "addColumn",
  "Shift+Enter": "addColumn",
  "Alt+ArrowUp": "arrowUp",
  "Alt+ArrowDown": "arrowDown",
  "Alt+ArrowLeft": "arrowLeft",
  "Alt+ArrowRight": "arrowRight",
  "Alt+Shift+ArrowUp": "arrowUp:dashed",
  "Alt+Shift+ArrowDown": "arrowDown:dashed",
  "Alt+Shift+ArrowLeft": "arrowLeft:dashed",
  "Alt+Shift+ArrowRight": "arrowRight:dashed",
};

/** The key combination of a keyboard event, in the keymap's notation. */
function comboOf(e: KeyboardEvent): string {
  const parts: string[] = [];
  if (IS_MAC ? e.metaKey : e.ctrlKey) parts.push("Mod");
  if (e.altKey) parts.push("Alt");
  if (e.shiftKey) parts.push("Shift");
  parts.push(e.key.length === 1 ? e.key.toUpperCase() : e.key);
  return parts.join("+");
}

export class ContextEquationField extends HTMLElement {
  static observedAttributes = ["locale", "value", "inline", "readonly", "placeholder", "aria-label"];

  /** The editor without a screen: the formula, the cursor, the commands. */
  readonly editor = new Editor();
  /** How long the last change took. */
  lastTiming: Timing = { render: 0, total: 0 };
  /** Keys added to (or replacing) `DEFAULT_KEYMAP`; an empty string turns a key off. */
  keymap: Keymap = {};

  private _contextEquationOptions: Partial<Options> = {};
  private drawing: HTMLDivElement;
  private caret: HTMLDivElement;
  private selectionBox: HTMLDivElement;
  private input: HTMLTextAreaElement;
  private placeholderEl: HTMLDivElement;
  private live: HTMLDivElement;
  private box: HTMLDivElement;
  private boxes = new Map<string, Box>();
  private plain: Result | null = null;
  private refusal: string | null = null;
  private latexAtFocus = "";
  private dragging = false;
  private liveTimer = 0;
  private commandShown = false;
  private hoverBox!: HTMLDivElement;
  private blockPanel!: HTMLDivElement;
  private blockSource!: HTMLTextAreaElement;
  private block: BlockSession | null = null;
  private dgov!: SVGSVGElement;
  private dbar!: HTMLDivElement;
  /** A drag from a cell of a diagram: an arrow being drawn once it leaves the cell. */
  private chemDrag: { atom: Raw; k: number; x0: number; y0: number; x: number; y: number; active: boolean } | null = null;
  /** The kind of bond the structure bar draws next. */
  private chemKind = "single";
  /** An arrow being drawn by dragging: to a cell (`to`), or out past the cells, to a new neighbour one step that way (`out`). */
  private arrowDrag: { diagram: Diagram; from: [number, number]; to: [number, number] | null; out?: [number, number] | null; x: number; y: number; active: boolean } | null = null;
  private dbarKey = "";
  /** The marked LaTeX last drawn. */
  private drawnMarked = "";
  private ownValue: string | null = null;

  constructor() {
    super();
    const root = this.attachShadow({ mode: "open", delegatesFocus: true });
    root.innerHTML = `<style>${STYLE}</style><div class="box" part="box"><div class="drawing" part="drawing"></div><div class="overlay selection" part="selection" hidden></div><div class="overlay caret" part="caret"></div><div class="overlay hover" part="block-hover" hidden></div><svg class="dgov" part="diagram-overlay" aria-hidden="true"></svg><div class="placeholder" part="placeholder" hidden></div><textarea part="input" autocapitalize="off" autocomplete="off" autocorrect="off" spellcheck="false"></textarea><div class="live" aria-live="polite"></div></div><div class="dbar" part="diagram-bar" role="toolbar" hidden></div><div class="block" part="block-editor" role="dialog" hidden><div class="title"><b></b><span class="hint"></span></div><textarea class="src" spellcheck="false" autocapitalize="off" autocomplete="off"></textarea><div class="status" aria-live="polite"></div><div class="actions"><button type="button" data-block="cancel"></button><button type="button" class="primary" data-block="done"></button></div></div>`;
    this.box = root.querySelector(".box")!;
    this.drawing = root.querySelector(".drawing")!;
    this.caret = root.querySelector(".caret")!;
    this.selectionBox = root.querySelector(".selection")!;
    this.placeholderEl = root.querySelector(".placeholder")!;
    this.input = root.querySelector("textarea")!;
    this.live = root.querySelector(".live")!;
    this.hoverBox = root.querySelector(".hover")!;
    this.dgov = root.querySelector(".dgov")!;
    this.dbar = root.querySelector(".dbar")!;
    this.dbar.addEventListener("pointerdown", (e) => {
      // the label's box takes the focus; the buttons leave it in the formula
      if ((e.target as HTMLElement).closest("input")) return e.stopPropagation();
      e.preventDefault();
      e.stopPropagation();
    });
    this.dbar.addEventListener("click", (e) => {
      const b = (e.target as HTMLElement).closest<HTMLElement>("button[data-run]");
      if (b) this.run(b.dataset.run!);
    });
    // the atom's label, typed freely: each change goes into the structure at once
    this.dbar.addEventListener("input", (e) => {
      const input = e.target as HTMLInputElement;
      const label = cleanLabel(input.value);
      if (label !== input.value) input.value = label;
      this.editor.setChemLabel(label);
    });
    this.dbar.addEventListener("keydown", (e) => {
      if (!(e.target as HTMLElement).closest("input")) return;
      e.stopPropagation();
      // Enter or Escape: back to the formula, the atom still picked
      if (e.key === "Enter" || e.key === "Escape") {
        e.preventDefault();
        this.focus();
      }
    });
    this.blockPanel = root.querySelector(".block")!;
    this.blockSource = root.querySelector("textarea.src")!;
    this.blockSource.addEventListener("input", () => this.blockInput());
    this.blockSource.addEventListener("keydown", (e) => {
      e.stopPropagation();
      if (e.key === "Escape") {
        e.preventDefault();
        this.closeBlock(false);
      } else if (e.key === "Enter" && (e.ctrlKey || e.metaKey)) {
        e.preventDefault();
        this.closeBlock(true);
      }
    });
    this.blockPanel.addEventListener("click", (e) => {
      const b = (e.target as HTMLElement).closest<HTMLElement>("[data-block]");
      if (b) this.closeBlock(b.dataset.block === "done");
    });
    this.blockPanel.addEventListener("pointerdown", (e) => e.stopPropagation());
    this.box.addEventListener("pointerleave", () => (this.hoverBox.hidden = true));
    this.input.addEventListener("keydown", (e) => this.keydown(e));
    this.input.addEventListener("input", () => this.typed());
    this.input.addEventListener("copy", (e) => this.clipboard(e, "copy"));
    this.input.addEventListener("cut", (e) => this.clipboard(e, "cut"));
    this.input.addEventListener("paste", (e) => this.pasted(e));
    this.input.addEventListener("focus", () => (this.latexAtFocus = this.latex));
    this.input.addEventListener("blur", () => {
      if (this.latex !== this.latexAtFocus) this.dispatchEvent(new CustomEvent<InputDetail>("change", { detail: { latex: this.latex }, bubbles: true, composed: true }));
      this.sendCommand(false);
    });
    this.box.addEventListener("pointerdown", (e) => this.pointerdown(e));
    this.box.addEventListener("pointermove", (e) => this.pointermove(e));
    this.box.addEventListener("pointerup", () => {
      this.dragging = false;
      const chem = this.chemDrag;
      this.chemDrag = null;
      if (chem?.active) {
        const angle = this.snapAngle(chem.x - chem.x0, chem.y - chem.y0);
        this.editor.selectChem(chem.atom, "atom", chem.k);
        this.editor.addChemBond(angle, this.chemKind);
      }
      const drag = this.arrowDrag;
      this.arrowDrag = null;
      this.box.classList.remove("dragging-arrow");
      if (drag?.active && drag.to) this.editor.addArrow(drag.diagram, drag.from, drag.to);
      else if (drag?.active && drag.out) this.editor.arrowToNeighbour(drag.diagram, drag.from, drag.out[0], drag.out[1]);
      else if (drag?.active) this.placeDiagram();
    });
    this.box.addEventListener("dblclick", (e) => {
      // in a structure, a click picks atoms and bonds; its LaTeX opens from its bar
      if (this.editor.chemSelection()) return;
      const b = this.blockUnder(e);
      if (b && !this.readOnly) this.openBlock(b);
      else this.selectRow();
    });
    this.editor.onChange((kind) => this.changed(kind));
    // LaTeX the editor keeps as written gets the class contextEquation reads it into
    this.editor.classifyRaw = (latex) => classOfTree(render(latex, { locale: this.locale as Options["locale"] }).tree);
    // structures (\chemfig): contextEquation says which atoms and bonds they have and where each is written
    this.editor.parseChem = (latex) => render(latex).tree;
    if (typeof ResizeObserver !== "undefined") new ResizeObserver(() => this.placeOverlays()).observe(this.box);
  }

  connectedCallback(): void {
    if (!this.hasAttribute("tabindex")) this.tabIndex = 0;
    ready ??= init();
    ready.then(() => {
      const value = this.getAttribute("value");
      if (value !== null && this.editor.isEmpty()) this.editor.setLatex(value);
      else this.draw();
    });
  }

  attributeChangedCallback(name: string, old: string | null, value: string | null): void {
    if (old === value) return;
    if (name === "value") {
      // a value written back by the page from the field's own `input` changes nothing
      if (value !== null && value !== this.latex && value !== this.ownValue && ready) ready.then(() => this.editor.setLatex(value));
      return;
    }
    if (name === "readonly") this.input.readOnly = value !== null;
    if (this.isConnected && ready) ready.then(() => this.draw());
  }

  // ---------- public API ----------

  /** The formula as plain LaTeX. Setting it replaces the formula (it can be undone). */
  get value(): string {
    return this.editor.toLatex();
  }

  set value(latex: string) {
    if (latex !== this.value) this.editor.setLatex(latex);
  }

  /** The formula as plain LaTeX (the same as `value`). */
  get latex(): string {
    return this.editor.toLatex();
  }

  get locale(): string {
    return this.getAttribute("locale") ?? "en";
  }

  get readOnly(): boolean {
    return this.hasAttribute("readonly");
  }

  /** More options for contextEquation (a font from `loadFont`, fallback fonts…); the field sets `locale` and `display`. */
  get contextEquationOptions(): Partial<Options> {
    return this._contextEquationOptions;
  }

  set contextEquationOptions(o: Partial<Options>) {
    this._contextEquationOptions = o;
    if (ready) ready.then(() => this.draw());
  }

  /** Everything contextEquation gives for the formula, with all of `contextEquationOptions`: SVG, MathML, speech, braille… (null while it is empty, or refused). */
  get output(): Result | null {
    return this.plain;
  }

  /** Why contextEquation refused the formula (in strict mode, or nested too deep); null otherwise. */
  get error(): string | null {
    return this.refusal;
  }

  /** The options of the outputs: `contextEquationOptions`, the locale, and display unless set (inline fields: text style). */
  get options(): Options {
    return { ...this._contextEquationOptions, locale: this.locale as Options["locale"], display: this._contextEquationOptions.display ?? !this.hasAttribute("inline") };
  }

  /** The options of the box itself: the font, the language and the style; a maximum width, strict
   * mode, braille and animation are for the outputs (the box is one line, edited as it is typed). */
  private get drawingOptions(): Options {
    const o = this.options;
    return { locale: o.locale, display: o.display, font: o.font, fallbackFonts: o.fallbackFonts };
  }

  /** Runs an action, as a key, a palette or a virtual keyboard does:
   * `moveLeft` `moveRight` `moveUp` `moveDown` `selectLeft` `selectRight` `home` `end` `selectHome`
   * `selectEnd` `selectAll` `backspace` `deleteForward` `enter` `tab` `tabBack` `undo` `redo` `clear`
   * `addRow` `addRowAbove` `addColumn` `addColumnLeft` `removeRow` `removeColumn` `sup` `sub`,
   * `editBlock` (the LaTeX block beside the cursor), `arrowUp` `arrowDown` `arrowLeft` `arrowRight`
   * (`:<style>` for a style: an arrow from the diagram cell the cursor is in), `arrowStyle:<style>`,
   * `arrowBend:left|right|none`, `arrowSwap`, `arrowRemove` (the arrow whose label the cursor is in),
   * `editAsLatex` (the diagram around the cursor), or `insert:<template>`, `latex:<snippet>`,
   * `type:<text>`, `symbol:<latex>`, `block:<latex>` (a block of mhchem, chemfig or a diagram, its
   * editor opened, `‸` where its cursor starts). */
  run(action: string): void {
    if (this.readOnly) return;
    const e = this.editor;
    const colon = action.indexOf(":");
    const verb = colon < 0 ? action : action.slice(0, colon);
    const arg = colon < 0 ? "" : action.slice(colon + 1);
    // in a structure, the arrows move between atoms and Alt+arrow draws a bond
    const chem = e.chemSelection();
    const towards: Record<string, number> = { moveLeft: 180, moveRight: 0, moveUp: 90, moveDown: 270, arrowLeft: 180, arrowRight: 0, arrowUp: 90, arrowDown: 270 };
    if (chem && verb in towards) {
      if (verb.startsWith("move")) e.moveChem(towards[verb]);
      else e.addChemBond(towards[verb], arg === "dashed" ? "double" : this.chemKind);
      return;
    }
    const actions: Record<string, () => void> = {
      moveLeft: () => e.moveLeft(),
      moveRight: () => e.moveRight(),
      moveUp: () => e.moveUp(),
      moveDown: () => e.moveDown(),
      selectLeft: () => e.moveLeft(true),
      selectRight: () => e.moveRight(true),
      home: () => e.home(),
      end: () => e.end(),
      selectHome: () => e.home(true),
      selectEnd: () => e.end(true),
      selectAll: () => e.selectAll(),
      backspace: () => e.backspace(),
      deleteForward: () => e.deleteForward(),
      enter: () => e.enter(),
      tab: () => e.tab(),
      tabBack: () => e.tab(true),
      undo: () => e.undo(),
      redo: () => e.redo(),
      clear: () => e.clear(),
      addRow: () => e.addRow(),
      addRowAbove: () => e.addRow(true),
      addColumn: () => e.addColumn(),
      addColumnLeft: () => e.addColumn(true),
      removeRow: () => e.removeRow(),
      removeColumn: () => e.removeColumn(),
      sup: () => e.insert("sup"),
      sub: () => e.insert("sub"),
      insert: () => e.insert(arg),
      latex: () => e.insertLatex(arg, { autoDelimiters: true }),
      type: () => e.typeText(arg),
      symbol: () => e.insertSymbol(arg),
      block: () => e.insertBlock(arg),
      arrowUp: () => e.arrowFromCursor("up", (arg || "plain") as ArrowStyle),
      arrowDown: () => e.arrowFromCursor("down", (arg || "plain") as ArrowStyle),
      arrowLeft: () => e.arrowFromCursor("left", (arg || "plain") as ArrowStyle),
      arrowRight: () => e.arrowFromCursor("right", (arg || "plain") as ArrowStyle),
      arrowStyle: () => e.setArrowStyle(arg as ArrowStyle),
      arrowBend: () => e.setArrowBend(arg === "left" || arg === "right" ? arg : null),
      arrowSwap: () => e.swapArrowLabel(),
      arrowRemove: () => e.removeArrow(),
      chemLabel: () => e.setChemLabel(arg),
      chemBond: () => e.addChemBond(Number(arg), this.chemKind),
      chemKind: () => {
        if (e.chemSelection()?.type === "bond") e.setChemBondKind(arg);
        else {
          this.chemKind = arg;
          this.dbarKey = "";
          this.placeDiagram();
        }
      },
      chemRing: () => e.addChemRing(Number(arg.replace("a", "")), arg.endsWith("a")),
      chemRemove: () => e.removeChem(),
      editAsLatex: () => {
        const s = e.chemSelection();
        if (s) {
          e.clearChem();
          this.openBlock(s.atom);
          return;
        }
        const at = e.matrixAtCursor();
        const d = at?.matrix.kind === "diagram" ? at.matrix : e.reactionAtCursor()?.reaction ?? null;
        // a block of LaTeX beside the cursor: its panel
        const block = d ? null : e.blockAtCursor();
        if (block) return void this.openBlock(block);
        if (!d) return;
        const raw = e.toBlock(d);
        if (raw) this.openBlock(raw);
      },
      rxArrow: () => e.setReactionArrow(arg),
      rxSign: () => e.addReactionSign(arg),
      rxPlace: () => e.enterReactionPlace(arg === "below" ? "below" : "above"),
      rxRemove: () => e.removeReactionPiece(),
      rxStructure: () => e.addSchemeStructure(),
      editBlock: () => {
        const b = e.blockAtCursor();
        if (b) this.openBlock(b);
      },
    };
    const f = actions[verb];
    if (!f) throw new Error(`context-equation-field: no action named ${verb}`);
    f();
  }

  /** Inserts a structure by name (`frac`, `sqrt`, `pmatrix`, `sup`…). */
  insert(template: string): void {
    this.run(template === "sup" || template === "sub" ? template : `insert:${template}`);
  }

  /** Inserts a snippet of LaTeX (`#?` an empty place, `#0` the selection). */
  insertLatex(snippet: string): void {
    this.run(`latex:${snippet}`);
  }

  /** Inserts a symbol: `\alpha`, `\pm`… */
  insertSymbol(latex: string, cls?: Parameters<Editor["insertSymbol"]>[1]): void {
    if (!this.readOnly) this.editor.insertSymbol(latex, cls);
  }

  override focus(options?: FocusOptions): void {
    this.input.focus(options);
  }

  override blur(): void {
    this.input.blur();
  }

  // ---------- drawing ----------

  private changed(kind: ChangeKind): void {
    const pending = this.editor.takePendingBlock();
    if (kind === "content") {
      this.draw();
      this.ownValue = this.latex;
      this.dispatchEvent(new CustomEvent<InputDetail>("input", { detail: { latex: this.latex }, bubbles: true, composed: true }));
    }
    // the cursor alone moved: drawn again only where the drawing follows it (an empty place shown
    // where the cursor is, the conditions of the arrow it is on)
    else if (this.editor.toLatex({ marked: true }) !== this.drawnMarked) this.draw();
    else this.placeOverlays();
    this.sendCommand(!!this.editor.commandBeingTyped());
    if (pending && !this.block) this.openBlock(pending.atom, pending.caret);
    else if (this.block) this.placeBlock();
  }

  private draw(): void {
    // a formula set before contextEquation has loaded is drawn once it has
    if (!loaded) {
      ready ??= init();
      ready.then(() => {
        loaded = true;
        this.draw();
      });
      return;
    }
    const t0 = performance.now();
    const locale = this.locale;
    this.editor.decimalComma = false;
    let r: Result | null = null;
    let renderError: string | undefined;
    try {
      this.drawnMarked = this.editor.toLatex({ marked: true });
      r = render(this.drawnMarked, this.drawingOptions);
    } catch (error) {
      // contextEquation refuses the formula with its marks (nested too deep for them): the last drawing stays
      renderError = error instanceof Error ? error.message : String(error);
    }
    const t1 = performance.now();
    if (r) {
      this.drawing.innerHTML = r.svg;
      // the field's description carries the speech: the drawing itself is hidden from screen readers
      this.drawing.querySelector("svg")?.setAttribute("aria-hidden", "true");
      this.boxes = boxesById(r.boxes as Box[]);
      // a diagram's empty cells have no mark (any changes tikz-cd's layout): a second drawing, with
      // them marked, says where they are, near enough to place them
      if (this.editor.hasEmptyDiagramCells()) {
        try {
          const probe = boxesById(render(this.editor.toLatex({ marked: true, probeEmptyCells: true }), this.drawingOptions).boxes as Box[]);
          for (const [id, b] of probe) if (b.attributes["data-empty"] && !this.boxes.has(id)) this.boxes.set(id, { ...b, x: b.x - 250, y: b.y - 500, width: 500, height: 700 });
        } catch {
          // without it, the cells are placed by their neighbours
        }
      }
      for (const g of this.drawing.querySelectorAll("g[data-empty]")) g.setAttribute("part", "slot");
    }
    // the plain formula: its speech (marks would make contextEquation read b² as "b to the power of 2") and the outputs
    const plain = this.editor.toLatex();
    this.refusal = null;
    try {
      this.plain = plain ? render(plain, this.options) : null;
    } catch (e) {
      // strict mode refuses invalid LaTeX: the box still shows it, the outputs say why
      this.plain = null;
      this.refusal = e instanceof Error ? e.message : String(e);
    }
    this.input.setAttribute("aria-label", this.getAttribute("aria-label") ?? say(UI.formula, locale));
    this.input.setAttribute("aria-description", this.plain?.speech ?? "");
    this.input.setAttribute("aria-roledescription", "math");
    this.placeholderEl.hidden = !this.editor.isEmpty() || !this.getAttribute("placeholder");
    // with a placeholder, the empty place shows only once the field has the focus
    this.toggleAttribute("data-empty", this.editor.isEmpty() && !!this.getAttribute("placeholder"));
    this.placeholderEl.textContent = this.getAttribute("placeholder") ?? "";
    this.placeOverlays();
    this.announce();
    this.lastTiming = { render: t1 - t0, total: performance.now() - t0 };
    this.dispatchEvent(new CustomEvent(r ? "context-equation-render" : "context-equation-render-error", {
      detail: renderError || "The equation renderer produced no drawing.", bubbles: true, composed: true
    }));
  }

  /** The floating bar beside what it edits (`p`, in the box's pixels): under it, else over it; when
   * neither fits (a small screen), at the edge of the field away from the cursor, never over it. */
  private placeBar(p: { left: number; top: number; width: number; height: number }): void {
    const host = this.getBoundingClientRect();
    const box = this.box.getBoundingClientRect();
    // a low field (a phone): the bar without its hint, a line less
    this.dbar.classList.toggle("compact", host.height < 320);
    const width = this.dbar.offsetWidth;
    const height = this.dbar.offsetHeight;
    let left = box.left - host.left + p.left - this.box.scrollLeft + p.width / 2 - width / 2;
    left = Math.max(8, Math.min(left, host.width - width - 8));
    // where it may go, first the best: under what it edits, over it (inside the field); just under or
    // over the field; the field's bottom or top edge. The first that keeps the cursor's line free and
    // is on the screen; else the first on the screen
    const caret = this.caret.getBoundingClientRect();
    const c0 = caret.top - host.top;
    const c1 = c0 + caret.height;
    const free = (t: number) => t + height <= c0 || t >= c1;
    const inside = (t: number) => t >= 4 && t + height <= host.height - 4;
    const seen = (t: number) => host.top + t >= 0 && host.top + t + height <= innerHeight;
    const places: [number, boolean][] = [
      [box.top - host.top + p.top + p.height + 14, true],
      [box.top - host.top + p.top - height - 14, true],
      [host.height + 6, false],
      [-height - 6, false],
      [host.height - height - 4, true],
      [4, true],
    ];
    const fits = places.filter(([t, within]) => seen(t) && (!within || inside(t))).map(([t]) => t);
    const top = fits.find(free) ?? fits[0] ?? Math.max(-host.top, Math.min(host.height + 6, innerHeight - host.top - height));
    Object.assign(this.dbar.style, { left: `${left}px`, top: `${top}px` });
  }

  /** From the SVG's coordinates to pixels inside the box. */
  private toBox(rect: Rect): { left: number; top: number; width: number; height: number } | null {
    const svg = this.drawing.querySelector("svg");
    if (!svg) return null;
    const vb = svg.viewBox.baseVal;
    const s = svg.getBoundingClientRect();
    if (!vb.width || !s.width) return null;
    const k = s.width / vb.width;
    const b = this.box.getBoundingClientRect();
    return { left: s.left - b.left + this.box.scrollLeft + (rect.x - vb.x) * k, top: s.top - b.top + (rect.y - vb.y) * k, width: rect.width * k, height: rect.height * k };
  }

  private placeOverlays(): void {
    const caret = caretRect(this.editor.cursor, this.boxes);
    const p = caret && this.toBox(caret);
    if (p) {
      Object.assign(this.caret.style, { left: `${p.left}px`, top: `${p.top}px`, width: `${Math.max(1.5, Math.min(p.width, 2.5))}px`, height: `${p.height}px` });
      // restart the blink, so the cursor shows right after it moves
      this.caret.style.animation = "none";
      void this.caret.offsetWidth;
      this.caret.style.animation = "";
      // keep the cursor in view in a long formula
      const left = p.left - this.box.scrollLeft;
      if (left < 0 || left > this.box.clientWidth) this.box.scrollLeft += left - this.box.clientWidth / 2;
      // the hidden input sits at the cursor, so a phone's keyboard and IME windows open there
      Object.assign(this.input.style, { left: `${p.left}px`, top: `${p.top}px` });
    }
    const sel = selectionRect(this.editor.selection(), this.boxes);
    const q = sel && this.toBox(sel);
    this.selectionBox.hidden = !q;
    if (q) Object.assign(this.selectionBox.style, { left: `${q.left}px`, top: `${q.top}px`, width: `${q.width}px`, height: `${q.height}px` });
    // the bars last: they keep clear of the cursor where it now is
    this.placeDiagram();
  }

  /** Tells screen readers the formula, a moment after typing stops. */
  private announce(): void {
    clearTimeout(this.liveTimer);
    this.liveTimer = window.setTimeout(() => {
      if (this.matches(":focus-within")) this.live.textContent = this.plain?.speech ?? "";
    }, 700);
  }

  /** Tells a command menu what is being typed after `\`, and where. */
  private sendCommand(active: boolean): void {
    if (!active && !this.commandShown) return;
    this.commandShown = active;
    const command = this.editor.commandBeingTyped();
    let rect: DOMRect | null = null;
    if (active && command) {
      const b = this.boxes.get(command.id);
      const p = b && this.toBox(b);
      const box = this.box.getBoundingClientRect();
      if (p) rect = new DOMRect(box.left + p.left - this.box.scrollLeft, box.top + p.top, p.width, p.height);
    }
    this.dispatchEvent(new CustomEvent<CommandDetail>("context-equation-command", { detail: { name: command?.name ?? "", active, rect }, bubbles: true, composed: true }));
  }

  // ---------- input ----------

  private pointerdown(e: PointerEvent): void {
    if (e.button !== 0) return;
    // a click outside the block being edited finishes it
    if (this.block) this.closeBlock(true);
    e.preventDefault();
    this.input.focus();
    const at = this.pointAt(e);
    if (!at || this.readOnly) return;
    const p = this.svgPoint(e);
    if (p) {
      // on an atom or a bond of a structure: picked; a drag from an atom draws a bond
      const hit = this.chemAt(p.x, p.y);
      if (hit) {
        this.editor.selectChem(hit.atom, hit.type, hit.index);
        if (hit.type === "atom") {
          const c = this.chemPoints(hit.atom)?.[hit.index];
          if (c) this.chemDrag = { atom: hit.atom, k: hit.index, x0: c.x, y0: c.y, x: p.x, y: p.y, active: false };
        }
        this.dragging = true;
        this.box.setPointerCapture?.(e.pointerId);
        return;
      }
      // on an arrow: its label to edit
      const arrow = arrowAt(this.editor, this.boxes, p.x, p.y);
      const cell = cellAt(this.editor, this.boxes, p.x, p.y);
      // an arrow over an empty cell (one it passes, as A → C over an empty middle) is what is seen there
      const emptyCell = cell && !cell.diagram.cells[cell.i][cell.j].children.length;
      if (arrow && (!cell || emptyCell)) {
        this.editor.selectArrow(arrow.diagram, arrow.index);
        return;
      }
      // in a cell: maybe the start of an arrow, once the pointer leaves it
      if (cell) this.arrowDrag = { diagram: cell.diagram, from: [cell.i, cell.j], to: null, x: p.x, y: p.y, active: false };
    }
    this.editor.setCursor(at, e.shiftKey);
    this.dragging = true;
    this.box.setPointerCapture?.(e.pointerId);
  }

  private pointermove(e: PointerEvent): void {
    if (!e.buttons) this.hoverBlock(e);
    if (!this.dragging || !(e.buttons & 1)) return;
    const chem = this.chemDrag;
    if (chem) {
      const p = this.svgPoint(e);
      if (!p) return;
      chem.x = p.x;
      chem.y = p.y;
      if (Math.hypot(p.x - chem.x0, p.y - chem.y0) > 450) chem.active = true;
      if (chem.active) this.placeDiagram();
      return;
    }
    const drag = this.arrowDrag;
    const p = this.svgPoint(e);
    // a drag that starts in a diagram's cell draws an arrow, never a selection
    if (drag && !p) return;
    if (drag && p) {
      const over = cellAt(this.editor, this.boxes, p.x, p.y);
      const away = !over || over.diagram !== drag.diagram || over.i !== drag.from[0] || over.j !== drag.from[1];
      if (away) drag.active = true;
      if (drag.active) {
        drag.x = p.x;
        drag.y = p.y;
        drag.to = over && over.diagram === drag.diagram && away ? [over.i, over.j] : null;
        drag.out = drag.to ? null : this.dragOut(drag);
        this.box.classList.add("dragging-arrow");
        this.placeDiagram();
      }
      return;
    }
    const at = this.pointAt(e);
    if (at) this.editor.setCursor(at, true);
  }

  private pointAt(e: PointerEvent) {
    const svg = this.drawing.querySelector("svg");
    if (!svg) return null;
    const vb = svg.viewBox.baseVal;
    const b = svg.getBoundingClientRect();
    if (!b.width) return null;
    const k = b.width / vb.width;
    return cursorAt(this.editor, this.boxes, vb.x + (e.clientX - b.left) / k, vb.y + (e.clientY - b.top) / k);
  }

  /** Double click: all of the row the cursor is in. */
  private selectRow(): void {
    const { row } = this.editor.cursor;
    if (row.children.length && !this.readOnly) this.editor.select(row, 0, row.children.length);
  }

  /** A pointer's position in the SVG's coordinates. */
  private svgPoint(e: MouseEvent): { x: number; y: number } | null {
    const svg = this.drawing.querySelector("svg");
    if (!svg) return null;
    const vb = svg.viewBox.baseVal;
    const b = svg.getBoundingClientRect();
    if (!b.width) return null;
    const k = b.width / vb.width;
    return { x: vb.x + (e.clientX - b.left) / k, y: vb.y + (e.clientY - b.top) / k };
  }

  /** The diagram the cursor is in, if any. */
  private diagramAtCursor(): Diagram | null {
    for (let row = this.editor.cursor.row; row.parent; row = row.parent.parent!) if (row.parent.kind === "diagram") return row.parent;
    return null;
  }

  /** Where an arrow dragged past the cells goes: the neighbour of its cell one step that way (in
   * eight directions), when the pointer is far enough from the cell; a new cell there if it is empty. */
  private dragOut(drag: { diagram: Diagram; from: [number, number]; x: number; y: number }): [number, number] | null {
    const r = cellRect(drag.diagram, drag.from[0], drag.from[1], this.boxes);
    if (!r) return null;
    const dx = drag.x - (r.x + r.width / 2);
    const dy = drag.y - (r.y + r.height / 2);
    // about one and a half cells' height away: a short drag draws nothing
    if (Math.hypot(dx, dy) < 1500) return null;
    const a = Math.round(Math.atan2(dy, dx) / (Math.PI / 4));
    const steps: [number, number][] = [[0, 1], [1, 1], [1, 0], [1, -1], [0, -1], [-1, -1], [-1, 0], [-1, 1]];
    return steps[(a + 8) % 8];
  }

  /** Over the diagram the cursor is in (or an arrow is drawn in): its cells outlined, the arrow being
   * edited highlighted, the arrow being drawn; and its bar of tools. */
  private placeDiagram(): void {
    const svg = this.drawing.querySelector("svg");
    const drag = this.arrowDrag?.active ? this.arrowDrag : null;
    const d = drag?.diagram ?? this.diagramAtCursor();
    const chemSel = svg && !this.readOnly ? this.editor.chemSelection() : null;
    this.box.classList.toggle("chem", !!chemSel);
    if (chemSel && svg) return this.placeStructure(svg, chemSel);
    if (!svg || !d || this.readOnly) {
      this.dgov.innerHTML = "";
      const rx = svg && !this.readOnly ? this.editor.reactionAtCursor() : null;
      if (rx && svg) {
        // each piece of the reaction outlined, the one the cursor is on marked
        const vb = svg.viewBox.baseVal;
        const sr = svg.getBoundingClientRect();
        const b = this.box.getBoundingClientRect();
        this.dgov.setAttribute("viewBox", `${vb.x} ${vb.y} ${vb.width} ${vb.height}`);
        Object.assign(this.dgov.style, { left: `${sr.left - b.left + this.box.scrollLeft}px`, top: `${sr.top - b.top}px`, width: `${sr.width}px`, height: `${sr.height}px` });
        const pad = 50;
        this.dgov.innerHTML = rx.reaction.body.children
          .filter((p) => p.kind !== "rxtoken")
          .map((p) => {
            const r = atomRect(p, this.boxes);
            if (!r) return "";
            const cls = p === rx.piece ? "cell here" : "cell";
            return `<rect class="${cls}" x="${r.x - pad}" y="${r.y - pad}" width="${r.width + 2 * pad}" height="${r.height + 2 * pad}" rx="50"/>`;
          })
          .join("");
        return this.paintReactionBar(rx.reaction);
      }
      // beside a block of LaTeX (what the editor keeps as written): a bar that says it is edited as LaTeX
      const block = svg && !this.readOnly && !this.block ? this.editor.blockAtCursor() : null;
      if (block) return this.paintBlockBar(block);
      this.dbar.hidden = true;
      this.dbarKey = "";
      return;
    }
    // the overlay sits exactly on the drawing, with its viewBox
    const vb = svg.viewBox.baseVal;
    const s = svg.getBoundingClientRect();
    const b = this.box.getBoundingClientRect();
    this.dgov.setAttribute("viewBox", `${vb.x} ${vb.y} ${vb.width} ${vb.height}`);
    Object.assign(this.dgov.style, { left: `${s.left - b.left + this.box.scrollLeft}px`, top: `${s.top - b.top}px`, width: `${s.width}px`, height: `${s.height}px` });
    const here = this.editor.matrixAtCursor();
    const current = this.editor.arrowAtCursor();
    let out = "";
    d.cells.forEach((line, i) =>
      line.forEach((_, j) => {
        const r = cellRect(d, i, j, this.boxes);
        if (!r) return;
        const pad = 60;
        const cls = drag?.to && drag.to[0] === i && drag.to[1] === j ? "cell target" : here?.matrix === d && here.i === i && here.j === j ? "cell here" : "cell";
        out += `<rect class="${cls}" x="${r.x - pad}" y="${r.y - pad}" width="${r.width + 2 * pad}" height="${r.height + 2 * pad}" rx="50"/>`;
      }),
    );
    const path = (pts: { x: number; y: number }[]) => `M${pts.map((p) => `${p.x} ${p.y}`).join(" L")}`;
    if (current?.diagram === d) {
      const pts = arrowPath(d, current.arrow, this.boxes);
      if (pts) out += `<path class="arrow" d="${path(pts)}"/>`;
    }
    if (drag) {
      const r = cellRect(d, drag.from[0], drag.from[1], this.boxes);
      if (r) out += `<line class="ghost" x1="${r.x + r.width / 2}" y1="${r.y + r.height / 2}" x2="${drag.x}" y2="${drag.y}"/>`;
      // past the cells: the new cell, where tikz-cd's spacing puts it
      if (r && drag.out) out += `<rect class="cell target" x="${r.x + drag.out[1] * 2400 - 60}" y="${r.y + drag.out[0] * 1911 - 60}" width="${r.width + 120}" height="${r.height + 120}" rx="50"/>`;
      // the bar stays out of the way while an arrow is drawn
      this.dgov.innerHTML = out;
      this.dbar.hidden = true;
      this.dbarKey = "";
      return;
    }
    this.dgov.innerHTML = out;
    this.paintDiagramBar(d, !!current, current?.arrow.style ?? null, current?.arrow.bend ?? null);
  }

  private paintDiagramBar(d: Diagram, onArrow: boolean, style: ArrowStyle | null, bend: "left" | "right" | null): void {
    const l = this.locale;
    const key = `${l}|${onArrow}|${style}|${bend}`;
    if (key !== this.dbarKey) {
      this.dbarKey = key;
      const btn = (run: string, label: string, title: string, pressed?: boolean, text = false) =>
        `<button type="button" data-run="${run}" title="${title}" aria-label="${title}"${pressed === undefined ? "" : ` aria-pressed="${pressed}"`}${text ? ' class="text"' : ""}>${label}</button>`;
      const w = (k: keyof typeof UI) => say(UI[k], l);
      if (onArrow) {
        const styles: [ArrowStyle, string, keyof typeof UI][] = [["plain", "→", "stylePlain"], ["dashed", "⇢", "styleDashed"], ["dotted", "⋯", "styleDotted"], ["hook", "↪", "styleHook"], ["twohead", "↠", "styleTwohead"], ["double", "⇒", "styleDouble"], ["equal", "=", "styleEqual"], ["mapsto", "↦", "styleMapsto"], ["squiggly", "⇝", "styleSquiggly"]];
        this.dbar.innerHTML =
          styles.map(([s, glyph, k]) => btn(`arrowStyle:${s}`, glyph, w(k), style === s)).join("") +
          '<span class="sep"></span>' +
          btn("arrowBend:none", "—", w("bendNone"), bend === null) +
          btn("arrowBend:left", "↶", w("bendLeft"), bend === "left") +
          btn("arrowBend:right", "↷", w("bendRight"), bend === "right") +
          '<span class="sep"></span>' +
          btn("arrowSwap", "⇅", w("swapLabel")) +
          btn("arrowRemove", icon("clear"), w("removeArrow")) +
          `<span class="hint">${w("arrowHint")}</span>`;
      } else {
        this.dbar.innerHTML =
          btn("addRowAbove", icon("rowAdd"), w("rowAbove")) +
          btn("addRow", icon("rowAdd"), w("rowBelow")) +
          btn("addColumnLeft", icon("columnAdd"), w("colLeft")) +
          btn("addColumn", icon("columnAdd"), w("colRight")) +
          btn("removeRow", icon("rowRemove"), w("rowRemove")) +
          btn("removeColumn", icon("columnRemove"), w("colRemove")) +
          '<span class="sep"></span>' +
          btn("editAsLatex", w("editLatex"), w("editLatex"), undefined, true) +
          `<span class="hint">${w("cellHint")}</span>`;
      }
    }
    this.dbar.hidden = false;
    // under the diagram and its outlined cells (empty ones included), inside the field
    const cellRects = d.cells.flatMap((line, i) => line.map((_, j) => cellRect(d, i, j, this.boxes))).filter((x): x is Rect => !!x);
    const own = atomRect(d, this.boxes);
    const all = [...cellRects, ...(own ? [own] : [])];
    const r = all.length ? { x: Math.min(...all.map((q) => q.x)), y: Math.min(...all.map((q) => q.y)), width: Math.max(...all.map((q) => q.x + q.width)) - Math.min(...all.map((q) => q.x)), height: Math.max(...all.map((q) => q.y + q.height)) - Math.min(...all.map((q) => q.y)) + 120 } : null;
    const p = r && this.toBox(r);
    if (p) this.placeBar(p);
  }

  /** Where each atom of a structure is drawn (its label's middle; a vertex's point), by index. */
  private chemPoints(atom: Raw): { x: number; y: number; r: number }[] | null {
    const g = this.editor.graphOf(atom);
    if (!g) return null;
    const out: { x: number; y: number; r: number }[] = [];
    g.atoms.forEach((a, k) => {
      // a ring that closes on its first atom names it again: the box is the first one's
      const first = g.atoms.findIndex((b) => b.span && a.span && b.span[0] === a.span[0]);
      const b = this.boxes.get(`${atom.id}a${first >= 0 ? first : k}`);
      out.push(b ? { x: b.x + b.width / 2, y: b.y + b.height / 2, r: Math.max(b.width, b.height) / 2 } : { x: NaN, y: NaN, r: 0 });
    });
    return out;
  }

  /** The atom or bond of a structure at a point of the drawing. */
  private chemAt(x: number, y: number): { atom: Raw; type: "atom" | "bond"; index: number } | null {
    for (const row of this.editor.rows())
      for (const a of row.children) {
        if (a.kind !== "raw") continue;
        const g = this.editor.graphOf(a);
        const pts = g && this.chemPoints(a);
        if (!g || !pts) continue;
        let best: { type: "atom" | "bond"; index: number; d: number } | null = null;
        pts.forEach((p, k) => {
          const d = Math.hypot(x - p.x, y - p.y) - p.r;
          if (d < 220 && (!best || d < best.d)) best = { type: "atom", index: k, d };
        });
        if (!best)
          g.bonds.forEach((b, k) => {
            const p = pts[b.from];
            const q = pts[b.to];
            if (!p || !q || Number.isNaN(p.x + q.x)) return;
            const len2 = (q.x - p.x) ** 2 + (q.y - p.y) ** 2 || 1;
            const t = Math.max(0, Math.min(1, ((x - p.x) * (q.x - p.x) + (y - p.y) * (q.y - p.y)) / len2));
            const d = Math.hypot(x - (p.x + t * (q.x - p.x)), y - (p.y + t * (q.y - p.y)));
            if (d < 160 && (!best || d < best.d)) best = { type: "bond", index: k, d };
          });
        const found = best as { type: "atom" | "bond"; index: number; d: number } | null;
        if (found) return { atom: a, type: found.type, index: found.index };
      }
    return null;
  }

  /** A drag's direction as chemfig draws bonds: counterclockwise from the right, in steps of 30°. */
  private snapAngle(dx: number, dy: number): number {
    const a = (Math.atan2(-dy, dx) * 180) / Math.PI;
    return ((Math.round(a / 30) * 30) % 360 + 360) % 360;
  }

  /** Over the structure being edited: its atoms outlined, the atom or bond picked, a bond being dragged; its bar. */
  private placeStructure(svg: SVGSVGElement, s: ChemSelection): void {
    const vb = svg.viewBox.baseVal;
    const sr = svg.getBoundingClientRect();
    const b = this.box.getBoundingClientRect();
    this.dgov.setAttribute("viewBox", `${vb.x} ${vb.y} ${vb.width} ${vb.height}`);
    Object.assign(this.dgov.style, { left: `${sr.left - b.left + this.box.scrollLeft}px`, top: `${sr.top - b.top}px`, width: `${sr.width}px`, height: `${sr.height}px` });
    const pts = this.chemPoints(s.atom) ?? [];
    let out = "";
    pts.forEach((p, k) => {
      if (Number.isNaN(p.x)) return;
      const here = s.type === "atom" && s.index === k;
      out += `<circle class="atom${here ? " here" : ""}" cx="${p.x}" cy="${p.y}" r="${p.r + 110}"/>`;
    });
    if (s.type === "bond") {
      const bd = s.graph.bonds[s.index];
      const p = pts[bd.from];
      const q = pts[bd.to];
      if (p && q) out += `<line class="arrow" x1="${p.x}" y1="${p.y}" x2="${q.x}" y2="${q.y}"/>`;
    }
    const drag = this.chemDrag?.active ? this.chemDrag : null;
    if (drag) {
      const a = (this.snapAngle(drag.x - drag.x0, drag.y - drag.y0) * Math.PI) / 180;
      const len = 1300;
      out += `<line class="ghost" x1="${drag.x0}" y1="${drag.y0}" x2="${drag.x0 + Math.cos(a) * len}" y2="${drag.y0 - Math.sin(a) * len}"/>`;
    }
    this.dgov.innerHTML = out;
    // the bar
    const l = this.locale;
    const label = s.type === "atom" ? s.graph.atoms[s.index].label : "";
    const kind = s.type === "bond" ? s.graph.bonds[s.index].kind : this.chemKind;
    // the label is left out of the key: the bar is not made again while it is typed in its box
    const key = `ch|${l}|${s.type}|${kind}`;
    if (key !== this.dbarKey) {
      this.dbarKey = key;
      const btn = (run: string, text: string, title: string, pressed?: boolean, cls = "") =>
        `<button type="button" data-run="${run}" title="${title}" aria-label="${title}"${pressed === undefined ? "" : ` aria-pressed="${pressed}"`}${cls ? ` class="${cls}"` : ""}>${text}</button>`;
      const w = (k: keyof typeof UI) => say(UI[k], l);
      const kinds: [string, string, keyof typeof UI][] = [["single", "—", "chSingle"], ["double", "=", "chDouble"], ["triple", "≡", "chTriple"], ["wedge", "◀", "chWedge"], ["hash", "⋯", "chHash"]];
      let html = "";
      if (s.type === "atom") {
        html += `<div class="line"><input class="label" type="text" title="${w("chLabel")}" aria-label="${w("chLabel")}" placeholder="${w("chLabelPlaceholder")}" autocomplete="off" autocapitalize="off" spellcheck="false">`;
        html += btn("chemLabel:", "·", w("chVertex"), false) + ["C", "H", "N", "O", "S", "P", "F", "Cl", "Br", "I"].map((el) => btn(`chemLabel:${el}`, el, `${w("chElement")} ${el}`, false)).join("");
        html += '</div><div class="line">' + kinds.slice(0, 3).map(([k2, g, t]) => btn(`chemKind:${k2}`, g, t ? w(t) : k2, kind === k2)).join("");
        const dirs: [number, string][] = [[0, "→"], [30, "↗"], [90, "↑"], [150, "↖"], [180, "←"], [210, "↙"], [270, "↓"], [330, "↘"]];
        html += '<span class="sep"></span>' + dirs.map(([a, g]) => btn(`chemBond:${a}`, g, `${w("chBondTo")} ${a}°`)).join("");
        html += '</div><div class="line">' + btn("chemRing:6a", "⌬", w("chBenzene")) + btn("chemRing:6", "⬡", w("chHexagon")) + btn("chemRing:5", "⬠", w("chPentagon"));
      } else html += kinds.map(([k2, g, t]) => btn(`chemKind:${k2}`, g, w(t), kind === k2)).join("");
      html += btn("chemRemove", icon("clear"), w("chRemove"));
      html += '<span class="sep"></span>' + btn("editAsLatex", w("editLatex"), w("editLatex"), undefined, "text");
      // an atom's bar is in lines (label, bonds, rings and the rest), its hint the last one
      html += s.type === "atom" ? `</div><div class="line"><span class="hint">${w("chAtomHint")}</span></div>` : `<span class="hint">${w("chBondHint")}</span>`;
      this.dbar.innerHTML = html;
    }
    // the label in its box (unless it is being typed there) and the element it is, pressed
    const input = this.dbar.querySelector<HTMLInputElement>("input.label");
    // as it is typed: NH3 for chemfig's NH_3
    if (input && this.shadowRoot!.activeElement !== input) input.value = labelText(label);
    for (const b of this.dbar.querySelectorAll<HTMLElement>('button[data-run^="chemLabel:"]')) b.setAttribute("aria-pressed", String(b.dataset.run === `chemLabel:${labelText(label)}`));
    this.dbar.hidden = false;
    const r = atomRect(s.atom, this.boxes);
    const p = r && this.toBox({ ...r, height: r.height + 200 });
    if (p) this.placeBar(p);
  }

  /** The bar beside a block of LaTeX: open it to edit as written. */
  private paintBlockBar(block: Raw): void {
    const l = this.locale;
    const key = `block|${l}`;
    if (key !== this.dbarKey) {
      this.dbarKey = key;
      const w = (k: keyof typeof UI) => say(UI[k], l);
      this.dbar.innerHTML = `<button type="button" data-run="editAsLatex" class="text" title="${w("editLatex")}" aria-label="${w("editLatex")}">${w("editLatex")}</button><span class="hint">${w("blockBarHint")}</span>`;
    }
    this.dbar.hidden = false;
    const r = atomRect(block, this.boxes);
    const p = r && this.toBox({ ...r, height: r.height + 60 });
    if (p) this.placeBar(p);
  }

  /** The bar of a reaction: the arrow's kind and conditions, signs to add. */
  private paintReactionBar(reaction: Reaction): void {
    const l = this.locale;
    const at = this.editor.reactionAtCursor();
    const piece = at?.piece ?? null;
    const arrow = this.editor.activeArrow()?.arrow ?? null;
    const scheme = !!reaction.scheme;
    const key = `rx|${l}|${scheme}|${piece?.kind ?? ""}|${arrow}`;
    if (key !== this.dbarKey) {
      this.dbarKey = key;
      const btn = (run: string, label: string, title: string, pressed?: boolean, text = false) =>
        `<button type="button" data-run="${run}" title="${title}" aria-label="${title}"${pressed === undefined ? "" : ` aria-pressed="${pressed}"`}${text ? ' class="text"' : ""}>${label}</button>`;
      const w = (k: keyof typeof UI) => say(UI[k], l);
      const arrows: [string, string, keyof typeof UI][] = scheme
        ? [["->", "→", "rxForward"], ["<=>", "⇌", "rxEquilibrium"], ["<->", "↔", "rxResonance"], ["<-", "←", "rxBackward"], ["-/>", "↛", "rxNoReaction"]]
        : [["->", "→", "rxForward"], ["<=>", "⇌", "rxEquilibrium"], ["<=>>", "⥂", "rxEquilibriumRight"], ["<<=>", "⥄", "rxEquilibriumLeft"], ["<->", "↔", "rxResonance"], ["<-", "←", "rxBackward"]];
      let html = arrows.map(([a, glyph, k]) => btn(`rxArrow:${a}`, glyph, w(k), arrow === null ? undefined : arrow === a)).join("");
      if (this.editor.activeArrow()) html += '<span class="sep"></span>' + btn("rxPlace:above", "▔", w("rxAbove"), undefined) + btn("rxPlace:below", "▁", w("rxBelow"), undefined);
      html += '<span class="sep"></span>' + (scheme ? btn("rxStructure", "⌬", w("rxStructure")) + btn("rxSign:+", "+", w("rxPlus")) : btn("rxSign:+", "+", w("rxPlus")) + btn("rxSign:^", "↑", w("rxGas")) + btn("rxSign:v", "↓", w("rxPrecipitate")));
      const current = piece ?? this.editor.cursor.row.children[this.editor.cursor.offset - 1];
      if (current) html += btn("rxRemove", icon("clear"), w("rxRemove"));
      html += '<span class="sep"></span>' + btn("editAsLatex", w("editLatex"), w("editLatex"), undefined, true) + `<span class="hint">${w(scheme ? "rxSchemeHint" : "rxHint")}</span>`;
      this.dbar.innerHTML = html;
    }
    this.dbar.hidden = false;
    const r = atomRect(reaction, this.boxes);
    const p = r && this.toBox({ ...r, height: r.height + 120 });
    if (p) this.placeBar(p);
  }

  /** The block of LaTeX under the pointer. */
  private blockUnder(e: MouseEvent): Raw | null {
    const svg = this.drawing.querySelector("svg");
    if (!svg) return null;
    const vb = svg.viewBox.baseVal;
    const b = svg.getBoundingClientRect();
    if (!b.width) return null;
    const k = b.width / vb.width;
    return blockAt(this.editor, this.boxes, vb.x + (e.clientX - b.left) / k, vb.y + (e.clientY - b.top) / k) as Raw | null;
  }

  /** Outlines the block under the pointer, with a tooltip that says it can be edited. */
  private hoverBlock(e: PointerEvent): void {
    const under = this.readOnly || this.block ? null : this.blockUnder(e);
    // a structure is edited atom by atom, not as a block
    const b = under && !this.editor.graphOf(under) ? under : null;
    const r = b && atomRect(b, this.boxes);
    const p = r && this.toBox(r);
    this.hoverBox.hidden = !p;
    this.box.classList.toggle("over-block", !!p);
    this.box.title = p ? say(UI.blockEdit, this.locale) : "";
    if (p) Object.assign(this.hoverBox.style, { left: `${p.left - 3}px`, top: `${p.top - 3}px`, width: `${p.width + 6}px`, height: `${p.height + 6}px` });
  }

  /** Opens the editor of a block's LaTeX under the block. */
  openBlock(atom: Raw, caret?: number): void {
    if (this.block) this.closeBlock(true);
    this.block = this.editor.editBlock(atom, caret);
    const l = this.locale;
    this.blockPanel.setAttribute("aria-label", say(UI.blockTitle, l));
    this.blockPanel.querySelector(".title b")!.textContent = say(UI.blockTitle, l);
    this.blockPanel.querySelector(".hint")!.textContent = say(UI.blockHint, l);
    this.blockPanel.querySelector('[data-block="done"]')!.textContent = say(UI.blockDone, l);
    this.blockPanel.querySelector('[data-block="cancel"]')!.textContent = say(UI.blockCancel, l);
    this.blockSource.setAttribute("aria-label", say(UI.blockTitle, l));
    this.blockSource.value = atom.latex;
    this.blockSource.rows = Math.min(8, Math.max(3, atom.latex.split("\n").length + 1));
    this.blockPanel.hidden = false;
    this.hoverBox.hidden = true;
    this.placeBlock();
    this.blockStatus();
    this.blockSource.focus();
    const c = caret ?? atom.latex.length;
    this.blockSource.setSelectionRange(c, c);
  }

  private blockInput(): void {
    this.block?.update(this.blockSource.value);
    this.blockStatus();
  }

  /** What contextEquation says of the block's LaTeX, as it is typed. */
  private blockStatus(): void {
    const status = this.blockPanel.querySelector(".status")!;
    const latex = this.blockSource.value;
    let warnings: string[] = [];
    try {
      warnings = latex.trim() ? render(latex, this.drawingOptions).warnings.map((w) => w.description) : [];
    } catch (e) {
      warnings = [e instanceof Error ? e.message : String(e)];
    }
    status.classList.toggle("warn", warnings.length > 0);
    status.textContent = warnings.length ? warnings[0] : latex.trim() ? say(UI.blockOk, this.locale) : "";
  }

  /** Keeps the editor of the block under the block (or above it, near the bottom). */
  private placeBlock(): void {
    if (!this.block) return;
    const r = atomRect(this.block.atom, this.boxes);
    const p = r && this.toBox(r);
    const host = this.getBoundingClientRect();
    const box = this.box.getBoundingClientRect();
    const width = Math.min(560, host.width - 16);
    let left = p ? box.left - host.left + p.left - this.box.scrollLeft : 8;
    left = Math.max(8, Math.min(left, host.width - width - 8));
    const below = p ? box.top - host.top + p.top + p.height + 10 : box.bottom - host.top + 8;
    const panel = this.blockPanel.offsetHeight || 200;
    const top = below + panel > Math.max(host.height, window.innerHeight - host.top) && p && p.top - panel - 10 > 0 ? box.top - host.top + p.top - panel - 10 : below;
    Object.assign(this.blockPanel.style, { left: `${left}px`, top: `${top}px` });
  }

  private closeBlock(keep: boolean): void {
    const s = this.block;
    if (!s) return;
    this.block = null;
    this.blockPanel.hidden = true;
    if (keep) s.commit();
    else s.cancel();
    this.input.focus();
  }

  private keydown(e: KeyboardEvent): void {
    // a command menu, when open, takes the keys it needs first
    const ask = new CustomEvent("context-equation-key", { detail: { key: e.key, shift: e.shiftKey }, cancelable: true, bubbles: true, composed: true });
    if (!this.dispatchEvent(ask)) {
      e.preventDefault();
      return;
    }
    if (e.isComposing) return;
    let action = { ...DEFAULT_KEYMAP, ...this.keymap }[comboOf(e)];
    if (action === "") return;
    // Enter beside a block of LaTeX (out of a command and a matrix) opens it
    if (action === "enter" && !this.editor.commandBeingTyped() && !this.editor.matrixAtCursor() && this.editor.blockAtCursor()) action = "editBlock";
    if (action) {
      // Tab leaves an empty field, so a form can be crossed with it
      if ((action === "tab" || action === "tabBack") && this.editor.isEmpty()) return;
      e.preventDefault();
      this.run(action);
      return;
    }
    if (e.key === "Escape") {
      // leave a command being typed, a structure, else the selection
      if (this.editor.chemSelection()) this.editor.clearChem();
      else if (this.editor.commandBeingTyped()) this.editor.completeCommand("");
      else if (this.editor.anchor) this.editor.setCursor(this.editor.cursor);
      // out of an arrow's label, back to the cell it leaves
      else if (this.editor.arrowAtCursor()) this.editor.leaveArrowLabel();
      // out of a reaction, after it
      else if (this.editor.reactionAtCursor()) this.editor.leaveReaction();
    }
    // printable characters arrive through `input`, which also covers phones and dead keys
  }

  private typed(): void {
    const text = this.input.value;
    this.input.value = "";
    if (!this.readOnly && text) this.editor.typeText(text);
  }

  private clipboard(e: ClipboardEvent, kind: "copy" | "cut"): void {
    const latex = this.editor.selection() ? this.editor.selectedLatex() : this.latex;
    if (!latex) return;
    e.preventDefault();
    e.clipboardData?.setData("text/plain", latex);
    if (kind === "cut" && !this.readOnly && this.editor.selection()) this.editor.cut();
  }

  private pasted(e: ClipboardEvent): void {
    e.preventDefault();
    const text = e.clipboardData?.getData("text/plain") ?? "";
    if (text && !this.readOnly) this.editor.paste(text);
  }
}

if (typeof customElements !== "undefined" && !customElements.get("context-equation-field")) customElements.define("context-equation-field", ContextEquationField);
