// A matrix of any size, picked on a grid as a table is in a word processor: the pointer (or the
// arrows) over the grid says how many lines and columns, a click (or Enter) inserts it. With the
// cursor in a matrix, the grid starts at its size and gives it the size picked.

import { html, say, type Words } from "./shared.ts";
import type { ContextEquationField } from "./field.ts";

const t = (en: string, pt: string): Words => ({ en, "pt-BR": pt });
const WORDS = {
  insert: t("Insert a matrix", "Inserir matriz"),
  resize: t("Size of this matrix", "Tamanho desta matriz"),
  diagram: t("Size of this diagram", "Tamanho deste diagrama"),
  grid: t("Lines and columns", "Linhas e colunas"),
  delimiters: t("Delimiters", "Delimitadores"),
  lost: t("cells with content will be removed", "células com conteúdo serão removidas"),
  lostArrows: t("cells with content or arrows will be removed", "células com conteúdo ou setas serão removidas"),
};

/** The delimiters a matrix can have, with what the button shows. */
const DELIMS: [env: string, face: string, title: Words][] = [
  ["pmatrix", "( )", t("Parentheses", "Parênteses")],
  ["bmatrix", "[ ]", t("Brackets", "Colchetes")],
  ["Bmatrix", "{ }", t("Braces", "Chaves")],
  ["vmatrix", "| |", t("Bars (determinant)", "Barras (determinante)")],
  ["Vmatrix", "‖ ‖", t("Double bars", "Barras duplas")],
  ["matrix", "—", t("None", "Nenhum")],
];

const MAX = 20;

const STYLE = `
.matrix-picker { position: fixed; inset: auto; margin: 0; padding: 10px; color: var(--_ink); background: var(--_background); border: 1px solid var(--_border); border-radius: calc(var(--_radius) + 2px); box-shadow: 0 8px 28px rgb(0 0 0 / 0.18); font: 13px var(--_font); }
.matrix-picker .title { font-weight: 600; margin: 0 2px 8px; }
.matrix-picker .delims { display: flex; gap: 4px; margin-bottom: 8px; }
.matrix-picker .delims[hidden] { display: none; }
.matrix-picker .delims button { min-width: 34px; height: 28px; font: 14px var(--_font); color: var(--_ink); background: var(--_key); border: 1px solid var(--_border); border-radius: var(--_radius); cursor: pointer; }
.matrix-picker .delims button[aria-pressed="true"] { border-color: var(--_accent); box-shadow: inset 0 0 0 1px var(--_accent); }
.matrix-picker .cells { display: grid; width: max-content; gap: 3px; outline: none; border-radius: 3px; touch-action: none; }
.matrix-picker .cells:focus-visible { outline: 2px solid var(--_focus); outline-offset: 3px; }
.matrix-picker .cell { width: 18px; height: 18px; border: 1px solid var(--_border); border-radius: 2px; background: var(--_surface); }
.matrix-picker .cell.on { background: color-mix(in srgb, var(--_accent) 22%, transparent); border-color: var(--_accent); }
.matrix-picker .cell.filled { background-image: radial-gradient(circle, var(--_muted) 2px, transparent 2.5px); }
.matrix-picker .cell.cut { border-color: #c0392b; background-color: color-mix(in srgb, #c0392b 18%, transparent); }
.matrix-picker .size { margin-top: 8px; text-align: center; font-variant-numeric: tabular-nums; }
.matrix-picker .warn { color: #c0392b; font-size: 12px; text-align: center; min-height: 1em; }
`;

/** Opens the grid under `anchor` (a palette's button), for `field`. */
export function openMatrixPicker(anchor: HTMLElement, field: ContextEquationField, locale: string): HTMLElement {
  const root = anchor.getRootNode() as ShadowRoot | Document;
  (root.querySelector?.(".matrix-picker") as HTMLElement | null)?.remove();
  const current = field.editor.shapeableMatrix();
  let env = current?.env ?? "pmatrix";
  let rows = current?.rows ?? 1;
  let cols = current?.cols ?? 1;
  let shownRows = Math.min(MAX, Math.max(6, rows + 2));
  let shownCols = Math.min(MAX, Math.max(8, cols + 2));

  const pop = document.createElement("div");
  pop.className = "matrix-picker";
  pop.setAttribute("popover", "auto");
  pop.setAttribute("role", "dialog");
  const title = say(current?.diagram ? WORDS.diagram : current ? WORDS.resize : WORDS.insert, locale);
  pop.setAttribute("aria-label", title);
  pop.innerHTML = `<style>${STYLE}</style><div class="title">${html(title)}</div>
<div class="delims"${current?.diagram ? " hidden" : ""} role="group" aria-label="${html(say(WORDS.delimiters, locale))}">${DELIMS.map(([e, f, title]) => `<button type="button" data-env="${e}" title="${html(say(title, locale))}" aria-label="${html(say(title, locale))}" aria-pressed="${e === env}">${f}</button>`).join("")}</div>
<div class="cells" tabindex="0" role="slider" aria-label="${html(say(WORDS.grid, locale))}"></div>
<div class="size" aria-hidden="true"></div><div class="warn" aria-live="polite"></div>`;
  (root === document ? document.body : (root as ShadowRoot)).appendChild(pop);
  const grid = pop.querySelector<HTMLElement>(".cells")!;
  const size = pop.querySelector<HTMLElement>(".size")!;
  const warn = pop.querySelector<HTMLElement>(".warn")!;

  const lost = () => !!current?.filled.some((line, i) => line.some((f, j) => f && (i >= rows || j >= cols))) || !!current?.arrows?.some((ends) => ends.some(([i, j]) => i >= rows || j >= cols));
  let built = "";
  const paint = () => {
    // the cells are made again only when the grid grows: the one under the pointer stays
    if (built !== `${shownRows}×${shownCols}`) {
      built = `${shownRows}×${shownCols}`;
      grid.style.gridTemplateColumns = `repeat(${shownCols}, 18px)`;
      let cells = "";
      for (let i = 0; i < shownRows; i++) for (let j = 0; j < shownCols; j++) cells += `<div class="cell${current?.filled[i]?.[j] ? " filled" : ""}" data-i="${i}" data-j="${j}"></div>`;
      grid.innerHTML = cells;
    }
    for (const cell of grid.children as HTMLCollectionOf<HTMLElement>) {
      const on = Number(cell.dataset.i) < rows && Number(cell.dataset.j) < cols;
      cell.classList.toggle("on", on);
      cell.classList.toggle("cut", !on && cell.classList.contains("filled"));
    }
    size.textContent = `${rows} × ${cols}`;
    grid.setAttribute("aria-valuetext", `${rows} × ${cols}`);
    warn.textContent = lost() ? say(current?.diagram ? WORDS.lostArrows : WORDS.lost, locale) : "";
  };
  const to = (r: number, c: number) => {
    rows = Math.max(1, Math.min(MAX, r));
    cols = Math.max(1, Math.min(MAX, c));
    // the grid grows as the size reaches its edge
    shownRows = Math.min(MAX, Math.max(shownRows, rows + 1));
    shownCols = Math.min(MAX, Math.max(shownCols, cols + 1));
    paint();
  };
  const pick = () => {
    if (current) field.editor.shapeMatrix(rows, cols, env);
    else field.run(`latex:\\begin{${env}}${Array.from({ length: rows }, () => Array.from({ length: cols }, () => "#?").join("&")).join("\\\\")}\\end{${env}}`);
    pop.hidePopover();
    field.focus();
  };

  grid.addEventListener("pointermove", (e) => {
    const cell = (e.target as HTMLElement).closest<HTMLElement>(".cell");
    if (cell) to(Number(cell.dataset.i) + 1, Number(cell.dataset.j) + 1);
  });
  grid.addEventListener("click", (e) => {
    const cell = (e.target as HTMLElement).closest<HTMLElement>(".cell");
    if (cell) to(Number(cell.dataset.i) + 1, Number(cell.dataset.j) + 1);
    pick();
  });
  grid.addEventListener("keydown", (e) => {
    const moves: Record<string, [number, number]> = { ArrowRight: [0, 1], ArrowLeft: [0, -1], ArrowDown: [1, 0], ArrowUp: [-1, 0] };
    const m = moves[e.key];
    if (m) to(rows + m[0], cols + m[1]);
    else if (e.key === "Enter" || e.key === " ") pick();
    else return;
    e.preventDefault();
  });
  pop.querySelector(".delims")!.addEventListener("click", (e) => {
    const b = (e.target as HTMLElement).closest<HTMLElement>("button");
    if (!b) return;
    env = b.dataset.env!;
    for (const x of pop.querySelectorAll(".delims button")) x.setAttribute("aria-pressed", String(x === b));
    // a matrix there: its delimiters change at once
    if (current) field.editor.shapeMatrix(current.rows, current.cols, env);
    grid.focus();
  });
  pop.addEventListener("toggle", (e) => {
    // Escape or a click outside: the field again
    if ((e as ToggleEvent).newState !== "closed") return;
    pop.remove();
    field.focus();
  });

  paint();
  pop.showPopover();
  // under the button, kept inside the window
  const a = anchor.getBoundingClientRect();
  const w = pop.offsetWidth;
  const h = pop.offsetHeight;
  pop.style.left = `${Math.max(8, Math.min(a.left, innerWidth - w - 8))}px`;
  pop.style.top = `${a.bottom + 6 + h > innerHeight - 8 ? Math.max(8, a.top - h - 6) : a.bottom + 6}px`;
  grid.focus();
  return pop;
}
