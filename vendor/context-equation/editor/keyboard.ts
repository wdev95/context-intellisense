// <context-equation-keyboard>: a keyboard on the screen, for phones and tablets or for anyone who prefers it.
// Layers of keys (numbers, letters, functions and symbols), each key a character, a snippet or an action.

import { FieldClient, TOKENS, html, icon, keepFocus, say, UI, type Words } from "./shared.ts";
import { BUTTON_STYLE, face, contextEquationReady } from "./palette.ts";
import type { PaletteItem } from "./sets.ts";

export interface Key extends PaletteItem {
  /** Characters typed, as on a physical keyboard. */
  type?: string;
  /** Goes to another layer (its id). */
  layer?: string;
  /** How many columns the key takes. */
  span?: number;
}

export interface KeyboardLayer {
  id: string;
  /** The tab's name. */
  title: Words;
  /** Lines of keys. */
  rows: Key[][];
}

const t = (en: string, pt: string): Words => ({ en, "pt-BR": pt });
const ch = (c: string, en?: string, pt?: string): Key => ({ type: c, label: c, title: en ? t(en, pt ?? en) : c });
const snip = (insert: string, en: string, pt: string, preview?: string): Key => ({ insert, preview, title: t(en, pt) });
const act = (action: string, iconName: string, words: Words, span = 1): Key => ({ action, icon: iconName, title: words, span });

const NAV: Key[] = [act("moveLeft", "left", UI.left), act("moveRight", "right", UI.right), act("backspace", "backspace", UI.backspace, 2)];

export const NUMBERS: KeyboardLayer = {
  id: "numbers",
  title: t("123", "123"),
  rows: [
    [ch("7"), ch("8"), ch("9"), ch("+", "Plus", "Mais"), snip("\\frac{#0}{#?}", "Fraction", "Fração"), { action: "sup", preview: "#?^{#?}", title: t("Exponent", "Expoente") }, ch("x"), ch("y")],
    [ch("4"), ch("5"), ch("6"), ch("-", "Minus", "Menos"), snip("\\sqrt{#0}", "Square root", "Raiz quadrada"), { action: "sub", preview: "#?_{#?}", title: t("Index", "Índice") }, ch("a"), ch("b")],
    [ch("1"), ch("2"), ch("3"), snip("\\times", "Times", "Vezes"), snip("\\div", "Divided by", "Dividido por"), ch("(", "Open parenthesis", "Abre parênteses"), ch(")", "Close parenthesis", "Fecha parênteses"), snip("\\pi", "Pi", "Pi")],
    [ch("0"), ch(".", "Point", "Ponto"), ch(",", "Comma", "Vírgula"), ch("=", "Equals", "Igual"), snip("\\pm", "Plus or minus", "Mais ou menos"), ch("<", "Less than", "Menor que"), ch(">", "Greater than", "Maior que"), snip("\\infty", "Infinity", "Infinito")],
    [{ layer: "letters", label: "abc", title: t("Letters", "Letras") }, { layer: "functions", label: "f(x)", title: t("Functions and symbols", "Funções e símbolos") }, act("tab", "tab", UI.tab), act("moveUp", "up", UI.up), act("moveDown", "down", UI.down), ...NAV],
  ],
};

const letters = (s: string) => [...s].map((c) => ch(c));

export const LETTERS: KeyboardLayer = {
  id: "letters",
  title: t("abc", "abc"),
  rows: [
    letters("qwertyuiop"),
    letters("asdfghjkl"),
    [{ action: "shift", icon: "shift", title: UI.shift }, ...letters("zxcvbnm"), act("backspace", "backspace", UI.backspace, 2)],
    [{ layer: "numbers", label: "123", title: t("Numbers", "Números") }, { layer: "functions", label: "f(x)", title: t("Functions and symbols", "Funções e símbolos") }, { type: " ", label: "␣", title: t("Space", "Espaço"), span: 4 }, act("moveLeft", "left", UI.left), act("moveRight", "right", UI.right), act("enter", "enter", UI.enter)],
  ],
};

export const FUNCTIONS: KeyboardLayer = {
  id: "functions",
  title: t("f(x)", "f(x)"),
  rows: [
    [snip("\\sin", "Sine", "Seno"), snip("\\cos", "Cosine", "Cosseno"), snip("\\tan", "Tangent", "Tangente"), snip("\\log", "Logarithm", "Logaritmo"), snip("\\ln", "Natural logarithm", "Logaritmo natural"), snip("e^{#?}", "e to the", "e elevado a"), snip("\\left|#0\\right|", "Absolute value", "Módulo", "\\left|#?\\right|"), snip("\\sqrt[#?]{#0}", "n-th root", "Raiz de índice n")],
    [snip("\\alpha", "alpha", "alfa"), snip("\\beta", "beta", "beta"), snip("\\theta", "theta", "teta"), snip("\\lambda", "lambda", "lambda"), snip("\\mu", "mu", "mi"), snip("\\sigma", "sigma", "sigma"), snip("\\Delta", "Delta", "Delta"), snip("\\omega", "omega", "ômega")],
    [snip("\\le", "Less or equal", "Menor ou igual"), snip("\\ge", "Greater or equal", "Maior ou igual"), snip("\\ne", "Not equal", "Diferente"), snip("\\approx", "Approximately", "Aproximadamente"), snip("\\in", "Belongs to", "Pertence"), snip("\\to", "Tends to", "Tende a"), snip("\\cdot", "Dot", "Ponto"), snip("^{\\circ}", "Degree", "Grau")],
    [snip("\\sum_{#?}^{#?}#0", "Sum", "Somatório", "\\sum_{#?}^{#?}"), snip("\\int_{#?}^{#?}#0", "Integral", "Integral", "\\int_{#?}^{#?}"), snip("\\lim_{#?}#0", "Limit", "Limite", "\\lim_{#?}"), snip("\\left[#0\\right]", "Brackets", "Colchetes", "\\left[#?\\right]"), snip("\\left\\{#0\\right\\}", "Braces", "Chaves", "\\left\\{#?\\right\\}"), snip("\\startpmatrix#?\\NC#?\\NR#?\\NC#?\\stoppmatrix", "Matrix", "Matriz"), snip("\\vec{#0}", "Vector", "Vetor"), snip("\\text{#0}", "Text", "Texto", "\\text{abc}")],
    [{ layer: "numbers", label: "123", title: t("Numbers", "Números") }, { layer: "letters", label: "abc", title: t("Letters", "Letras") }, act("undo", "undo", UI.undo), act("redo", "redo", UI.redo), ...NAV],
  ],
};

const STYLE = `
${TOKENS}
:host { display: block; color: var(--_ink); background: var(--_surface); border: 1px solid var(--_border); border-radius: var(--_radius); padding: 6px; user-select: none; -webkit-user-select: none; }
:host([hidden]) { display: none; }
/* the keys' width follows the keyboard's: the words on them (abc, f(x), 123) with it, down to 10px in a
   narrow sidebar, 15px from a phone's width up */
.keys { display: grid; gap: 5px; container-type: inline-size; }
.line { display: grid; grid-template-columns: repeat(var(--columns), minmax(0, 1fr)); gap: 5px; }
.keys button { min-height: var(--context-equation-key-height, 42px); font: 17px Georgia, "Times New Roman", serif; touch-action: manipulation; }
.keys button[data-layer], .keys button[data-action] { background: var(--_surface); }
.keys button.on { border-color: var(--_accent); color: var(--_accent); }
.keys button .label { font-size: clamp(10px, 4.3cqi, 15px); white-space: nowrap; }
.keys button:has(> .label) { padding-inline: clamp(1px, 1cqi, 4px); }
${BUTTON_STYLE}
`;

/** `layers="numbers letters"` picks layers that come with the editor; the `layers` property takes your own. */
export class ContextEquationKeyboard extends FieldClient {
  static observedAttributes = ["locale", "layer", "layers"];
  private _layers: KeyboardLayer[] | null = null;
  private current = "";
  private shift = false;
  private keys: HTMLDivElement;

  constructor() {
    super();
    const root = this.attachShadow({ mode: "open" });
    root.innerHTML = `<style>${STYLE}</style><div class="keys" part="keys" role="group"></div>`;
    this.keys = root.querySelector(".keys")!;
    keepFocus(this.keys);
    this.keys.addEventListener("click", (e) => {
      const b = (e.target as HTMLElement).closest<HTMLButtonElement>("button");
      if (!b) return;
      const [i, j] = (b.dataset.k ?? "0,0").split(",").map(Number);
      this.press(this.layer.rows[i][j]);
    });
  }

  get layers(): KeyboardLayer[] {
    if (this._layers) return this._layers;
    const all = [NUMBERS, LETTERS, FUNCTIONS];
    const names = this.getAttribute("layers")?.split(/[\s,]+/).filter(Boolean);
    return names ? names.map((n) => all.find((l) => l.id === n)).filter((l): l is KeyboardLayer => !!l) : all;
  }

  set layers(layers: KeyboardLayer[]) {
    this._layers = layers;
    this.current = "";
    this.paint();
  }

  private get layer(): KeyboardLayer {
    return this.layers.find((l) => l.id === (this.current || this.getAttribute("layer"))) ?? this.layers[0];
  }

  override connectedCallback(): void {
    super.connectedCallback();
    contextEquationReady().then(() => this.paint());
  }

  attributeChangedCallback(): void {
    contextEquationReady().then(() => this.paint());
  }

  private press(key: Key): void {
    const field = this.field;
    if (key.layer) {
      this.current = key.layer;
      this.paint();
      return;
    }
    if (key.action === "shift") {
      this.shift = !this.shift;
      this.paint();
      return;
    }
    if (!field) return;
    if (key.type !== undefined) {
      field.run(`type:${this.shift && key.type.length === 1 ? key.type.toUpperCase() : key.type}`);
      if (this.shift) {
        this.shift = false;
        this.paint();
      }
    } else if (key.action) field.run(key.action);
    else if (key.insert !== undefined) field.run(`latex:${key.insert}`);
  }

  private paint(): void {
    const locale = this.locale;
    const layer = this.layer;
    this.keys.setAttribute("aria-label", `${say(UI.keyboard, locale)}: ${say(layer.title, locale)}`);
    this.keys.innerHTML = layer.rows
      .map((row, i) => {
        const columns = row.reduce((s, k) => s + (k.span ?? 1), 0);
        return `<div class="line" style="--columns:${columns}">${row
          .map((key, j) => {
            const name = html(say(key.title, locale));
            // a character key is drawn by contextEquation, as it will look in the formula (x in italics, a true minus)
            const typed = key.type !== undefined && key.type.trim().length === 1 ? (this.shift ? key.type.toUpperCase() : key.type) : null;
            const label = typed !== null ? { ...key, label: undefined, preview: typed === "\\" ? "\\backslash" : typed } : key;
            const extra = key.layer ? ` data-layer="${html(key.layer)}"` : key.action ? ` data-action="${html(key.action)}"` : "";
            const on = key.action === "shift" && this.shift ? ' class="on" aria-pressed="true"' : key.action === "shift" ? ' aria-pressed="false"' : "";
            return `<button type="button" part="key" data-k="${i},${j}"${extra}${on} title="${name}" aria-label="${name}" style="grid-column: span ${key.span ?? 1}">${key.icon ? icon(key.icon) : face(label)}</button>`;
          })
          .join("")}</div>`;
      })
      .join("");
  }
}

if (typeof customElements !== "undefined" && !customElements.get("context-equation-keyboard")) customElements.define("context-equation-keyboard", ContextEquationKeyboard);
