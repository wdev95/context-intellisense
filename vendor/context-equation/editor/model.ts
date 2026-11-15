// The formula being edited: a tree of rows and atoms, a cursor and a selection, and the commands
// that change them. No DOM and no contextEquation here: `toLatex` writes the LaTeX that contextEquation draws, with
// each atom and each row marked (`\htmlData{m=<id>}{…}`) so the boxes contextEquation returns tell where
// everything sits.

import { readLatex, type AtomSpec } from "./latex.ts";
import { writeDiagram, type ArrowData, type ArrowStyle, type DiagramEnv } from "./diagram.ts";
import { REACTION_ARROWS, SCHEME_ARROWS, arrowText, chemToLatex, conditionPieces, latexToChem, writeReaction, type PieceData } from "./reaction.ts";
import { addBond, addRing, atomAt, chemfigLabel, editable, labelText, markBody, readGraph, removeAtom, setBondKind, setLabel, type Graph } from "./structure.ts";
import { CALLS, CHAR_CLASS, CHAR_LATEX, CLOSING, COMPOSED, DELIMITERS, LOWER_LIMIT_ONLY, WORDS, classOfCommand, escapeText, type AtomClass } from "./symbols.ts";

export type { AtomClass } from "./symbols.ts";
export type { AtomSpec } from "./latex.ts";
export type { ArrowStyle, DiagramEnv } from "./diagram.ts";

/** A horizontal list: the whole formula, a numerator, an exponent, a cell… */
export interface Row {
  kind: "row";
  id: string;
  children: Atom[];
  /** The atom this row belongs to; none for the formula itself. */
  parent: Atom | null;
}

interface Base {
  id: string;
  /** The row this atom is in. */
  parent: Row | null;
}

/** One symbol: a letter, a digit, an operator, `\alpha`, `\sin`, a space… */
export interface SymbolAtom extends Base { kind: "symbol"; latex: string; cls: AtomClass }
/** One character of text (inside `\text{…}`). */
export interface CharAtom extends Base { kind: "char"; ch: string }
export interface TextAtom extends Base { kind: "text"; cmd: string; body: Row }
/** `\frac`, `\dfrac`, `\tfrac`, `\cfrac`, `\binom`… */
export interface Fraction extends Base { kind: "frac"; cmd: string; num: Row; den: Row }
export interface Root extends Base { kind: "sqrt"; index: Row | null; body: Row }
/** An exponent, an index, or both, on the atom before it. */
export interface Scripts extends Base { kind: "scripts"; sup: Row | null; sub: Row | null }
/** Delimiters around a row: they grow (`\left…\right`) only when what they hold is tall. */
export interface Delimited extends Base { kind: "delim"; left: string; right: string; body: Row; auto: boolean }
/** A large operator with its limits: `\sum`, `\int`, `\lim`… */
export interface LargeOperator extends Base { kind: "bigop"; op: string; sub: Row | null; sup: Row | null }
/** `\vec`, `\hat`, `\overline`… over a row. */
export interface Accent extends Base { kind: "accent"; cmd: string; body: Row }
/** `\mathbf`, `\mathbb`, `\operatorname`… over a row. */
export interface Styled extends Base { kind: "style"; cmd: string; body: Row }
/** A grid of cells: matrices, `cases`, `aligned`. */
export interface Matrix extends Base { kind: "matrix"; env: string; cells: Row[][]; rowLabels?: (string | null)[]; /** An array's columns, as written. */ spec?: string }

/** A command whose arguments are math (`\pdv{f}{x}`, `\ket{\psi}`, `\xrightarrow[a]{b}`), each a place; `opt` its `[…]`. */
export interface Call extends Base { kind: "call"; name: string; opt: Row | null; args: Row[] }
/** LaTeX the editor does not take apart: drawn as written, moved and deleted whole. */
export interface Raw extends Base { kind: "raw"; latex: string }
/** A command being typed after `\`, until a space or Enter turns it into what it names. */
export interface Command extends Base { kind: "command"; name: string }
/** An arrow of a diagram: from a cell to a cell, its labels rows to edit. */
export type DiagramArrow = ArrowData<Row>;
/** A commutative diagram (tikz-cd, amscd, xymatrix): a grid of cells and the arrows between them. */
export interface Diagram extends Base { kind: "diagram"; env: DiagramEnv; options?: string; cells: Row[][]; arrows: DiagramArrow[] }

/** A reaction (mhchem's \ce): a row of species, arrows and signs. */
export interface Reaction extends Base { kind: "reaction"; body: Row; /** A chemfig scheme (\schemestart): compounds are structures, arrows chemfig's. */ scheme?: boolean }
/** A species of a reaction: its formula, edited as math (`2H_2O(l)`, `SO_4^{2-}`) and written as mhchem writes it. */
export interface Species extends Base { kind: "species"; formula: Row }
/** An arrow of a reaction (`->`, `<=>`…), with what goes over and under it. */
export interface ReactionArrow extends Base { kind: "rxarrow"; arrow: string; above: Row; below: Row }
/** A sign of a reaction: `+`, `^` (a gas), `v` (a precipitate), or an arrow being typed. */
export interface ReactionToken extends Base { kind: "rxtoken"; text: string }

export type Atom = SymbolAtom | CharAtom | TextAtom | Fraction | Root | Scripts | Delimited | LargeOperator | Accent | Styled | Matrix | Call | Raw | Command | Diagram | Reaction | Species | ReactionArrow | ReactionToken;

/** Where a new arrow goes from a cell. */
export type ArrowDirection = "up" | "down" | "left" | "right";

/** Where the cursor is: between two atoms of a row, `offset` atoms from its start. */
export interface Cursor {
  row: Row;
  offset: number;
}

/** A selection: atoms `start` to `end` (not included) of one row. */
export interface Selection {
  row: Row;
  start: number;
  end: number;
}

/** The structures a name inserts (`insert("frac")`, `\frac` typed, a palette's button). */
export const TEMPLATES: Record<string, string> = {
  frac: "\\frac{#0}{#?}",
  dfrac: "\\dfrac{#0}{#?}",
  tfrac: "\\tfrac{#0}{#?}",
  cfrac: "\\cfrac{#0}{#?}",
  binom: "\\binom{#0}{#?}",
  sqrt: "\\sqrt{#0}",
  nroot: "\\sqrt[#?]{#0}",
  root: "\\sqrt[#?]{#0}",
  paren: "\\left(#0\\right)",
  brackets: "\\left[#0\\right]",
  braces: "\\left\\{#0\\right\\}",
  abs: "\\left|#0\\right|",
  norm: "\\left\\|#0\\right\\|",
  angle: "\\left\\langle #0\\right\\rangle",
  floor: "\\left\\lfloor #0\\right\\rfloor",
  ceil: "\\left\\lceil #0\\right\\rceil",
  sum: "\\sum_{#?}^{#?}#0",
  prod: "\\prod_{#?}^{#?}#0",
  coprod: "\\coprod_{#?}^{#?}#0",
  int: "\\int_{#?}^{#?}#0",
  iint: "\\iint_{#?}#0",
  iiint: "\\iiint_{#?}#0",
  oint: "\\oint_{#?}#0",
  bigcup: "\\bigcup_{#?}^{#?}#0",
  bigcap: "\\bigcap_{#?}^{#?}#0",
  lim: "\\lim_{#?}#0",
  limsup: "\\limsup_{#?}#0",
  liminf: "\\liminf_{#?}#0",
  max: "\\max_{#?}#0",
  min: "\\min_{#?}#0",
  sup_: "\\sup_{#?}#0",
  inf: "\\inf_{#?}#0",
  text: "\\text{#0}",
  matrix: "\\startmatrix#?\\NC#?\\NR#?\\NC#?\\stopmatrix",
  pmatrix: "\\startpmatrix#?\\NC#?\\NR#?\\NC#?\\stoppmatrix",
  bmatrix: "\\startbmatrix#?\\NC#?\\NR#?\\NC#?\\stopbmatrix",
  Bmatrix: "\\startBmatrix#?\\NC#?\\NR#?\\NC#?\\stopBmatrix",
  vmatrix: "\\startvmatrix#?\\NC#?\\NR#?\\NC#?\\stopvmatrix",
  Vmatrix: "\\startVmatrix#?\\NC#?\\NR#?\\NC#?\\stopVmatrix",
  cases: "\\startcases#?\\NC#?\\NR#?\\NC#?\\stopcases",
  aligned: "\\startalign#?\\NC#?\\NR#?\\NC#?\\stopalign",
};
for (const a of ["vec", "hat", "bar", "dot", "ddot", "tilde", "widehat", "widetilde", "overline", "underline", "overrightarrow", "overleftarrow", "overbrace", "underbrace", "boxed", "cancel"]) TEMPLATES[a] = `\\${a}{#0}`;
for (const s of ["mathbf", "mathbb", "mathcal", "mathscr", "mathfrak", "mathrm", "mathit", "mathsf", "mathtt", "boldsymbol"]) TEMPLATES[s] = `\\${s}{#0}`;

/** Commands typed after `\` that open a block of their own language (mhchem, chemfig, diagrams), to
 * edit as LaTeX: `‸` is where the cursor starts in it. */
export const BLOCKS: Record<string, string> = {
  pu: "\\pu{‸}",
};

/** Commands typed after `\` that start a diagram: one empty cell, the cursor in it. */
const STRUCTURED: Record<string, string> = { ce: "\\ce{}", chemfig: "\\chemfig{}", schemestart: "\\schemestart \\chemfig{} \\schemestop" };

/** The selection inside a structure: an atom or a bond, by its index in contextEquation's reading. */
export interface ChemSelection { atom: Raw; type: "atom" | "bond"; index: number; graph: Graph }

const CHEMFIG = /^\\chemfig\{([\s\S]*)\}$/;
/** Characters that end a label in chemfig's code (bonds, branches, rings…): not typed into one. */
const LABEL_STOPS = /[-=~<>()*!?@\[\],\s]/;

const DIAGRAMS: Record<string, string> = { tikzcd: "\\begin{tikzcd}#?\\end{tikzcd}", CD: "\\begin{CD}#?\\end{CD}", xymatrix: "\\xymatrix{#?}" };

/** Editing a block's LaTeX: changes show at once; `commit` makes them one step of undo, `cancel` takes them back. */
export interface BlockSession {
  atom: Raw;
  /** Where the cursor starts in the LaTeX. */
  caret: number;
  update(latex: string): void;
  commit(): void;
  cancel(): void;
}

/** Names of the structures that attach to the atom before the cursor. */
export type ScriptTemplate = "sup" | "sub";

/** Wrappers that keep a marked symbol's class: a group alone is an ordinary atom, as in TeX. */
const CLASS_WRAPPER: Record<AtomClass, string | null> = { ord: null, bin: "\\mathbin", rel: "\\mathrel", open: "\\mathopen", close: "\\mathclose", punct: "\\mathpunct", op: null, inner: "\\mathinner" };

/** The class of LaTeX the editor keeps as written, from the tree contextEquation reads it into (`render(latex).tree`). */
export function classOfTree(tree: unknown): AtomClass {
  const node = tree as { type?: string; children?: unknown[] };
  // spaces are not neighbors; of several pieces, the first sets the spacing on the left
  const children = (node?.type === "row" ? (node.children ?? []) : [node]).filter((c) => (c as { type?: string })?.type !== "space");
  if (!children.length) return "ord";
  const n = children[0] as { type?: string; class?: string; base?: unknown; shape?: string };
  switch (n?.type) {
    case "operator":
    case "forced-class":
      return (n.class as AtomClass) ?? "ord";
    case "function":
    case "large-operator":
      return "op";
    case "fraction":
    case "delimited":
      return "inner";
    case "table":
      return n.shape && !/array|aligned|centered|substack|diagram/.test(n.shape) ? "inner" : "ord";
    case "scripts":
    case "attributed":
    case "colored":
    case "style":
      return n.base ? classOfTree(n.base) : "ord";
    default:
      return "ord";
  }
}

export interface LatexOptions {
  /** Mark every atom and row with `\htmlData{m=<id>}`, and draw empty rows as `\square`: what the editor draws. Off: the plain LaTeX to hand out. */
  marked?: boolean;
  /** Also mark the empty cells of diagrams (which moves tikz-cd's layout a little): only to measure where they are. */
  probeEmptyCells?: boolean;
}

export type ChangeKind = "content" | "cursor";
export type ChangeListener = (kind: ChangeKind) => void;

interface Snapshot {
  root: AtomSpec[];
  cursor: [number, number][];
  offset: number;
  anchor: [number, number][] | null;
  anchorOffset: number;
}

/** An editor without a screen: the formula, the cursor, the selection and the commands. */
export class Editor {
  root: Row;
  cursor: Cursor;
  /** The other end of the selection; none when nothing is selected. */
  anchor: Cursor | null = null;
  /** Typing as on a calculator (on by default): words become what they name (`sqrt`, `pi`, `sin`),
   * `<=` is ≤, an exponent or index that is a number or a name ends at the operator after it, `/`
   * takes `(a+b)` without its parentheses, and `(…)` typed alone in an exponent, a root or a
   * denominator is that place, closed by `)`. Off: each character is itself. */
  smartTyping = true;
  /** Places opened with `{` (as LaTeX groups them): `}` leaves them, and nothing typed in them ends them. */
  private braced = new WeakSet<Row>();
  /** Atoms a space was typed after: the letters on either side are not one word. */
  private wordBreaks = new WeakSet<Atom>();
  /** Where a space was typed: it ends a word, and keeps `<` and `=` apart. */
  private spacedAt: Cursor | null = null;
  /** The locale writes `3,14` for a decimal (pt-BR, fr, de…): a comma between digits is then part of the number. The field sets it from contextEquation. */
  decimalComma = false;
  /** The class of LaTeX kept as written (`\pdv{u}{t}` is a fraction), so its mark keeps its spacing. The field asks contextEquation (`classOfTree(render(latex).tree)`); without it, ordinary. */
  classifyRaw: ((latex: string) => AtomClass) | null = null;
  /** contextEquation's reading of LaTeX (its `tree`), for structures: which atoms and bonds a \chemfig has and
   * where each is written. The field sets it; without it, a structure is a block of LaTeX. */
  parseChem: ((latex: string) => unknown) | null = null;
  private graphs = new Map<string, Graph | null>();
  private chem: { atom: Raw; type: "atom" | "bond"; index: number; fresh: boolean; row: Row; offset: number } | null = null;
  private rawClasses = new Map<string, AtomClass>();
  /** The formula before boxes became a block to edit as LaTeX (`toBlock`). */
  private blockOrigin: string | null = null;
  /** A block just inserted that wants its LaTeX edited (the field opens the editor of blocks). */
  private pendingBlock: { atom: Raw; caret: number } | null = null;
  private next = 0;
  private listeners = new Set<ChangeListener>();
  private undoStack: string[] = [];
  private redoStack: string[] = [];
  private batching = 0;
  /** Typing in a row: letters typed one after another are one step of undo. */
  private typingRun = false;
  private typingAt = 0;

  constructor(latex = "") {
    this.root = this.row(null);
    this.cursor = { row: this.root, offset: 0 };
    if (latex) {
      this.adoptInto(this.root, readLatex(latex), []);
      this.cursor = { row: this.root, offset: this.root.children.length };
    }
  }

  // ---------- reading ----------

  /** The formula as LaTeX: plain to hand out, or marked to draw with contextEquation. */
  toLatex(options: LatexOptions = {}): string {
    this.probing = !!options.probeEmptyCells;
    try {
      return this.rowLatex(this.root, options.marked ?? false);
    } finally {
      this.probing = false;
    }
  }

  /** Does the formula have a diagram with an empty cell (placed by a probe drawing)? */
  hasEmptyDiagramCells(): boolean {
    return this.rows().some((r) => r.parent?.kind === "diagram" && r.children.length === 0 && r.parent.cells.some((line) => line.includes(r)));
  }

  private probing = false;

  /** Every row, in document order (outer before inner, left to right). */
  rows(): Row[] {
    const out: Row[] = [];
    const walk = (r: Row) => {
      out.push(r);
      for (const a of r.children) for (const c of childRows(a)) walk(c);
    };
    walk(this.root);
    return out;
  }

  isEmpty(): boolean {
    return this.root.children.length === 0;
  }

  /** The selection, as atoms of one row; none when nothing is selected. */
  selection(): Selection | null {
    if (!this.anchor) return null;
    const a = this.chain(this.anchor);
    const c = this.chain(this.cursor);
    let i = 0;
    while (i + 1 < a.length && i + 1 < c.length && a[i + 1].row === c[i + 1].row) i++;
    const row = a[i].row;
    const edges = [a[i].start, a[i].end, c[i].start, c[i].end];
    const start = Math.min(...edges);
    const end = Math.max(...edges);
    return start === end ? null : { row, start, end };
  }

  /** The selection as plain LaTeX (empty when nothing is selected). */
  selectedLatex(): string {
    const s = this.selection();
    if (!s) return "";
    const atoms = s.row.children.slice(s.start, s.end);
    // pieces of a reaction: a reaction of their own (a piece alone has no LaTeX)
    const owner = s.row.parent;
    if (owner?.kind === "reaction") return this.atomLatex({ ...owner, body: { ...s.row, children: atoms } }, false);
    return this.listLatex(atoms, false);
  }

  /** The command being typed after `\`, if any. */
  commandBeingTyped(): Command | null {
    const { row, offset } = this.cursor;
    const before = row.children[offset - 1];
    return before && before.kind === "command" ? before : null;
  }

  /** Is the cursor in text (inside `\text{…}`)? */
  inText(): boolean {
    const owner = this.cursor.row.parent;
    return !!owner && owner.kind === "text";
  }

  // ---------- structures (\chemfig) ----------

  /** The atoms and bonds of a \chemfig block, when the editor can edit it in place. */
  graphOf(raw: Raw): Graph | null {
    const m = CHEMFIG.exec(raw.latex);
    if (!m || !this.parseChem || balanced(m[1]) !== m[1]) return null;
    if (!this.graphs.has(raw.latex)) {
      let g: Graph | null = null;
      try {
        g = readGraph(this.parseChem(raw.latex));
      } catch {
        g = null;
      }
      this.graphs.set(raw.latex, g && editable(g) ? g : null);
    }
    return this.graphs.get(raw.latex) ?? null;
  }

  /** The atom or bond selected in a structure (while the cursor stays where the selection put it). */
  chemSelection(): ChemSelection | null {
    const c = this.chem;
    if (!c || this.anchor || this.cursor.row !== c.row || this.cursor.offset !== c.offset || c.row.children[c.offset - 1] !== c.atom) return null;
    const graph = this.graphOf(c.atom);
    if (!graph) return null;
    const count = c.type === "atom" ? graph.atoms.length : graph.bonds.length;
    if (c.index < 0 || c.index >= count) return null;
    return { atom: c.atom, type: c.type, index: c.index, graph };
  }

  /** Selects an atom or a bond of a structure; typing then writes the atom's label. */
  selectChem(atom: Raw, type: "atom" | "bond", index: number): void {
    const row = atom.parent;
    if (!row) return;
    const offset = row.children.indexOf(atom) + 1;
    this.anchor = null;
    this.cursor = { row, offset };
    this.chem = { atom, type, index, fresh: true, row, offset };
    this.emit("cursor");
  }

  /** Leaves the structure (the cursor stays after it). */
  clearChem(): void {
    if (!this.chem) return;
    this.chem = null;
    this.emit("cursor");
  }

  private setStructure(atom: Raw, body: string): void {
    atom.latex = `\\chemfig{${body}}`;
  }

  private chemBody(atom: Raw): string {
    return CHEMFIG.exec(atom.latex)?.[1] ?? "";
  }

  /** A new label for the selected atom (`OH`, `CH_3`, empty for a carbon at a vertex). */
  setChemLabel(label: string): void {
    const s = this.chemSelection();
    if (!s || s.type !== "atom") return;
    this.change(() => this.setStructure(s.atom, setLabel(this.chemBody(s.atom), s.graph, s.index, chemfigLabel(label))));
    if (this.chem) this.chem.fresh = false;
  }

  /** A bond from the selected atom in `angle` degrees (counterclockwise from the right) to a new atom,
   * which is then selected; with a bond selected, from its far atom. */
  addChemBond(angle: number, kind = "single", label = ""): void {
    const s = this.chemSelection();
    if (!s) return;
    const from = s.type === "atom" ? s.index : s.graph.bonds[s.index].to;
    let at = -1;
    this.change(() => {
      const r = addBond(this.chemBody(s.atom), s.graph, from, angle, kind, label);
      at = r.at;
      this.setStructure(s.atom, r.body);
    });
    const g = this.graphOf(s.atom);
    const k = g ? atomAt(g, at) : -1;
    if (k >= 0) this.selectChem(s.atom, "atom", k);
  }

  /** The selected bond's kind (`single`, `double`, `triple`, `wedge`, `hash`…). */
  setChemBondKind(kind: string): void {
    const s = this.chemSelection();
    if (!s || s.type !== "bond") return;
    this.change(() => this.setStructure(s.atom, setBondKind(this.chemBody(s.atom), s.graph, s.index, kind)));
  }

  /** A ring on the selected atom (`size` sides; `aromatic`: alternate double bonds). */
  addChemRing(size: number, aromatic: boolean): void {
    const s = this.chemSelection();
    if (!s || s.type !== "atom") return;
    this.change(() => this.setStructure(s.atom, addRing(this.chemBody(s.atom), s.graph, s.index, size, aromatic)));
    this.selectChem(s.atom, "atom", s.index);
  }

  /** Removes the selected atom (with its bond, when nothing else hangs from it; otherwise its label
   * goes) or the selected bond's far atom; the atom it hung from is selected. */
  removeChem(): void {
    const s = this.chemSelection();
    if (!s) return;
    const k = s.type === "atom" ? s.index : s.graph.bonds[s.index].to;
    const parent = s.graph.bonds.find((b) => b.to === k)?.from ?? 0;
    const before = s.atom.latex;
    // an empty structure (one atom, no label): it goes whole, from a scheme or from the formula
    if (s.graph.atoms.length === 1 && !s.graph.atoms[0].label) {
      this.clearChem();
      return this.change(() => {
        const row = s.atom.parent!;
        this.removeAtom(s.atom);
        // a scheme left with nothing goes too
        const scheme = row.parent;
        if (scheme?.kind === "reaction" && scheme.scheme && !row.children.some((a) => a.kind === "raw")) this.removeAtom(scheme);
      });
    }
    this.change(() => this.setStructure(s.atom, removeAtom(this.chemBody(s.atom), s.graph, k)));
    const g = this.graphOf(s.atom);
    const removed = g && g.atoms.length < s.graph.atoms.length;
    if (s.atom.latex !== before) this.selectChem(s.atom, "atom", removed ? Math.min(parent, g!.atoms.length - 1) : k);
  }

  /** The atom next to the selected one in a direction (degrees): the bond closest to it. */
  moveChem(angle: number): void {
    const s = this.chemSelection();
    if (!s) return;
    const k = s.type === "atom" ? s.index : s.graph.bonds[s.index].to;
    let best = -1;
    let gap = 70;
    for (const b of s.graph.bonds) {
      const [other, dir] = b.from === k ? [b.to, b.angle] : b.to === k ? [b.from, b.angle + 180] : [-1, 0];
      if (other < 0) continue;
      const d = Math.abs(((dir - angle) % 360 + 540) % 360 - 180);
      if (d < gap) {
        gap = d;
        best = other;
      }
    }
    if (best >= 0) this.selectChem(s.atom, "atom", best);
  }

  /** In a scheme just made: the first structure's substituent R (or its first atom) picked. */
  private pickFirstStructure(scheme: Reaction): void {
    for (const [i, p] of scheme.body.children.entries()) {
      const g = p.kind === "raw" ? this.graphOf(p) : null;
      if (!g || p.kind !== "raw") continue;
      const r = g.atoms.findIndex((a) => a.label === "R");
      this.cursor = { row: scheme.body, offset: i + 1 };
      this.anchor = null;
      this.chem = { atom: p, type: "atom", index: Math.max(0, r), fresh: true, row: scheme.body, offset: i + 1 };
      return;
    }
  }

  /** Typing on a selected atom: its label (the first character replaces it). */
  private typeInStructure(ch: string): boolean {
    const s = this.chemSelection();
    if (!s || !this.chem) return false;
    // a space leaves the structure: what follows is typed after it (in a scheme, an arrow)
    if (ch === " ") {
      this.clearChem();
      return true;
    }
    if (s.type !== "atom") return true;
    const current = s.graph.atoms[s.index].label;
    // a sign after ^ is a charge (O^-); other bond and branch characters end nothing here
    if (LABEL_STOPS.test(ch) && !(/[-+]/.test(ch) && labelText(current).endsWith("^") && !this.chem.fresh)) return true;
    const label = this.chem.fresh ? ch : labelText(current) + ch;
    this.change(() => this.setStructure(s.atom, setLabel(this.chemBody(s.atom), s.graph, s.index, chemfigLabel(label))), true);
    this.chem.fresh = false;
    return true;
  }

  /** Backspace on a selected atom: a letter of its label; on one just selected (or empty), the atom. */
  private backspaceInStructure(): boolean {
    const s = this.chemSelection();
    if (!s || !this.chem) return false;
    const label = s.type === "atom" ? s.graph.atoms[s.index].label : "";
    if (s.type === "bond" || this.chem.fresh || !label) {
      this.removeChem();
      return true;
    }
    const chars = Array.from(labelText(label));
    chars.pop();
    this.change(() => this.setStructure(s.atom, setLabel(this.chemBody(s.atom), s.graph, s.index, chemfigLabel(chars.join("")))), true);
    return true;
  }

  /** The reaction the cursor is in, with where: its row of pieces, or a species' or an arrow's row. */
  reactionAtCursor(): { reaction: Reaction; piece: Species | ReactionArrow | null } | null {
    const row = this.cursor.row;
    const owner = row.parent;
    if (owner?.kind === "reaction") return { reaction: owner, piece: null };
    if ((owner?.kind === "species" || owner?.kind === "rxarrow") && owner.parent?.parent?.kind === "reaction") return { reaction: owner.parent.parent, piece: owner };
    // deeper in a species (its index, its charge): the species
    const species = this.speciesAround();
    if (species?.parent?.parent?.kind === "reaction") return { reaction: species.parent.parent, piece: species };
    return null;
  }

  /** The arrow of a reaction or scheme the cursor is on: in its conditions, or beside it (after it, or before it, where a click on it puts the cursor). */
  activeArrow(): ReactionArrow | null {
    const { row, offset } = this.cursor;
    if (row.parent?.kind === "rxarrow") return row.parent;
    if (row.parent?.kind !== "reaction") return null;
    const beside = [row.children[offset - 1], row.children[offset]].find((a) => a?.kind === "rxarrow");
    return (beside as ReactionArrow | undefined) ?? null;
  }

  /** The matrix or diagram around the cursor, with the cell it is in. */
  matrixAtCursor(): { matrix: Matrix | Diagram; i: number; j: number } | null {
    for (let row: Row = this.cursor.row; row.parent; row = row.parent.parent!) {
      const owner = row.parent;
      if (owner.kind === "matrix" || owner.kind === "diagram") {
        for (let i = 0; i < owner.cells.length; i++) {
          const j = owner.cells[i].indexOf(row);
          if (j >= 0) return { matrix: owner, i, j };
        }
      }
    }
    return null;
  }

  /** The matrix around the cursor that `shapeMatrix` can shape (one with delimiters or none, not
   * cases or alignments), with its size and which cells hold something. */
  shapeableMatrix(): { env: string; rows: number; cols: number; filled: boolean[][]; diagram: boolean; arrows?: number[][][] } | null {
    const at = this.matrixAtCursor();
    // a diagram's grid too (its delimiters are none), and the physics package's matrices (by the
    // environment of the same delimiters)
    if (!at || (at.matrix.kind === "matrix" && !MATRIX_SHAPES.has(at.matrix.env))) return null;
    const cells = at.matrix.cells;
    const diagram = at.matrix.kind === "diagram";
    // a diagram's arrows by their ends, to say which a smaller size takes away
    const arrows = at.matrix.kind === "diagram" ? at.matrix.arrows.map((a) => [a.from, a.to]) : undefined;
    return { env: at.matrix.env, rows: cells.length, cols: cells[0].length, filled: cells.map((line) => line.map((c) => c.children.length > 0)), diagram, ...(arrows ? { arrows } : {}) };
  }

  /** Gives the matrix around the cursor `rows` lines and `cols` columns, empty cells added at the
   * end, the last lines and columns removed (with what they held: one step of undo); `env` changes
   * its delimiters. The cursor stays in its cell, or the nearest one left. */
  shapeMatrix(rows: number, cols: number, env?: string): void {
    const at = this.matrixAtCursor();
    if (!this.shapeableMatrix() || rows < 1 || cols < 1) return;
    const m = at!.matrix;
    this.change(() => {
      m.cells = Array.from({ length: rows }, (_, i) => Array.from({ length: cols }, (_, j) => m.cells[i]?.[j] ?? this.row(m)));
      if (m.kind === "matrix") m.rowLabels = Array.from({ length: rows }, (_, i) => m.rowLabels?.[i] ?? null);
      // a diagram's arrows to cells that went, go with them
      if (m.kind === "diagram") m.arrows = m.arrows.filter((a) => [a.from, a.to].every(([i, j]) => i < rows && j < cols));
      // a physics matrix stays one where the package has those delimiters
      else if (env && MATRIX_SHAPES.has(env)) m.env = env;
      const cell = m.cells[Math.min(at!.i, rows - 1)][Math.min(at!.j, cols - 1)];
      if (cell !== this.cursor.row) this.cursor = { row: cell, offset: cell.children.length };
      this.anchor = null;
    });
  }

  /** Called after every change: `content` when the formula changed, `cursor` when only the cursor or the selection moved. Returns a function that removes it. */
  onChange(listener: ChangeListener): () => void {
    this.listeners.add(listener);
    return () => this.listeners.delete(listener);
  }

  // ---------- whole formula ----------

  /** Replaces the formula with this LaTeX; the cursor goes to the end. */
  setLatex(latex: string): void {
    this.change(() => {
      this.root.children = [];
      this.adoptInto(this.root, readLatex(latex), []);
      this.cursor = { row: this.root, offset: this.root.children.length };
      this.anchor = null;
    });
  }

  /** Empties the formula. */
  clear(): void {
    this.setLatex("");
  }

  // ---------- typing ----------

  /** Types one character, as a keyboard does. In math: letters, digits and operators become symbols;
   * `/` makes a fraction of what is before the cursor; `^` and `_` an exponent and an index; `(`, `[`,
   * `{` and `|` open delimiters that grow, and their closing character leaves them; `\` starts a
   * command that a space or Enter finishes. In text, every character is itself. */
  type(ch: string): void {
    if (this.typeInStructure(ch)) return;
    const species = this.speciesAround();
    // a space anywhere in a species (its index too) ends it
    if (species && ch === " " && !this.commandBeingTyped()) return this.change(() => this.afterPiece(species));
    // over or under an arrow, `\` is a character too: the conditions are mhchem's text (`->[\Delta]`)
    const rx = this.reactionAtCursor();
    if (rx && !this.commandBeingTyped() && (ch !== "\\" || rx.piece?.kind === "rxarrow") && !this.speciesTakes(ch)) return this.typeInReaction(ch);
    if (this.inText() && !this.commandBeingTyped()) {
      if (ch === "\n") return;
      return this.change(() => this.replaceSelectionWith([{ kind: "char", ch }]), true);
    }
    const command = this.commandBeingTyped();
    if (command) {
      if (/^[A-Za-z]$/.test(ch)) return this.change(() => void (command.name += ch), true);
      // `\` then one other character: the command is that character (`\,`, `\{`, `\|`)
      if (command.name === "" && ",:;! {}|#$%&_".includes(ch) && ch !== " ") {
        return this.change(() => {
          const row = command.parent!;
          const at = row.children.indexOf(command);
          row.children.splice(at, 1);
          this.cursor = { row, offset: at };
          this.insertSpecs(readLatex(`\\${ch}`));
        });
      }
      let opened = false;
      this.change(() => void (opened = this.finishCommand(command)));
      // the space or the brace that ended \ce goes: the block's editor takes it from there
      if (ch === " " || (opened && ch === "{")) return;
    }
    const spaced = this.spacedAt?.row === this.cursor.row && this.spacedAt.offset === this.cursor.offset;
    this.spacedAt = null;
    const smart = this.smartTyping && !this.anchor && this.cursor.row.parent?.kind !== "style" && !species;
    const letter = /^[A-Za-z]$/.test(ch);
    if (ch === " " || ch === "\n") {
      if (smart && ch === " ") {
        // a space ends a word (letters on either side are not one word) and a place a calculator ends
        const before = this.cursor.row.children[this.cursor.offset - 1];
        if (before) this.wordBreaks.add(before);
        this.leavePlace(ch);
        this.spacedAt = { ...this.cursor };
      }
      return;
    }
    if (this.smartTyping && !this.anchor && this.typeBrace(ch)) return;
    if (smart && !spaced && this.compose(ch)) return;
    if (smart && this.typeInLimit(ch)) return;
    if (smart) this.leavePlace(ch);
    if (species && this.smartTyping && !this.anchor && this.typeInSpecies(ch)) return;
    if (smart && letter) {
      return this.change(() => {
        if (!spaced) this.reopenWord(ch);
        this.replaceSelectionWith([{ kind: "symbol", latex: ch, cls: "ord" }]);
        this.convertWord();
      }, true);
    }
    if (ch === "\\") return this.change(() => this.replaceSelectionWith([{ kind: "command", name: "" }]));
    if (ch === "/") return this.change(() => this.fractionOfOperand());
    if (ch === "^") return this.script("sup");
    if (ch === "_") return this.script("sub");
    if (ch === "&" || ch === "\t") return this.nextCell();
    const opener = ({ "(": "(", "[": "[", "{": "\\{", "|": "|" } as Record<string, string>)[ch];
    const closer = ({ ")": ")", "]": "]", "}": "\\}", "|": "|" } as Record<string, string>)[ch];
    if (closer && this.leaveDelimiter(closer)) return;
    if (opener) return this.change(() => this.insertSpecs(readLatex(`\\left${opener}#0\\right${CLOSING[opener]}`, { autoDelimiters: true })));
    if (/^[A-Za-z0-9.]$/.test(ch)) return this.typeSymbol(ch, "ord");
    const cls = CHAR_CLASS[ch];
    if (cls) return this.typeSymbol(CHAR_LATEX[ch] ?? ch, cls);
    if (ch === "∞") return this.insertSymbol("\\infty", "ord");
    if (ch === "'") return this.insertSymbol("'", "ord");
    // any other character (π, α, ∫ typed or pasted): a symbol of its own
    if (ch.trim()) this.insertSymbol(ch, "ord");
  }

  // ---------- typing as on a calculator (`smartTyping`) ----------

  /** The species the cursor is in, at any depth (its index, its charge). */
  private speciesAround(): Species | null {
    for (let r: Row = this.cursor.row; r.parent; r = r.parent.parent!) {
      if (r.parent.kind === "species") return r.parent;
      if (r.parent.kind === "reaction") return null;
    }
    return null;
  }

  /** Does the species the cursor is in take this character as math? Not `->` or `<`, which start an arrow. */
  private speciesTakes(ch: string): boolean {
    const at = this.reactionAtCursor();
    if (at?.piece?.kind !== "species") return false;
    const { row, offset } = this.cursor;
    const last = row.children[offset - 1];
    const atEnd = offset === row.children.length;
    return !(atEnd && (ch === "<" || (ch === ">" && last?.kind === "symbol" && last.latex === "-")));
  }

  /** Typing in a species, as chemistry is written: a number after an element or a closing is its
   * index (`H2` is H₂); in an index or a charge that is done, a letter or an opening goes on after it,
   * and `^` after an index starts the charge over it (`SO4^2-`). */
  private typeInSpecies(ch: string): boolean {
    const { row, offset } = this.cursor;
    const owner = row.parent;
    if (owner?.kind === "scripts" && offset === row.children.length && !this.braced.has(row)) {
      const text = row.children.map((a) => (a.kind === "symbol" ? a.latex : "?")).join("");
      const index = row === owner.sub && /^[0-9]+$/.test(text);
      // a letter or an opening ends a number; after an index, a sign or a dot too (NH4+, CH3-CH3, CuSO4.5H2O)
      const done = (/^[0-9]+$/.test(text) && /^[A-Za-z(\[^]$/.test(ch) && !(ch === "^" && !index)) || (index && /^[-+*.=,]$/.test(ch)) || (/^[0-9]*[+-]$/.test(text) && /^[A-Za-z(\[]$/.test(ch));
      if (!done) return false;
      const outer = owner.parent!;
      this.change(() => (this.cursor = { row: outer, offset: outer.children.indexOf(owner) + 1 }));
      // what ends it is typed after it, as chemistry
      return this.typeInSpecies(ch);
    }
    // a slash in chemistry is a coefficient's (1/2 O2), not a fraction; a star the dot of a hydrate (CuSO4*5H2O)
    if (ch === "/" || ch === "*") {
      this.change(() => this.replaceSelectionWith([{ kind: "symbol", latex: ch, cls: "ord" }]), true);
      return true;
    }
    const before = row.children[offset - 1];
    const element = before && ((before.kind === "symbol" && (/^[A-Za-z]$/.test(before.latex) || before.cls === "close")) || before.kind === "delim");
    if (!/^[0-9]$/.test(ch) || !element || owner?.kind === "scripts") return false;
    this.change(() => {
      this.addScript("sub");
      this.replaceSelectionWith([{ kind: "symbol", latex: ch, cls: "ord" }]);
    }, true);
    return true;
  }

  /** The letters just before the cursor (one-letter ordinary symbols), as one word. */
  private lettersBefore(): string {
    const { row, offset } = this.cursor;
    let text = "";
    for (let i = offset - 1; i >= 0; i--) {
      const a = row.children[i];
      if (a.kind !== "symbol" || a.cls !== "ord" || !/^[A-Za-z]$/.test(a.latex) || (i < offset - 1 && this.wordBreaks.has(a))) break;
      text = a.latex + text;
    }
    return text;
  }

  /** The `n` letters before the cursor become this LaTeX: the cursor in its first empty place, or after it. */
  private replaceLetters(n: number, latex: string): void {
    const { row, offset } = this.cursor;
    row.children.splice(offset - n, n);
    this.cursor = { row, offset: offset - n };
    this.insertSpecs(readLatex(latex), { cursorAfter: !latex.includes("#?") });
  }

  /** After a letter: the letters typed since the last non-letter (or space) become what they name,
   * when they are a whole word (`pi`, `sqrt`, `sin`); inside a longer run (`point`, `xpi`) nothing is
   * read, so words of a language and products of variables stay as typed. */
  private convertWord(): void {
    const text = this.lettersBefore();
    if (Object.hasOwn(WORDS, text)) this.replaceLetters(text.length, WORDS[text]);
  }

  /** A letter right after a word made into a symbol that it carries on (`\sin` and `h`): the word is letters again, to make the longer one. */
  private reopenWord(ch: string): void {
    const { row, offset } = this.cursor;
    const before = row.children[offset - 1];
    if (before?.kind !== "symbol") return;
    const w = before.latex.slice(1);
    if (WORDS[w] !== before.latex || !WORD_LIST.some((x) => x.startsWith(w + ch))) return;
    row.children.splice(offset - 1, 1);
    this.cursor = { row, offset: offset - 1 };
    this.replaceSelectionWith([...w].map((c) => ({ kind: "symbol" as const, latex: c, cls: "ord" as const })));
  }

  /** `<` then `=` is ≤ (see `COMPOSED`), three dots are `\ldots`. */
  private compose(ch: string): boolean {
    const { row, offset } = this.cursor;
    const before = row.children[offset - 1];
    if (before?.kind !== "symbol") return false;
    const dots = ch === "." && before.latex === "." && (row.children[offset - 2] as SymbolAtom | undefined)?.latex === ".";
    const made = dots ? "\\ldots" : COMPOSED[before.latex + ch];
    if (!made) return false;
    const n = dots ? 2 : 1;
    this.change(() => {
      row.children.splice(offset - n, n);
      this.cursor = { row, offset: offset - n };
      this.replaceSelectionWith([{ kind: "symbol", latex: made, cls: classOfCommand(made.slice(1)) }]);
    }, true);
    return true;
  }

  /** `{` in an empty exponent, index, root or part of a fraction groups it, as in LaTeX (nothing
   * drawn); `}` at its end leaves it: to the denominator from a numerator, else after the structure. */
  private typeBrace(ch: string): boolean {
    const { row, offset } = this.cursor;
    const owner = row.parent;
    if (ch === "{" && !row.children.length && (owner?.kind === "scripts" || owner?.kind === "bigop" || owner?.kind === "sqrt" || owner?.kind === "frac")) {
      this.braced.add(row);
      return true;
    }
    if (ch !== "}") return false;
    // the group `}` closes: this place, or one it ends in (the denominator of x^{2/3} ends the exponent)
    let r = row;
    let at = offset;
    while (!this.braced.has(r)) {
      const up = r.parent;
      const rows = up ? childRows(up) : [];
      if (!up || at !== r.children.length || rows[rows.length - 1] !== r) return false;
      const around = up.parent!;
      at = around.children.indexOf(up) + 1;
      r = around;
    }
    if (at !== r.children.length) return false;
    const place = r.parent!;
    const outer = place.parent!;
    this.braced.delete(r);
    this.change(() => (this.cursor = place.kind === "frac" && r === place.num ? { row: place.den, offset: place.den.children.length } : { row: outer, offset: outer.children.indexOf(place) + 1 }));
    return true;
  }

  /** At the end of a place a calculator ends by itself, a character typed goes on after it. An
   * exponent, an index, a denominator or a root holding a number or a name (`2`, `-1`, `n`, `2x`):
   * an operator, a relation, a comma, an opening, `/` or a space ends it, and a letter after a
   * number. A large operator's limit (`i=1`, `x\to0`): a space. Never a place opened with `{`. */
  private leavePlace(ch: string): void {
    const { row, offset } = this.cursor;
    const owner = row.parent;
    const atoms = row.children;
    if (!owner || offset !== atoms.length || !atoms.length || this.braced.has(row)) return;
    const last = atoms[atoms.length - 1];
    // a function waits for its argument (`\frac{\sin}{…}` is never meant)
    if (last.kind === "symbol" && last.cls === "op") return;
    const limit = owner.kind === "bigop" && (row === owner.sub || row === owner.sup);
    const place = owner.kind === "scripts" || (owner.kind === "frac" && row === owner.den) || (owner.kind === "sqrt" && row === owner.body && !owner.index);
    if (limit) {
      if (ch !== " ") return;
    } else {
      if (!place) return;
      const plain = (a: Atom, i: number) => a.kind === "symbol" && ((a.cls === "ord" && /^([A-Za-z0-9.]|\\[A-Za-z]+)$/.test(a.latex)) || (i === 0 && a.latex === "-" && atoms.length > 1));
      if (!atoms.every(plain)) return;
      const number = atoms.every((a) => /^[-0-9.]$/.test((a as SymbolAtom).latex));
      // a letter after a number ends an exponent or an index (x^2y, H_2O), not a denominator (2a) or a root
      if (!/^[-+*=<>,;:(\[/ ]$/.test(ch) && !(number && owner.kind === "scripts" && /^[A-Za-z\\]$/.test(ch))) return;
    }
    const outer = owner.parent!;
    this.change(() => (this.cursor = { row: outer, offset: outer.children.indexOf(owner) + 1 }));
  }

  /** In a large operator's limit: `_` where the cursor already is does nothing, `^` goes to the upper
   * limit and `_` to the lower one (`\sum_{k=1}^n`, `\int_0^1`), not an exponent of what is there. */
  private typeInLimit(ch: string): boolean {
    const owner = this.cursor.row.parent;
    if (owner?.kind !== "bigop" || (ch !== "^" && ch !== "_")) return false;
    const which = ch === "^" ? "sup" : "sub";
    if (owner[which] === this.cursor.row) return !this.cursor.row.children.length;
    this.change(() => {
      const target = owner[which] ?? (owner[which] = this.row(owner));
      this.cursor = { row: target, offset: target.children.length };
    });
    return true;
  }

  /** Typing in a reaction: the pieces build themselves (see `type`). */
  private typeInReaction(ch: string): void {
    const at = this.reactionAtCursor()!;
    const { row, offset } = this.cursor;
    const piece = at.piece;
    const chars = (text: string): AtomSpec[] => [...text].map((c) => ({ kind: "char" as const, ch: c }));
    if (ch === "\n") return;
    // at the end of a species, `->` or `<` (the rest is typed as math): out of it, an arrow begins
    if (piece?.kind === "species") {
      return this.change(() => {
        if (ch === ">") row.children.splice(offset - 1, 1);
        this.afterPiece(piece);
        this.putPiece({ id: this.id(), parent: piece.parent!, kind: "rxtoken", text: ch === ">" ? "->" : "<" });
        const token = this.cursor.row.children[this.cursor.offset - 1] as ReactionToken;
        if (ch === ">") this.toReactionArrow(token);
      });
    }
    // over or under an arrow: characters (mhchem reads them as chemistry or words)
    if (piece?.kind === "rxarrow") {
      if (ch === "]") return this.change(() => this.afterPiece(piece));
      return this.change(() => this.replaceSelectionWith(chars(ch)), true);
    }
    // between pieces: a space is remembered (`[` after it starts a species, not the arrow's conditions)
    const spaced = this.spacedAt?.row === row && this.spacedAt.offset === offset;
    this.spacedAt = null;
    if (ch === " ") return void (this.spacedAt = { row, offset });
    const before = row.children[offset - 1];
    // `^` then a number or `{`: not a gas, the start of an isotope (^{14}_{6}C)
    if (before?.kind === "rxtoken" && before.text === "^" && /^[{0-9]$/.test(ch)) {
      this.change(() => {
        row.children.splice(offset - 1, 1);
        this.cursor = { row, offset: offset - 1 };
        const sp = this.newSpecies(row);
        this.putPiece(sp);
        this.cursor = { row: sp.formula, offset: 0 };
      }, true);
      this.type("^");
      return this.type(ch);
    }
    const scheme = !!at.reaction.scheme;
    const arrows = scheme ? SCHEME_ARROWS : REACTION_ARROWS;
    // an arrow being typed: -> <=> <-> grow from - < =
    if (before?.kind === "rxtoken" && /^[-<=]/.test(before.text) && /[-<=>/]/.test(ch)) {
      const text = before.text + ch;
      if (arrows.some((a) => a.startsWith(text))) {
        return this.change(() => {
          before.text = text;
          // complete, and no longer arrow begins with it: the arrow
          if (arrows.includes(text) && !arrows.some((a) => a !== text && a.startsWith(text))) this.toReactionArrow(before);
        });
      }
    }
    // a pending arrow followed by anything else: the arrow it is, if it is one
    if (before?.kind === "rxtoken" && arrows.includes(before.text) && before.text !== "=") this.change(() => this.toReactionArrow(before));
    // in a scheme, anything but a sign or an arrow starts a structure, the character its first atom
    if (scheme && !/[-<=+\[]/.test(ch)) {
      const latex = `\\chemfig{${LABEL_STOPS.test(ch) ? "" : ch}}`;
      this.change(() => this.putPiece({ id: this.id(), parent: row, kind: "raw", latex }), true);
      const made = this.cursor.row.children[this.cursor.offset - 1] as Raw;
      if (this.graphOf(made)) this.chem = { atom: made, type: "atom", index: 0, fresh: false, row: this.cursor.row, offset: this.cursor.offset };
      return;
    }
    if (ch === "+" || ch === "^" || ch === "-" || ch === "<" || ch === "=")
      return this.change(() => this.putPiece({ id: this.id(), parent: row, kind: "rxtoken", text: ch }));
    if (ch === "[") {
      const prev = row.children[this.cursor.offset - 1];
      // the first [ goes over the arrow, the second under it (right after it: after a space, a species begins)
      if (prev?.kind === "rxarrow" && !spaced) {
        const target = prev.above.children.length ? prev.below : prev.above;
        return this.change(() => (this.cursor = { row: target, offset: target.children.length }));
      }
    }
    // anything else starts a species, typed in it as math
    this.change(() => {
      const sp = this.newSpecies(row);
      this.putPiece(sp);
      this.cursor = { row: sp.formula, offset: 0 };
    }, true);
    this.type(ch);
  }

  private newSpecies(parent: Row): Species {
    const sp: Species = { id: this.id(), parent, kind: "species", formula: null! };
    sp.formula = this.row(sp);
    return sp;
  }

  /** Where new pieces go in the reaction the cursor is in: after the piece it is in; from an arrow's
   * conditions, after what the arrow leads to (a + there is the product's, not the arrow's). */
  private pieceOffset(at: { reaction: Reaction; piece: Species | ReactionArrow | null }): number {
    const body = at.reaction.body;
    if (!at.piece) return this.cursor.offset;
    const i = body.children.indexOf(at.piece);
    const next = body.children[i + 1];
    return at.piece.kind === "rxarrow" && next && next.kind !== "rxarrow" ? i + 2 : i + 1;
  }

  /** Puts a piece of a reaction at the cursor (replacing a selection), the cursor after it. */
  private putPiece(piece: Atom): void {
    if (this.anchor) this.deleteSelection();
    const { row, offset } = this.cursor;
    piece.parent = row;
    row.children.splice(offset, 0, piece);
    this.cursor = { row, offset: offset + 1 };
  }

  /** The arrow of the reaction piece the cursor is on (or a new arrow after it): `->`, `<=>`… */
  setReactionArrow(arrow: string): void {
    const at = this.reactionAtCursor();
    if (!at) return;
    // the arrow the cursor is on, or beside
    const target = this.activeArrow();
    this.change(() => {
      if (target) {
        target.arrow = arrow;
        return;
      }
      const body = at.reaction.body;
      const offset = at.piece ? body.children.indexOf(at.piece) + 1 : this.cursor.offset;
      const a: ReactionArrow = { id: this.id(), parent: body, kind: "rxarrow", arrow, above: null!, below: null! };
      a.above = this.row(a);
      a.below = this.row(a);
      body.children.splice(offset, 0, a);
      this.cursor = { row: body, offset: offset + 1 };
      this.anchor = null;
    });
  }

  /** A new structure after the piece the cursor is on, in a scheme; its atom picked. */
  addSchemeStructure(): void {
    const at = this.reactionAtCursor();
    if (!at?.reaction.scheme) return;
    const body = at.reaction.body;
    const offset = this.pieceOffset(at);
    this.change(() => {
      this.cursor = { row: body, offset };
      this.anchor = null;
      this.putPiece({ id: this.id(), parent: body, kind: "raw", latex: "\\chemfig{}" });
    });
    const made = body.children[offset] as Raw;
    if (this.graphOf(made)) this.chem = { atom: made, type: "atom", index: 0, fresh: true, row: body, offset: offset + 1 };
    this.emit("cursor");
  }

  /** A sign after the piece the cursor is on: `+`, `^` (gas), `v` (precipitate); after `+`, a new species to type. */
  addReactionSign(text: string): void {
    const at = this.reactionAtCursor();
    if (!at) return;
    this.change(() => {
      const body = at.reaction.body;
      let offset = this.pieceOffset(at);
      body.children.splice(offset++, 0, { id: this.id(), parent: body, kind: "rxtoken", text });
      this.cursor = { row: body, offset };
      this.anchor = null;
      if (text === "+" && !at.reaction.scheme) {
        const sp = this.newSpecies(body);
        body.children.splice(offset, 0, sp);
        this.cursor = { row: sp.formula, offset: 0 };
      }
    });
    // in a scheme, a structure to draw after the +
    if (text === "+" && at.reaction.scheme) this.addSchemeStructure();
  }

  /** The cursor over or under the arrow it is on (or beside). */
  enterReactionPlace(place: "above" | "below"): void {
    const p = this.activeArrow();
    if (!p) return;
    this.change(() => {
      this.cursor = { row: p[place], offset: p[place].children.length };
      this.anchor = null;
    });
  }

  /** Removes the reaction piece the cursor is on; the cursor where it was. */
  removeReactionPiece(): void {
    const at = this.reactionAtCursor();
    const p = at?.piece ?? (at && this.cursor.row.children[this.cursor.offset - 1]);
    if (!at || !p) return;
    this.change(() => {
      const body = at.reaction.body;
      let i = body.children.indexOf(p);
      if (i < 0) return;
      // a species takes its + with it: the one after it, or else the one before it
      const plus = (a: Atom | undefined) => a?.kind === "rxtoken" && a.text === "+";
      if (p.kind !== "rxtoken" && plus(body.children[i + 1])) body.children.splice(i, 2);
      else if (p.kind !== "rxtoken" && plus(body.children[i - 1])) body.children.splice(--i, 2);
      else body.children.splice(i, 1);
      this.cursor = { row: body, offset: i };
      this.anchor = null;
    });
  }

  private afterPiece(piece: Atom): void {
    const row = piece.parent!;
    this.cursor = { row, offset: row.children.indexOf(piece) + 1 };
    this.anchor = null;
  }

  private toReactionArrow(token: ReactionToken): void {
    const row = token.parent!;
    const i = row.children.indexOf(token);
    const a: ReactionArrow = { id: this.id(), parent: row, kind: "rxarrow", arrow: token.text, above: null!, below: null! };
    a.above = this.row(a);
    a.below = this.row(a);
    row.children.splice(i, 1, a);
    if (this.cursor.row === row && this.cursor.offset > i) this.cursor = { row, offset: i + 1 };
  }

  /** Types a string, character by character. */
  typeText(text: string): void {
    // plain characters (no structure, no command) join the undo step of those typed before them
    const plain = !this.commandBeingTyped() && /^[A-Za-z0-9.+\-*=<>,;!?' ]+$/.test(text);
    this.change(() => {
      this.batching++;
      try {
        for (const ch of text) this.type(ch);
      } finally {
        this.batching--;
      }
    }, plain || this.inText());
  }

  /** Enter: finishes a command being typed; in a matrix, goes to the cell below, adding a line at the last one. */
  enter(): void {
    const command = this.commandBeingTyped();
    if (command) return this.change(() => void this.finishCommand(command));
    const at = this.matrixAtCursor();
    if (!at) return;
    this.change(() => {
      if (at.i === at.matrix.cells.length - 1) this.insertMatrixRow(at.matrix, at.i + 1);
      // from the end of a line, the start of the next (a matrix typed line by line); else the cell below
      const last = at.j === at.matrix.cells[at.i].length - 1;
      const target = at.matrix.cells[at.i + 1][last ? 0 : at.j];
      this.cursor = { row: target, offset: target.children.length };
      this.anchor = null;
    });
  }

  /** Finishes the command being typed with this name (what a command menu does). */
  completeCommand(name: string): void {
    const command = this.commandBeingTyped();
    if (!command) return;
    this.change(() => {
      command.name = name.replace(/^\\/, "");
      void this.finishCommand(command);
    });
  }

  /** A typed symbol: joins the undo step of the characters typed before it. */
  private typeSymbol(latex: string, cls: AtomClass): void {
    this.change(() => this.replaceSelectionWith([{ kind: "symbol", latex, cls }]), true);
  }

  /** Inserts one symbol at the cursor (replacing the selection): `x`, `\alpha`, `\pm` (class `bin`)… */
  insertSymbol(latex: string, cls?: AtomClass): void {
    const c = cls ?? (latex.startsWith("\\") ? classOfCommand(latex.slice(1)) : (CHAR_CLASS[latex] ?? "ord"));
    this.change(() => this.replaceSelectionWith([{ kind: "symbol", latex, cls: c }]));
  }

  /** Inserts a structure by name (`frac`, `sqrt`, `sum`, `pmatrix`, `vec`, `sup`…; see `TEMPLATES`). */
  insert(name: string): void {
    if (name === "sup" || name === "sub") return this.script(name);
    const snippet = TEMPLATES[name];
    if (!snippet) throw new Error(`context-equation-editor: no template named ${name}`);
    this.change(() => this.insertSpecs(readLatex(snippet, { autoDelimiters: true })));
  }

  /** Inserts LaTeX at the cursor. In a snippet, `#0` is where the selection goes and `#?` an empty
   * place (when it is a whole argument); the cursor goes to the first empty place inserted, or after
   * what was inserted. With `autoDelimiters`, `\\left…\\right` stays plain around small things. */
  insertLatex(latex: string, options: { autoDelimiters?: boolean } = {}): void {
    this.change(() => {
      if (this.inText() && !/[#\\{}^_]/.test(latex)) {
        this.replaceSelectionWith([...latex].map((ch) => ({ kind: "char" as const, ch })));
        return;
      }
      const specs = readLatex(latex, options);
      // a reaction's pieces into the reaction the cursor is in, after the piece it is in
      const at = this.reactionAtCursor();
      // a scheme into a reaction (or a reaction into a scheme): beside it, whole
      if (at && specs.length === 1 && specs[0].kind === "reaction" && !!specs[0].scheme !== !!at.reaction.scheme) {
        const outer = at.reaction.parent!;
        this.cursor = { row: outer, offset: outer.children.indexOf(at.reaction) + 1 };
        this.anchor = null;
      } else if (at) {
        if (specs.length === 1 && specs[0].kind === "reaction") {
          const body = at.reaction.body;
          let offset = this.pieceOffset(at);
          if (!at.piece && this.anchor) {
            this.deleteSelection();
            offset = this.cursor.offset;
          }
          const made = this.adopt(specs[0], body, []) as Reaction;
          for (const p of made.body.children) p.parent = body;
          body.children.splice(offset, 0, ...made.body.children);
          this.cursor = { row: body, offset: offset + made.body.children.length };
          this.anchor = null;
          // an arrow with empty conditions: the cursor over it
          const last = made.body.children[made.body.children.length - 1];
          if (made.body.children.length === 1 && last.kind === "rxarrow" && latex.includes("[")) this.cursor = { row: last.above, offset: last.above.children.length };
          return;
        }
        // other math over or under an arrow: its LaTeX as text (mhchem reads it); in a species, math
        if (at.piece?.kind === "rxarrow") {
          this.replaceSelectionWith([...latex.replace(/#[?0]/g, "")].map((ch) => ({ kind: "char" as const, ch })));
          return;
        }
      }
      this.insertSpecs(specs);
      // a scheme from a palette: its first structure's R (or first atom) picked
      const after = this.cursor.row.children[this.cursor.offset - 1];
      if (after?.kind === "reaction" && after.scheme) this.pickFirstStructure(after);
      // a diagram from a palette: its first object picked, to type over (as a structure's R)
      else if (after?.kind === "diagram" && after.cells[0]?.[0]?.children.length) {
        const first = after.cells[0][0];
        this.anchor = { row: first, offset: 0 };
        this.cursor = { row: first, offset: first.children.length };
      }
      // a reaction or a species from a palette: the cursor at its end, to go on typing it; over its
      // arrow when it is an arrow with conditions to fill
      else if (after?.kind === "reaction") {
        const pieces = after.body.children;
        const last = pieces[pieces.length - 1];
        this.cursor = pieces.length === 1 && last.kind === "rxarrow" && latex.includes("[") ? { row: last.above, offset: 0 } : { row: after.body, offset: pieces.length };
      }
      // a structure from a palette: its first substituent to fill (R), or its first atom, selected
      const made = this.cursor.row.children[this.cursor.offset - 1];
      const g = made?.kind === "raw" && !this.anchor ? this.graphOf(made) : null;
      if (made?.kind === "raw" && g) {
        const r = g.atoms.findIndex((a) => a.label === "R");
        this.chem = { atom: made, type: "atom", index: Math.max(0, r), fresh: true, row: this.cursor.row, offset: this.cursor.offset };
      }
    });
  }

  /** Inserts a block of LaTeX to edit as written (mhchem, chemfig, a diagram): `‸` marks where its
   * editor's cursor starts. The field opens the editor of blocks on it. */
  insertBlock(template: string): void {
    this.change(() => this.placeBlock(template));
  }

  /** Turns a structure into a block of LaTeX (what "edit as LaTeX" does); its editor reads it back when done. */
  toBlock(atom: Atom): Raw | null {
    const row = atom.parent;
    if (!row) return null;
    // the formula as it was: what undo and cancel go back to
    this.blockOrigin = this.snapshot();
    const raw: Raw = { id: this.id(), parent: row, kind: "raw", latex: this.listLatex([atom], false) };
    const i = row.children.indexOf(atom);
    row.children.splice(i, 1, raw);
    // the cursor leaves the atom that went (it may have been inside it)
    this.cursor = { row, offset: i + 1 };
    this.anchor = null;
    return raw;
  }

  /** The block just inserted that wants its LaTeX edited, once. */
  takePendingBlock(): { atom: Raw; caret: number } | null {
    const p = this.pendingBlock;
    this.pendingBlock = null;
    return p && p.atom.parent ? p : null;
  }

  /** The block of LaTeX right before the cursor (or right after it, at a row's start). */
  blockAtCursor(): Raw | null {
    const { row, offset } = this.cursor;
    const before = row.children[offset - 1];
    if (before?.kind === "raw") return before;
    const after = row.children[offset];
    return after?.kind === "raw" ? after : null;
  }

  /** Edits a block's LaTeX: each change shows at once, and all of them are one step of undo. */
  editBlock(atom: Raw, caret = atom.latex.length): BlockSession {
    // a block made from boxes (`toBlock`): undo and cancel go back to the boxes
    const origin = this.blockOrigin;
    this.blockOrigin = null;
    const before = origin ?? this.snapshot();
    const original = atom.latex;
    const editor = this;
    return {
      atom,
      caret,
      update(latex: string) {
        atom.latex = latex;
        editor.emit("content");
      },
      commit() {
        const row = atom.parent;
        if (row && !atom.latex.trim()) {
          const i = row.children.indexOf(atom);
          row.children.splice(i, 1);
          editor.cursor = { row, offset: i };
        } else if (row) {
          // read back: LaTeX the editor can take apart (a diagram) becomes boxes again
          const specs = readLatex(atom.latex);
          const i = row.children.indexOf(atom);
          // over or under an arrow, a piece of the conditions stays LaTeX as written (mhchem reads it)
          if (specs.length === 1 && specs[0].kind !== "raw" && row.parent?.kind !== "rxarrow") {
            const made = editor.adoptInto(row, specs, [], i);
            row.children.splice(i + made.length, 1);
            editor.cursor = { row, offset: i + made.length };
          } else editor.cursor = { row, offset: i + 1 };
        }
        editor.anchor = null;
        if (atom.latex !== original || !atom.latex.trim()) {
          editor.undoStack.push(before);
          editor.redoStack = [];
        }
        editor.typingRun = false;
        editor.emit("content");
      },
      cancel() {
        if (origin) {
          editor.restore(origin);
          editor.emit("content");
          return;
        }
        atom.latex = original;
        // a block that was empty when it opened (\ce typed) goes
        const row = atom.parent;
        if (row && !original.replace(/\\(ce|pu|chemfig|xymatrix)\{\}|\\begin\{(tikzcd|CD)\}\s*\\end\{\2\}|\\schemestart\s*\\schemestop/, "").trim()) {
          const i = row.children.indexOf(atom);
          row.children.splice(i, 1);
          editor.cursor = { row, offset: i };
        }
        editor.emit("content");
      },
    };
  }

  // ---------- diagrams ----------

  /** The arrow whose label the cursor is in, with its diagram. */
  arrowAtCursor(): { diagram: Diagram; arrow: DiagramArrow; index: number } | null {
    // the label the cursor is in, at any depth (the index of f_1 too)
    let row = this.cursor.row;
    while (row.parent && row.parent.kind !== "diagram") row = row.parent.parent!;
    const d = row.parent;
    if (d?.kind !== "diagram") return null;
    const index = d.arrows.findIndex((a) => a.label === row || a.swapLabel === row);
    return index < 0 ? null : { diagram: d, arrow: d.arrows[index], index };
  }

  /** Out of the reaction the cursor is in, after it (Escape). */
  leaveReaction(): void {
    const at = this.reactionAtCursor();
    if (!at) return;
    const outer = at.reaction.parent!;
    this.setCursor({ row: outer, offset: outer.children.indexOf(at.reaction) + 1 });
  }

  /** From an arrow's label back to the cell it leaves (Escape). */
  leaveArrowLabel(): void {
    const at = this.arrowAtCursor();
    if (!at) return;
    const cell = at.diagram.cells[at.arrow.from[0]][at.arrow.from[1]];
    this.setCursor({ row: cell, offset: cell.children.length });
  }

  /** Draws an arrow between two cells of a diagram; the cursor goes to its label. */
  addArrow(d: Diagram, from: [number, number], to: [number, number], style: ArrowStyle = "plain"): void {
    if (from[0] === to[0] && from[1] === to[1]) return;
    this.change(() => {
      const label = this.row(d);
      d.arrows.push({ from: [...from], to: [...to], label, swapLabel: null, labelExtra: "", swapExtra: "", style, bend: null, extra: [] });
      this.cursor = { row: label, offset: 0 };
      this.anchor = null;
    });
  }

  /** From the cell the cursor is in, an arrow to the neighbour on that side (a line or a column is
   * added at the edge); the cursor goes to its label. */
  arrowFromCursor(direction: ArrowDirection, style: ArrowStyle = "plain"): void {
    const at = this.matrixAtCursor();
    if (!at || at.matrix.kind !== "diagram") return;
    const [di, dj] = { up: [-1, 0], down: [1, 0], left: [0, -1], right: [0, 1] }[direction];
    this.arrowToNeighbour(at.matrix, [at.i, at.j], di, dj, style);
  }

  /** An arrow from a cell to its neighbour one step away (`di`, `dj` each -1, 0 or 1: diagonals too);
   * past the edge, a line or a column is added for it. The cursor goes to its label. */
  arrowToNeighbour(d: Diagram, from: [number, number], di: number, dj: number, style: ArrowStyle = "plain"): void {
    if (!di && !dj) return;
    this.change(() => {
      let [i, j] = from;
      if (i + di < 0) {
        this.insertMatrixRow(d, 0);
        i++;
      } else if (i + di >= d.cells.length) this.insertMatrixRow(d, d.cells.length);
      if (j + dj < 0) {
        this.insertMatrixColumn(d, 0);
        j++;
      } else if (j + dj >= d.cells[0].length) this.insertMatrixColumn(d, d.cells[0].length);
      const label = this.row(d);
      d.arrows.push({ from: [i, j], to: [i + di, j + dj], label, swapLabel: null, labelExtra: "", swapExtra: "", style, bend: null, extra: [] });
      this.cursor = { row: label, offset: 0 };
      this.anchor = null;
    });
  }

  /** Puts the cursor in an arrow's label (one is made if it has none): what a click on the arrow does. */
  selectArrow(d: Diagram, index: number): void {
    const a = d.arrows[index];
    if (!a) return;
    this.change(() => {
      const row = a.label ?? a.swapLabel ?? (a.label = this.row(d));
      this.cursor = { row, offset: row.children.length };
      this.anchor = null;
    });
  }

  /** The arrow the cursor is on: its style, its bend, its label's side, or gone. */
  setArrowStyle(style: ArrowStyle): void {
    const at = this.arrowAtCursor();
    if (at) this.change(() => void (at.arrow.style = style));
  }

  setArrowBend(bend: "left" | "right" | null): void {
    const at = this.arrowAtCursor();
    if (at) this.change(() => void (at.arrow.bend = bend));
  }

  /** The label goes to the arrow's other side. */
  swapArrowLabel(): void {
    const at = this.arrowAtCursor();
    if (!at) return;
    this.change(() => {
      const a = at.arrow;
      [a.label, a.swapLabel] = [a.swapLabel, a.label];
      [a.labelExtra, a.swapExtra] = [a.swapExtra, a.labelExtra];
    });
  }

  removeArrow(): void {
    const at = this.arrowAtCursor();
    if (!at) return;
    this.change(() => {
      const d = at.diagram;
      d.arrows.splice(at.index, 1);
      const cell = d.cells[at.arrow.from[0]][at.arrow.from[1]];
      this.cursor = { row: cell, offset: cell.children.length };
      this.anchor = null;
    });
  }

  /** Is this row the cursor's (an empty one shows as a place to fill only there)? */
  hasCursor(row: Row): boolean {
    return this.cursor.row === row;
  }

  // ---------- moving ----------

  /** Right: into the next structure, out at the end of a row. With `extend`, grows the selection by whole atoms. */
  moveRight(extend = false): void {
    this.change(() => (extend ? this.extend(+1) : this.step(+1)));
  }

  moveLeft(extend = false): void {
    this.change(() => (extend ? this.extend(-1) : this.step(-1)));
  }

  /** Up: from a denominator to its numerator, an index to the exponent, a cell to the one above. */
  moveUp(): void {
    this.change(() => this.vertical(-1));
  }

  moveDown(): void {
    this.change(() => this.vertical(+1));
  }

  home(extend = false): void {
    this.change(() => {
      if (extend) this.anchor ??= { ...this.cursor };
      else this.anchor = null;
      this.cursor = { row: this.cursor.row, offset: 0 };
      this.dropEmptySelection();
    });
  }

  end(extend = false): void {
    this.change(() => {
      if (extend) this.anchor ??= { ...this.cursor };
      else this.anchor = null;
      this.cursor = { row: this.cursor.row, offset: this.cursor.row.children.length };
      this.dropEmptySelection();
    });
  }

  /** Tab: the next empty place, else the next cell, else the end of the formula. Shift+Tab: back. */
  tab(back = false): void {
    this.change(() => {
      this.anchor = null;
      const rows = this.rows();
      const here = rows.indexOf(this.cursor.row);
      const order = back ? rows.slice(0, here).reverse() : rows.slice(here + 1);
      // an empty label of an arrow is not a place to fill (an arrow needs none)
      const label = (r: Row) => r.parent?.kind === "diagram" && !r.parent.cells.some((line) => line.includes(r));
      const empty = order.find((r) => r.children.length === 0 && !label(r));
      if (empty) return void (this.cursor = { row: empty, offset: 0 });
      const at = this.matrixAtCursor();
      if (at) {
        const flat = at.matrix.cells.flat();
        const k = flat.indexOf(at.matrix.cells[at.i][at.j]) + (back ? -1 : 1);
        if (k >= 0 && k < flat.length) return void (this.cursor = { row: flat[k], offset: back ? flat[k].children.length : 0 });
      }
      this.cursor = back ? { row: this.root, offset: 0 } : { row: this.root, offset: this.root.children.length };
    });
  }

  /** Puts the cursor somewhere, as a click does; with `extend`, keeps the selection's other end (a drag, Shift+click). */
  setCursor(cursor: Cursor, extend = false): void {
    this.change(() => {
      if (extend) this.anchor ??= { ...this.cursor };
      else this.anchor = null;
      this.cursor = { row: cursor.row, offset: Math.max(0, Math.min(cursor.offset, cursor.row.children.length)) };
      this.dropEmptySelection();
    });
  }

  /** Selects everything. */
  selectAll(): void {
    this.change(() => {
      this.anchor = { row: this.root, offset: 0 };
      this.cursor = { row: this.root, offset: this.root.children.length };
      this.dropEmptySelection();
    });
  }

  /** Selects the atoms `start` to `end` of a row. */
  select(row: Row, start: number, end: number): void {
    this.change(() => {
      this.anchor = { row, offset: start };
      this.cursor = { row, offset: end };
      this.dropEmptySelection();
    });
  }

  // ---------- deleting ----------

  /** Backspace: deletes the selection, or the atom before the cursor; a structure with something
   * inside is entered from its end; at the start of a place, the structure goes if it is empty. */
  backspace(): void {
    if (this.backspaceInStructure()) return;
    this.change(() => {
      if (this.deleteSelection()) return;
      const { row, offset } = this.cursor;
      if (offset > 0) {
        const atom = row.children[offset - 1];
        if (atom.kind === "command" && atom.name.length > 0) return void (atom.name = atom.name.slice(0, -1));
        const rows = childRows(atom);
        const filled = [...rows].reverse().find((r) => r.children.length > 0);
        if (filled) return void (this.cursor = { row: filled, offset: filled.children.length });
        return this.removeAt(row, offset - 1);
      }
      this.leaveOrRemoveOwner(row, -1);
    });
  }

  /** Delete: the selection, or the atom after the cursor (a structure with something inside is entered from its start). */
  deleteForward(): void {
    if (this.chemSelection()) return this.removeChem();
    this.change(() => {
      if (this.deleteSelection()) return;
      const { row, offset } = this.cursor;
      if (offset < row.children.length) {
        const atom = row.children[offset];
        const filled = childRows(atom).find((r) => r.children.length > 0);
        if (filled) return void (this.cursor = { row: filled, offset: 0 });
        return this.removeAt(row, offset);
      }
      this.leaveOrRemoveOwner(row, +1);
    });
  }

  // ---------- clipboard ----------

  /** The selection as plain LaTeX, for the clipboard. */
  copy(): string {
    return this.selectedLatex();
  }

  /** The selection as plain LaTeX, removed from the formula. */
  cut(): string {
    const latex = this.selectedLatex();
    if (latex) this.change(() => void this.deleteSelection());
    return latex;
  }

  /** Pastes LaTeX (or plain text inside `\text{…}`) at the cursor, replacing the selection. */
  paste(text: string): void {
    const latex = text.trim().replace(/^\$\$?([\s\S]*?)\$?\$$/, "$1").replace(/^\\\[([\s\S]*)\\\]$/, "$1").replace(/^\\\(([\s\S]*)\\\)$/, "$1");
    // in a reaction, as from a palette: its pieces join it, other math goes into a piece as text
    if (this.reactionAtCursor()) return this.insertLatex(latex);
    this.change(() => {
      if (this.inText()) return this.replaceSelectionWith([...text].map((ch) => ({ kind: "char" as const, ch })));
      this.insertSpecs(readLatex(latex), { cursorAfter: true });
    });
  }

  // ---------- matrices ----------

  /** Adds a line to the matrix around the cursor, below the cursor's line (above with `before`). */
  addRow(before = false): void {
    const at = this.matrixAtCursor();
    if (!at) return;
    this.change(() => {
      const i = before ? at.i : at.i + 1;
      this.insertMatrixRow(at.matrix, i);
      this.cursor = { row: at.matrix.cells[i][at.j], offset: 0 };
      this.anchor = null;
    });
  }

  /** Adds a column to the matrix around the cursor, right of the cursor's column (left with `before`). */
  addColumn(before = false): void {
    const at = this.matrixAtCursor();
    if (!at) return;
    this.change(() => {
      const j = before ? at.j : at.j + 1;
      this.insertMatrixColumn(at.matrix, j);
      this.cursor = { row: at.matrix.cells[at.i][j], offset: 0 };
      this.anchor = null;
    });
  }

  /** Removes the cursor's line from the matrix around it (the matrix goes with its last line). */
  removeRow(): void {
    const at = this.matrixAtCursor();
    if (!at) return;
    this.change(() => {
      const m = at.matrix;
      if (m.cells.length === 1) return this.removeAtom(m);
      m.cells.splice(at.i, 1);
      if (m.kind === "matrix") m.rowLabels?.splice(at.i, 1);
      if (m.kind === "diagram") this.dropFromDiagram(m, 0, at.i);
      const i = Math.min(at.i, m.cells.length - 1);
      this.cursor = { row: m.cells[i][at.j], offset: 0 };
      this.anchor = null;
    });
  }

  /** Removes the cursor's column from the matrix around it (the matrix goes with its last column). */
  removeColumn(): void {
    const at = this.matrixAtCursor();
    if (!at) return;
    this.change(() => {
      const m = at.matrix;
      if (m.cells[0].length === 1) return this.removeAtom(m);
      for (const line of m.cells) line.splice(at.j, 1);
      if (m.kind === "diagram") this.dropFromDiagram(m, 1, at.j);
      const j = Math.min(at.j, m.cells[0].length - 1);
      this.cursor = { row: m.cells[at.i][j], offset: 0 };
      this.anchor = null;
    });
  }

  // ---------- undo ----------

  canUndo(): boolean {
    return this.undoStack.length > 0;
  }

  canRedo(): boolean {
    return this.redoStack.length > 0;
  }

  undo(): void {
    const state = this.undoStack.pop();
    if (state === undefined) return;
    // what is typed next is a step of its own, not part of the one undone
    this.typingRun = false;
    this.redoStack.push(this.snapshot());
    this.restore(state);
    this.emit("content");
  }

  redo(): void {
    const state = this.redoStack.pop();
    if (state === undefined) return;
    this.typingRun = false;
    this.undoStack.push(this.snapshot());
    this.restore(state);
    this.emit("content");
  }

  /** Runs several commands as one change: one undo step, one notification. */
  batch(f: () => void): void {
    this.change(() => {
      this.batching++;
      try {
        f();
      } finally {
        this.batching--;
      }
    });
  }

  // ---------- internals: changes ----------

  private id(): string {
    return `n${this.next++}`;
  }

  private row(parent: Atom | null): Row {
    return { kind: "row", id: this.id(), children: [], parent };
  }

  private change(f: () => void, typing = false): void {
    if (this.batching) return f();
    const before = this.snapshot();
    const content = this.contentKey();
    f();
    const after = this.snapshot();
    if (after === before) return;
    const contentChanged = this.contentKey() !== content;
    if (contentChanged) {
      // characters typed in a row (less than two seconds apart) join the step of the first
      const now = Date.now();
      const joins = typing && this.typingRun && now - this.typingAt < 2000;
      this.typingRun = typing;
      this.typingAt = now;
      if (!joins && this.undoStack[this.undoStack.length - 1] !== before) this.undoStack.push(before);
      if (this.undoStack.length > 500) this.undoStack.shift();
      this.redoStack = [];
    } else this.typingRun = false;
    this.emit(contentChanged ? "content" : "cursor");
  }

  /** The formula, without the cursor. */
  private contentKey(): string {
    return JSON.stringify(this.listSpec(this.root.children));
  }

  private emit(kind: ChangeKind): void {
    for (const l of this.listeners) l(kind);
  }

  // ---------- internals: building ----------

  /** Makes atoms from specs, into `row` at `at`; `#0` takes `selected`. Returns the atoms made. */
  private adoptInto(row: Row, specs: AtomSpec[], selected: Atom[], at = row.children.length): Atom[] {
    const made: Atom[] = [];
    for (const spec of specs) {
      if (spec.kind === "selection") {
        for (const a of selected) {
          a.parent = row;
          made.push(a);
        }
        selected = [];
        continue;
      }
      made.push(this.adopt(spec, row, selected));
    }
    row.children.splice(at, 0, ...made);
    return made;
  }

  /** One spec made an atom; `#0` (the selection) is placed by `adoptInto`. */
  private adopt(spec: Exclude<AtomSpec, { kind: "selection" }>, parent: Row, selected: Atom[]): Atom {
    const base = { id: this.id(), parent };
    const rowOf = (owner: Atom, specs: AtomSpec[] | null): Row | null => {
      if (!specs) return null;
      const r = this.row(owner);
      this.adoptInto(r, specs, selected);
      return r;
    };
    switch (spec.kind) {
      case "symbol":
        return { ...base, kind: "symbol", latex: spec.latex, cls: spec.cls };
      case "char":
        return { ...base, kind: "char", ch: spec.ch };
      case "raw":
        return { ...base, kind: "raw", latex: spec.latex };
      case "command":
        return { ...base, kind: "command", name: spec.name };
      case "text": {
        const a: TextAtom = { ...base, kind: "text", cmd: spec.cmd, body: null! };
        // the selection inside text becomes its LaTeX, as characters
        const specs = spec.body.flatMap((s): AtomSpec[] => (s.kind === "selection" ? [...this.listLatex(selected, false)].map((ch) => ({ kind: "char" as const, ch })) : [s]));
        if (spec.body.some((s) => s.kind === "selection")) selected = [];
        a.body = rowOf(a, specs)!;
        return a;
      }
      case "frac": {
        const a: Fraction = { ...base, kind: "frac", cmd: spec.cmd, num: null!, den: null! };
        a.num = rowOf(a, spec.num)!;
        a.den = rowOf(a, spec.den)!;
        return a;
      }
      case "sqrt": {
        const a: Root = { ...base, kind: "sqrt", index: null, body: null! };
        a.index = rowOf(a, spec.index);
        a.body = rowOf(a, spec.body)!;
        return a;
      }
      case "scripts": {
        const a: Scripts = { ...base, kind: "scripts", sup: null, sub: null };
        a.sup = rowOf(a, spec.sup);
        a.sub = rowOf(a, spec.sub);
        return a;
      }
      case "delim": {
        const a: Delimited = { ...base, kind: "delim", left: spec.left, right: spec.right, body: null!, auto: spec.auto ?? false };
        a.body = rowOf(a, spec.body)!;
        return a;
      }
      case "bigop": {
        const a: LargeOperator = { ...base, kind: "bigop", op: spec.op, sub: null, sup: null };
        a.sub = rowOf(a, spec.sub);
        a.sup = rowOf(a, spec.sup);
        return a;
      }
      case "accent":
      case "style": {
        const a = { ...base, kind: spec.kind, cmd: spec.cmd, body: null! } as Accent | Styled;
        a.body = rowOf(a, spec.body)!;
        return a;
      }
      case "matrix": {
        const a: Matrix = { ...base, kind: "matrix", env: spec.env, cells: [], ...(spec.rowLabels ? { rowLabels: [...spec.rowLabels] } : {}), ...(spec.spec !== undefined ? { spec: spec.spec } : {}) };
        a.cells = spec.cells.map((line) => line.map((cell) => rowOf(a, cell)!));
        return a;
      }
      case "call": {
        const a: Call = { ...base, kind: "call", name: spec.name, opt: null, args: [] };
        a.opt = rowOf(a, spec.opt);
        a.args = spec.args.map((x) => rowOf(a, x)!);
        return a;
      }
      case "diagram": {
        const a: Diagram = { ...base, kind: "diagram", env: spec.env, ...(spec.options ? { options: spec.options } : {}), cells: [], arrows: [] };
        a.cells = spec.cells.map((line) => line.map((cell) => rowOf(a, cell)!));
        a.arrows = spec.arrows.map((x) => ({ ...x, from: [...x.from] as [number, number], to: [...x.to] as [number, number], extra: [...x.extra], label: rowOf(a, x.label), swapLabel: rowOf(a, x.swapLabel) }));
        return a;
      }
      case "reaction": {
        const a: Reaction = { ...base, kind: "reaction", body: null!, ...(spec.scheme ? { scheme: true } : {}) };
        a.body = this.row(a);
        for (const p of spec.pieces) {
          if (p.kind === "species") {
            const sp = this.newSpecies(a.body);
            this.adoptInto(sp.formula, readLatex(chemToLatex(p.text)), []);
            a.body.children.push(sp);
          } else if (p.kind === "rxarrow") {
            const ar: ReactionArrow = { id: this.id(), parent: a.body, kind: "rxarrow", arrow: p.arrow, above: null!, below: null! };
            // conditions: characters, a command or $…$ whole
            const pieces = (t: string | null): AtomSpec[] => conditionPieces(t ?? "").map((x) => ("ch" in x ? { kind: "char" as const, ch: x.ch } : { kind: "raw" as const, latex: x.latex }));
            ar.above = rowOf(ar, pieces(p.above))!;
            ar.below = rowOf(ar, pieces(p.below))!;
            a.body.children.push(ar);
          } else if (p.kind === "chem") a.body.children.push({ id: this.id(), parent: a.body, kind: "raw", latex: p.latex });
          else a.body.children.push({ id: this.id(), parent: a.body, kind: "rxtoken", text: p.text });
        }
        return a;
      }
    }
  }

  /** Inserts specs at the cursor, the selection going where `#0` is (or replaced, without `#0`). */
  private insertSpecs(specs: AtomSpec[], options: { cursorAfter?: boolean } = {}): void {
    const sel = this.selection();
    let selected: Atom[] = [];
    if (sel) {
      selected = sel.row.children.splice(sel.start, sel.end - sel.start);
      this.cursor = { row: sel.row, offset: sel.start };
      this.anchor = null;
    }
    const { row, offset } = this.cursor;
    // a lone exponent or index joins the one before it
    if (specs.length === 1 && specs[0].kind === "scripts" && !selected.length) {
      const before = row.children[offset - 1];
      const s = specs[0];
      if (before && before.kind === "scripts" && (!s.sup || !before.sup) && (!s.sub || !before.sub)) {
        const made = this.adopt(s, row, []) as Scripts;
        for (const which of ["sup", "sub"] as const) {
          const r = made[which];
          if (r) {
            r.parent = before;
            before[which] = r;
          }
        }
        const first = (made.sup ?? made.sub)!;
        this.cursor = { row: first, offset: first.children.length };
        return;
      }
    }
    const made = this.adoptInto(row, specs, selected, offset);
    this.cursor = { row, offset: offset + made.length };
    if (options.cursorAfter) return;
    // the first empty place inserted (the empty cells of a diagram with content are its layout, not places)
    const isPlace = (r: Row) => {
      const d = r.parent;
      // a reaction's optional places (coefficient, state, conditions) are not places to fill
      if (d?.kind === "species") return r === d.formula;
      if (d?.kind === "rxarrow") return false;
      return !(d?.kind === "diagram" && d.cells.flat().some((c) => c.children.length > 0));
    };
    for (const a of made) {
      const empty = rowsOf(a).find((r) => r.children.length === 0 && isPlace(r));
      if (empty) {
        this.cursor = { row: empty, offset: 0 };
        return;
      }
    }
  }

  private replaceSelectionWith(specs: AtomSpec[]): void {
    this.deleteSelection();
    const { row, offset } = this.cursor;
    const made = this.adoptInto(row, specs, [], offset);
    this.cursor = { row, offset: offset + made.length };
  }

  private finishCommand(command: Command): boolean {
    const row = command.parent!;
    const at = row.children.indexOf(command);
    row.children.splice(at, 1);
    this.cursor = { row, offset: at };
    const name = command.name;
    if (!name) return false;
    if (BLOCKS[name]) {
      this.placeBlock(BLOCKS[name]);
      return true;
    }
    if (DIAGRAMS[name]) {
      this.insertSpecs(readLatex(DIAGRAMS[name]));
      return true;
    }
    // \ce: an empty reaction, the cursor in its row of pieces
    if (STRUCTURED[name]) {
      this.insertSpecs(readLatex(STRUCTURED[name]), { cursorAfter: true });
      const rx = this.cursor.row.children[this.cursor.offset - 1];
      if (rx?.kind === "reaction") this.cursor = { row: rx.body, offset: 0 };
      if (rx?.kind === "reaction" && rx.scheme) this.pickFirstStructure(rx);
      if (rx?.kind === "raw" && this.graphOf(rx)) this.chem = { atom: rx, type: "atom", index: 0, fresh: true, row: this.cursor.row, offset: this.cursor.offset };
      else if (rx?.kind === "raw") this.pendingBlock = { atom: rx, caret: rx.latex.length - 1 };
      return true;
    }
    // typed, a derivative has its two places (what, and by what); other commands as many as they need
    const places = /^(pdv|dv|fdv)$/.test(name) ? 2 : CALLS[name]?.[0] ?? 0;
    const snippet = TEMPLATES[name] ?? (CALLS[name] ? `\\${name}{#0}${"{#?}".repeat(places - 1)}` : undefined);
    if (snippet) this.insertSpecs(readLatex(snippet, { autoDelimiters: true }));
    // a large operator typed alone: with its limits, as the palette inserts it
    else if (LOWER_LIMIT_ONLY.has(name)) this.insertSpecs(readLatex(`\\${name}_{#?}`));
    else this.insertSpecs(readLatex(`\\${name}`), { cursorAfter: true });
    return false;
  }

  private placeBlock(template: string): void {
    const caret = Math.max(0, template.indexOf("‸"));
    this.replaceSelectionWith([{ kind: "raw", latex: template.replace("‸", "") }]);
    const { row, offset } = this.cursor;
    this.pendingBlock = { atom: row.children[offset - 1] as Raw, caret };
  }

  private script(which: ScriptTemplate): void {
    this.change(() => this.addScript(which));
  }

  private addScript(which: ScriptTemplate): void {
    this.deleteSelection();
    const { row, offset } = this.cursor;
    const before = row.children[offset - 1];
    // a large operator takes it as its limit
    if (before && (before.kind === "scripts" || before.kind === "bigop")) {
      const existing = before[which];
      if (existing) return void (this.cursor = { row: existing, offset: existing.children.length });
      const r = this.row(before);
      before[which] = r;
      this.cursor = { row: r, offset: 0 };
      return;
    }
    this.insertSpecs([{ kind: "scripts", sup: which === "sup" ? [] : null, sub: which === "sub" ? [] : null }]);
  }

  /** `/`: the operand before the cursor (back to an operator, a relation or an opening) becomes the numerator. */
  private fractionOfOperand(): void {
    if (this.selection()) return this.insertSpecs(readLatex("\\frac{#0}{#?}"));
    const row = this.cursor.row;
    let offset = this.cursor.offset;
    let start = offset;
    let depth = 0;
    while (start > 0) {
      const a = row.children[start - 1];
      if (a.kind === "symbol" && a.cls === "close") depth++;
      else if (a.kind === "symbol" && a.cls === "open") {
        if (depth === 0) break;
        depth--;
      // a function (\sin, \log) goes with its argument: sin(x)/x is (sin x)/x
      } else if (depth === 0 && (a.kind === "command" || a.kind === "bigop" || (a.kind === "symbol" && a.cls !== "ord" && a.cls !== "op"))) break;
      start--;
    }
    // `(a+b)/`: the parentheses only grouped the numerator
    const only = row.children[start];
    if (this.smartTyping && offset - start === 1 && only.kind === "delim" && only.left === "(" && only.right === ")" && only.body.children.length) {
      const kept = only.body.children;
      for (const a of kept) a.parent = row;
      row.children.splice(start, 1, ...kept);
      offset = start + kept.length;
      this.cursor = { row, offset };
    }
    this.anchor = start < offset ? { row, offset: start } : null;
    this.insertSpecs(readLatex(start < offset ? "\\frac{#0}{#?}" : "\\frac{#?}{#?}"));
  }

  /** `)` typed at the end of a `(`…`)` (or at the end of what ends it, such as a denominator): leaves it. */
  private leaveDelimiter(closer: string): boolean {
    if (this.anchor) return false;
    let row = this.cursor.row;
    let offset = this.cursor.offset;
    while (row.parent) {
      if (offset !== row.children.length) return false;
      const owner = row.parent;
      const outer = owner.parent!;
      const at = outer.children.indexOf(owner);
      if (owner.kind === "delim" && owner.right === closer) {
        // `(…)` typed alone in an exponent, an index, a root or a denominator: it was that place, and `)` leaves it
        const place = outer.parent;
        const fills = place?.kind === "scripts" || place?.kind === "bigop" || (place?.kind === "sqrt" && place.body === outer) || (place?.kind === "frac" && place.den === outer);
        if (this.smartTyping && fills && owner.left === "(" && outer.children.length === 1 && owner.body.children.length) {
          this.change(() => {
            outer.children = owner.body.children;
            for (const a of outer.children) a.parent = outer;
            const around = place.parent!;
            this.cursor = { row: around, offset: around.children.indexOf(place) + 1 };
          });
          return true;
        }
        this.change(() => (this.cursor = { row: outer, offset: at + 1 }));
        return true;
      }
      // the last place of its structure, and the structure last in its row
      const rows = childRows(owner);
      if (rows[rows.length - 1] !== row) return false;
      row = outer;
      offset = at + 1;
    }
    return false;
  }

  private nextCell(): void {
    const at = this.matrixAtCursor();
    if (!at) return;
    this.change(() => {
      const m = at.matrix;
      if (at.j === m.cells[at.i].length - 1) this.insertMatrixColumn(m, at.j + 1);
      const target = m.cells[at.i][at.j + 1];
      this.cursor = { row: target, offset: target.children.length };
      this.anchor = null;
    });
  }

  private insertMatrixRow(m: Matrix | Diagram, i: number): void {
    const width = m.cells[0]?.length ?? 1;
    m.cells.splice(i, 0, Array.from({ length: width }, () => this.row(m)));
    if (m.kind === "matrix") m.rowLabels?.splice(i, 0, null);
    // a diagram's arrows keep their cells
    if (m.kind === "diagram") for (const a of m.arrows) for (const p of [a.from, a.to]) if (p[0] >= i) p[0]++;
  }

  private insertMatrixColumn(m: Matrix | Diagram, j: number): void {
    for (const line of m.cells) line.splice(j, 0, this.row(m));
    if (m.kind === "diagram") for (const a of m.arrows) for (const p of [a.from, a.to]) if (p[1] >= j) p[1]++;
  }

  /** The column, else the line, of cell (i, j) goes when all its cells are empty and no arrow touches
   * it (and the diagram keeps one). Which went, if one did. */
  private dropEmptyLines(d: Diagram, i: number, j: number): "column" | "line" | null {
    const touches = (axis: 0 | 1, k: number) => d.arrows.some((a) => a.from[axis] === k || a.to[axis] === k);
    if (j >= 0 && d.cells[0].length > 1 && d.cells.every((line) => !line[j]?.children.length) && !touches(1, j)) {
      for (const line of d.cells) line.splice(j, 1);
      this.dropFromDiagram(d, 1, j);
      return "column";
    }
    if (d.cells.length > 1 && d.cells[i].every((c) => !c.children.length) && !touches(0, i)) {
      d.cells.splice(i, 1);
      this.dropFromDiagram(d, 0, i);
      return "line";
    }
    return null;
  }

  /** A diagram loses a line or a column: the arrows that touch it go, the others follow their cells. */
  private dropFromDiagram(d: Diagram, axis: 0 | 1, k: number): void {
    d.arrows = d.arrows.filter((a) => a.from[axis] !== k && a.to[axis] !== k);
    for (const a of d.arrows) for (const p of [a.from, a.to]) if (p[axis] > k) p[axis]--;
  }

  // ---------- internals: moving and deleting ----------

  private step(side: 1 | -1): void {
    const sel = this.selection();
    if (sel) {
      this.anchor = null;
      this.cursor = { row: sel.row, offset: side > 0 ? sel.end : sel.start };
      return;
    }
    this.anchor = null;
    const { row, offset } = this.cursor;
    const next = row.children[side > 0 ? offset : offset - 1];
    if (next) {
      const rows = childRows(next);
      // an arrow of a reaction with no conditions: passed over (its places show beside it, to click into)
      const empty = next.kind === "rxarrow" && rows.every((r) => r.children.length === 0);
      if (rows.length && !empty) {
        // into an arrow's conditions: the one that has something (an empty one is passed)
        const places = next.kind === "rxarrow" ? rows.filter((r) => r.children.length) : rows;
        const r = side > 0 ? places[0] : places[places.length - 1];
        this.cursor = { row: r, offset: side > 0 ? 0 : r.children.length };
      } else this.cursor = { row, offset: offset + side };
      return;
    }
    this.leave(row, side);
  }

  /** Shift+arrow: the cursor moves over whole atoms of its row; at the row's end, the selection takes the structure around it. */
  private extend(side: 1 | -1): void {
    this.anchor ??= { ...this.cursor };
    const { row, offset } = this.cursor;
    const target = offset + side;
    if (target >= 0 && target <= row.children.length) this.cursor = { row, offset: target };
    else if (row.parent) {
      const owner = row.parent;
      const outer = owner.parent!;
      const at = outer.children.indexOf(owner);
      // the selection, lifted to this row, takes the whole structure
      this.cursor = { row: outer, offset: side > 0 ? at + 1 : at };
    }
    this.dropEmptySelection();
  }

  private dropEmptySelection(): void {
    if (this.anchor && !this.selection()) this.anchor = null;
  }

  /** Leaves `row` to the next place on that side: the next row of the same structure, or the row around it. */
  private leave(row: Row, side: 1 | -1): void {
    const owner = row.parent;
    if (!owner) return;
    if (owner.kind === "diagram") {
      // from a label: to the cell the arrow leaves (left) or reaches (right)
      const a = owner.arrows.find((x) => x.label === row || x.swapLabel === row);
      if (a) {
        const [i, j] = side > 0 ? a.to : a.from;
        const cell = owner.cells[i][j];
        this.cursor = { row: cell, offset: side > 0 ? 0 : cell.children.length };
        return;
      }
      // from a cell: the next cell, then out
      const cells = owner.cells.flat();
      const k = cells.indexOf(row) + side;
      if (k >= 0 && k < cells.length) {
        this.cursor = { row: cells[k], offset: side > 0 ? 0 : cells[k].children.length };
        return;
      }
      const outer = owner.parent!;
      const at = outer.children.indexOf(owner);
      this.cursor = { row: outer, offset: side > 0 ? at + 1 : at };
      return;
    }
    const rows = childRows(owner);
    const i = rows.indexOf(row) + side;
    if (i >= 0 && i < rows.length) {
      this.cursor = { row: rows[i], offset: side > 0 ? 0 : rows[i].children.length };
      return;
    }
    const outer = owner.parent!;
    const at = outer.children.indexOf(owner);
    this.cursor = { row: outer, offset: side > 0 ? at + 1 : at };
  }

  private vertical(side: 1 | -1): void {
    this.anchor = null;
    for (let row: Row = this.cursor.row; row.parent; row = row.parent.parent!) {
      const owner = row.parent;
      if (owner.kind === "matrix" || owner.kind === "diagram") {
        for (let i = 0; i < owner.cells.length; i++) {
          const j = owner.cells[i].indexOf(row);
          const line = owner.cells[i + side];
          if (j >= 0 && line) {
            this.cursor = { row: line[j], offset: line[j].children.length };
            return;
          }
        }
        continue;
      }
      const order = verticalRows(owner);
      const i = order.indexOf(row);
      const target = order[i + side];
      if (i >= 0 && target) {
        this.cursor = { row: target, offset: side > 0 ? 0 : target.children.length };
        return;
      }
    }
  }

  private deleteSelection(): boolean {
    const sel = this.selection();
    this.anchor = null;
    if (!sel) return false;
    sel.row.children.splice(sel.start, sel.end - sel.start);
    this.cursor = { row: sel.row, offset: sel.start };
    return true;
  }

  private removeAt(row: Row, i: number): void {
    row.children.splice(i, 1);
    this.cursor = { row, offset: i };
  }

  private removeAtom(atom: Atom): void {
    const row = atom.parent!;
    this.removeAt(row, row.children.indexOf(atom));
  }

  /** At the edge of a place: an empty exponent, index or root index goes alone; an empty structure goes whole; else the cursor leaves. */
  private leaveOrRemoveOwner(row: Row, side: 1 | -1): void {
    const owner = row.parent;
    if (!owner) return;
    if (owner.kind === "diagram") {
      const k = owner.arrows.findIndex((x) => x.label === row || x.swapLabel === row);
      // Backspace in an empty label: the arrow goes
      if (k >= 0 && side < 0 && row.children.length === 0) {
        const [a] = owner.arrows.splice(k, 1);
        const cell = owner.cells[a.from[0]][a.from[1]];
        // the empty cell it reached (made for it) goes, with its column and its line, when nothing else is there
        if (this.dropEmptyLines(owner, a.to[0], a.to[1]) === "column") this.dropEmptyLines(owner, a.to[0], -1);
        this.cursor = { row: cell, offset: cell.children.length };
        return;
      }
      if (k >= 0) return this.leave(row, side);
      // a whole diagram goes when it is one empty cell without arrows
      if (owner.cells.flat().every((c) => c.children.length === 0) && owner.arrows.length === 0) {
        const outer = owner.parent!;
        const at = outer.children.indexOf(owner);
        outer.children.splice(at, 1);
        this.cursor = { row: outer, offset: at };
        return;
      }
      // Backspace in an empty cell whose column (or line) is empty, without arrows: it goes
      const i = owner.cells.findIndex((line) => line.includes(row));
      const j = i < 0 ? -1 : owner.cells[i].indexOf(row);
      const dropped = side < 0 && row.children.length === 0 && i >= 0 ? this.dropEmptyLines(owner, i, j) : null;
      if (dropped) {
        // to the cell left of the column that went, or above the line that went
        const cell = dropped === "column" ? owner.cells[i][Math.max(0, j - 1)] : owner.cells[Math.max(0, i - 1)][j];
        this.cursor = { row: cell, offset: cell.children.length };
        return;
      }
      return this.leave(row, side);
    }
    const outer = owner.parent!;
    const at = outer.children.indexOf(owner);
    if (row.children.length === 0 && (owner.kind === "scripts" || owner.kind === "bigop") && (owner.sup === row || owner.sub === row)) {
      if (owner.sup === row) owner.sup = null;
      if (owner.sub === row) owner.sub = null;
      if (owner.kind === "scripts" && !owner.sup && !owner.sub) outer.children.splice(at, 1);
      this.cursor = { row: outer, offset: owner.kind === "scripts" && !owner.sup && !owner.sub ? at : at + 1 };
      return;
    }
    if (row.children.length === 0 && owner.kind === "sqrt" && owner.index === row) {
      owner.index = null;
      this.cursor = { row: owner.body, offset: 0 };
      return;
    }
    if (childRows(owner).every((r) => r.children.length === 0)) {
      outer.children.splice(at, 1);
      this.cursor = { row: outer, offset: at };
      return;
    }
    // Backspace in an empty denominator: the fraction goes, its numerator stays
    if (side < 0 && owner.kind === "frac" && owner.den === row && row.children.length === 0) {
      const kept = owner.num.children;
      for (const a of kept) a.parent = outer;
      outer.children.splice(at, 1, ...kept);
      this.cursor = { row: outer, offset: at + kept.length };
      return;
    }
    // a delimiter or a style around something: Backspace at its start takes it off, keeping what it holds
    if (side < 0 && row.children.length && (owner.kind === "delim" || owner.kind === "style" || owner.kind === "accent" || owner.kind === "text" || owner.kind === "sqrt") && childRows(owner).length === 1) {
      const kept = row.children.splice(0);
      outer.children.splice(at, 1, ...kept);
      for (const a of kept) a.parent = outer;
      this.cursor = { row: outer, offset: at };
      return;
    }
    this.cursor = { row: outer, offset: side > 0 ? at + 1 : at };
  }

  /** From the root to the cursor's row: each level's row and the span the cursor takes in it. */
  private chain(c: Cursor): { row: Row; start: number; end: number }[] {
    const levels: { row: Row; start: number; end: number }[] = [{ row: c.row, start: c.offset, end: c.offset }];
    for (let r = c.row; r.parent; r = r.parent.parent!) {
      const owner = r.parent;
      const outer = owner.parent!;
      const i = outer.children.indexOf(owner);
      levels.unshift({ row: outer, start: i, end: i + 1 });
    }
    return levels;
  }

  // ---------- internals: LaTeX ----------

  /** A row's LaTeX. `bare`: its atoms are marked but not the row around them (a cell of an alignment,
   * where `&= …` must keep the relation's spacing); its box is then where its atoms are. */
  private rowLatex(row: Row, marked: boolean, bare = false): string {
    if (row.children.length === 0) {
      if (!marked) return "";
      // an empty cell of an alignment (the left of `&= …`) is part of the layout, not a place to fill:
      // invisible and without width, still with a box for the cursor
      const owner = row.parent;
      if (bare && owner?.kind === "matrix" && /^(aligned|split|gathered|align\*?|alignat\*?)$/.test(owner.env)) return `\\htmlData{m=${row.id},empty=1}{\\vphantom{1}}`;
      return `\\htmlData{m=${row.id},empty=1}{\\square}`;
    }
    const inner = this.listLatex(row.children, marked);
    return marked && !bare ? `\\htmlData{m=${row.id}}{${inner}}` : inner;
  }

  private listLatex(atoms: Atom[], marked: boolean): string {
    let out = "";
    for (let i = 0; i < atoms.length; i++) {
      // where the locale rewrites a number's point (3.14 is 3,14 in pt-BR), the number is one group:
      // marks between its digits would keep contextEquation from reading it as a number
      if (marked) {
        const run = markedRun(atoms, i, this.decimalComma);
        if (run) {
          out = join(out, `\\htmlData{m=${atoms[i].id}n}{${run.map((a) => (a as SymbolAtom).latex).join("")}}`);
          i += run.length - 1;
          continue;
        }
      }
      const atom = atoms[i];
      // \underbrace{…}_{n}: a mark around it would turn the label below into an index
      const bare = marked && atom.kind === "accent" && /^\\(under|over)brace$/.test(atom.cmd) && atoms[i + 1]?.kind === "scripts";
      // after a command with arguments (\ket{\psi}), a letter keeps a space: macros that end in a
      // command (\rangle) would otherwise take it into their name
      if (atoms[i - 1]?.kind === "call" && atom.kind === "symbol" && /^[A-Za-z]/.test(atom.latex)) out += " ";
      out = join(out, bare ? `${atom.cmd}{${this.rowLatex(atom.body, true)}}` : this.atomLatex(atom, marked, atoms[i - 1], atoms[i + 1]));
    }
    return out;
  }

  private atomLatex(atom: Atom, marked: boolean, before?: Atom, after?: Atom): string {
    const mark = (s: string) => (marked ? `\\htmlData{m=${atom.id}}{${s}}` : s);
    const r = (row: Row) => this.rowLatex(row, marked);
    switch (atom.kind) {
      case "symbol": {
        if (!marked) return atom.latex;
        if (UNMARKED.test(atom.latex)) return atom.latex;
        let cls = atom.cls;
        // a command the editor does not know (\vdot, \grad): its class from contextEquation
        if (cls === "ord" && /^\\[A-Za-z]+$/.test(atom.latex) && !(atom.latex.slice(1) in KNOWN_ORDINARY)) cls = this.rawClass(atom.latex);
        // a decimal comma, where the locale reads one: part of the number, as contextEquation reads it
        if (atom.latex === "," && this.decimalComma && isDigit(before) && isDigit(after)) cls = "ord";
        const wrapper = CLASS_WRAPPER[cls];
        const m = mark(atom.latex);
        return wrapper ? `${wrapper}{${m}}` : m;
      }
      case "char":
        // marked, each character is its own \text, which contextEquation draws as the whole word
        return marked ? mark(`\\text{${atom.ch === " " ? "\\ " : escapeText(atom.ch)}}`) : escapeText(atom.ch);
      case "text":
        if (!marked) return `${atom.cmd}{${atom.body.children.map((c) => (c.kind === "char" ? escapeText(c.ch) : "")).join("")}}`;
        return mark(r(atom.body).replace(/\\text\{/g, `${atom.cmd}{`));
      case "raw": {
        // a block being typed may have a brace open: closed here, so the rest of the formula stays out of it
        const latex = balanced(atom.latex);
        // what acts on the alignment around it cannot be inside a mark
        if (!marked || UNMARKED.test(latex) || hasLooseAmpersand(latex)) return latex;
        const cls = this.rawClass(latex);
        const wrapper = cls === "op" ? "\\mathop" : CLASS_WRAPPER[cls ?? "ord"];
        // a structure: each atom marked inside it, for the field to find (and pick) them
        const g = this.graphOf(atom);
        const inner = g ? `\\chemfig{${markBody(this.chemBody(atom), g, (k) => `${atom.id}a${k}`)}}` : latex;
        // marked `raw`: the field outlines a block of LaTeX under the pointer, to edit as written
        const m = marked ? `\\htmlData{m=${atom.id},raw=1${g ? ",chem=1" : ""}}{${inner}}` : latex;
        return wrapper ? `${wrapper}{${m}}` : m;
      }
      case "command":
        return marked ? mark(`\\mathtt{\\backslash ${atom.name}}`) : "";
      case "frac": {
        const m = mark(`${atom.cmd}{${r(atom.num)}}{${r(atom.den)}}`);
        // \overset{?}{=} is a relation, as its base is: the mark keeps that class
        const base = /^\\(overset|underset|stackrel)$/.test(atom.cmd) && atom.den.children.length === 1 ? atom.den.children[0] : null;
        const wrapper = marked && base?.kind === "symbol" ? CLASS_WRAPPER[base.cls] : null;
        return wrapper ? `${wrapper}{${m}}` : m;
      }
      case "sqrt":
        return mark(atom.index ? `\\sqrt[${r(atom.index)}]{${r(atom.body)}}` : `\\sqrt{${r(atom.body)}}`);
      case "accent":
      case "style":
        // \color{c} is a switch: it and what it colors go in a group of their own
        if (atom.cmd.startsWith("\\color{")) return marked ? mark(`${atom.cmd}${r(atom.body)}`) : `{${atom.cmd}${r(atom.body)}}`;
        return mark(`${atom.cmd}{${r(atom.body)}}`);
      case "bigop":
        return mark(`${atom.op}${atom.sub ? `_{${r(atom.sub)}}` : ""}${atom.sup ? `^{${r(atom.sup)}}` : ""}`);
      case "scripts": {
        // on the atom before it; at the start of a row, after another script or a large operator's
        // limits, on an empty base
        const base = !before || before.kind === "scripts" || before.kind === "bigop" ? "{}" : "";
        return `${base}${atom.sup ? `^{${r(atom.sup)}}` : ""}${atom.sub ? `_{${r(atom.sub)}}` : ""}`;
      }
      case "delim": {
        const left = DELIMITERS[atom.left] ?? { open: atom.left, close: atom.left };
        const right = DELIMITERS[atom.right] ?? { open: atom.right, close: atom.right };
        if (!atom.auto || isTall(atom.body)) return mark(`\\left${atom.left}${spaceAfter(atom.left)}${r(atom.body)}\\right${atom.right}`);
        // small: plain delimiters, which keep TeX's spacing (f(x), \sin(x))
        if (!marked) return `${left.open}${spaceAfter(left.open)}${r(atom.body)}${right.close}`;
        const side = (latex: string, cls: "open" | "close", suffix: string) => (latex ? `\\math${cls}{\\htmlData{m=${atom.id}${suffix}}{${latex}}}` : "");
        return `${side(left.open, "open", "l")}${r(atom.body)}${side(right.close, "close", "r")}`;
      }
      case "reaction": {
        const text = (row: Row) => row.children.map((c) => (c.kind === "char" ? c.ch : c.kind === "raw" ? c.latex : "")).join("");
        // a place: the characters, marked; an empty one shows as a box where the cursor is, and over
        // and under the arrow the cursor is on, to say that conditions go there (and to click into)
        const active = this.activeArrow();
        const place = (row: Row) => {
          const t = text(row);
          if (!marked) return t;
          if (t) return `\\htmlData{m=${row.id}}{${t}}`;
          return this.hasCursor(row) || (active && row.parent === active) ? `\\htmlData{m=${row.id},empty=1}{$\\square$}` : "";
        };
        const piece = (p: Atom): string => {
          if (p.kind === "species") {
            if (!marked) return this.speciesText(p);
            // one mark around the formula's text: marks inside it would change how mhchem draws it
            const formula = p.formula.children.length ? `\\htmlData{m=${p.formula.id}}{${this.speciesText(p)}}` : `\\htmlData{m=${p.formula.id},empty=1}{$\\square$}`;
            return `\\htmlData{m=${p.id}}{${formula}}`;
          }
          if (p.kind === "rxarrow") {
            if (!marked) return arrowText(p.arrow, text(p.above), text(p.below));
            const above = place(p.above);
            const below = place(p.below);
            return `\\htmlData{m=${p.id}}{${above || below ? `${p.arrow}[${above}]${below ? `[${below}]` : ""}` : p.arrow}}`;
          }
          if (p.kind === "rxtoken") return marked ? `\\htmlData{m=${p.id}}{${p.text}}` : p.text;
          return "";
        };
        if (atom.scheme) {
          // chemfig's scheme: a structure's atoms marked inside it (a mark around one would break the
          // scheme), arrows with their conditions, \+
          const item = (p: Atom): string => {
            if (p.kind === "raw") {
              const g = marked ? this.graphOf(p) : null;
              return g ? `\\chemfig{${markBody(this.chemBody(p), g, (k) => `${p.id}a${k}`)}}` : p.latex;
            }
            if (p.kind === "rxarrow") {
              // chemfig writes conditions as text: a formula in them (H2SO4, H_2O) is set as mhchem's
              const chem = (row: Row) => {
                const t = text(row);
                const formula = /[A-Z][a-z]?[0-9]|[_^]/.test(t) && !/[$\\]/.test(t);
                if (!marked) return formula ? `\\ce{${t}}` : t;
                return t ? `\\htmlData{m=${row.id}}{${formula ? `\\ce{${t}}` : t}}` : place(row);
              };
              const above = chem(p.above);
              const below = chem(p.below);
              return `\\arrow{${p.arrow}${above || below ? `[${above}]` : ""}${below ? `[${below}]` : ""}}`;
            }
            if (p.kind === "rxtoken") return p.text === "+" ? "\\+" : "";
            return "";
          };
          const items = atom.body.children.map(item).filter(Boolean).join(" ");
          const latex = items ? `\\schemestart ${items} \\schemestop` : "\\schemestart \\schemestop";
          return marked ? mark(latex) : latex;
        }
        const inner = writeReaction(atom.body.children, piece);
        if (!marked) return `\\ce{${inner}}`;
        const body = inner ? `\\htmlData{m=${atom.body.id}}{${inner}}` : `\\htmlData{m=${atom.body.id},empty=1}{$\\square$}`;
        return mark(`\\ce{${body}}`);
      }
      case "species":
      case "rxarrow":
      case "rxtoken":
        return "";
      case "diagram": {
        // an empty cell an arrow reaches is written {}: a line of empty cells alone would be dropped, and the arrow with it
        const ends = new Set(atom.arrows.flatMap((a) => [a.from, a.to].map(([i, j]) => atom.cells[i]?.[j])));
        const cell = (row: Row) => {
          const hold = ends.has(row) ? "{}" : "";
          if (!marked) return row.children.length ? this.listLatex(row.children, false) : hold;
          if (row.children.length) return this.rowLatex(row, true);
          // an empty cell is a place to fill where the cursor is, and invisible elsewhere
          // (even an empty mark changes tikz-cd's layout: the editor places an empty cell by its neighbours)
          if (this.hasCursor(row)) return `\\htmlData{m=${row.id},empty=1}{\\square}`;
          return this.probing ? `\\htmlData{m=${row.id},empty=1}{{}}` : hold;
        };
        const label = (row: Row) => {
          if (!marked) return this.listLatex(row.children, false);
          if (row.children.length) return this.rowLatex(row, true);
          return this.hasCursor(row) ? `\\htmlData{m=${row.id},empty=1}{\\square}` : "";
        };
        return mark(writeDiagram(atom, cell, label));
      }
      case "matrix": {
        const body = atom.cells.map((line, i) => {
          const label = atom.rowLabels?.[i];
          const end = i < atom.cells.length - 1 || label ? ` \\NR${label ? `[${label}]` : ""}` : "";
          return `\\NC ${line.map((cell) => this.rowLatex(cell, marked, true)).join(" \\NC ")}${end}`;
        }).join(" ").replace(/ {2,}/g, " ");
        return mark(`\\start${atom.env}${atom.spec !== undefined ? `{${atom.spec}}` : ""} ${body}\\stop${atom.env}`);
      }
      case "call": {
        const latex = (m: boolean) => `\\${atom.name}${atom.opt ? `[${this.rowLatex(atom.opt, m)}]` : ""}${atom.args.map((x) => `{${this.rowLatex(x, m)}}`).join("")}`;
        if (!marked) return latex(false);
        // its class is the one contextEquation reads it into (\pdv a fraction, \abs delimited)
        const cls = this.rawClass(latex(false));
        const m = mark(latex(true));
        const wrapper = cls === "op" ? "\\mathop" : CLASS_WRAPPER[cls];
        return wrapper ? `${wrapper}{${m}}` : m;
      }
    }
  }

  /** The class contextEquation reads LaTeX into, asked once per piece of LaTeX. */
  private rawClass(latex: string): AtomClass {
    let cls = this.rawClasses.get(latex);
    if (cls === undefined) {
      try {
        cls = this.classifyRaw ? this.classifyRaw(latex) : "ord";
      } catch {
        cls = "ord";
      }
      this.rawClasses.set(latex, cls);
    }
    return cls;
  }

  // ---------- internals: undo ----------

  private listSpec(atoms: Atom[]): AtomSpec[] {
    // a reaction's pieces sit only in its body, which pieceData saves: no row saved here holds one
    return (atoms as Exclude<Atom, Species | ReactionArrow | ReactionToken>[]).map((a) => this.spec(a));
  }

  private spec(a: Exclude<Atom, Species | ReactionArrow | ReactionToken>): AtomSpec {
    const l = (r: Row | null) => (r ? this.listSpec(r.children) : null);
    switch (a.kind) {
      case "symbol":
        return { kind: "symbol", latex: a.latex, cls: a.cls };
      case "char":
        return { kind: "char", ch: a.ch };
      case "raw":
        return { kind: "raw", latex: a.latex };
      case "command":
        return { kind: "command", name: a.name };
      case "text":
        return { kind: "text", cmd: a.cmd, body: l(a.body)! };
      case "frac":
        return { kind: "frac", cmd: a.cmd, num: l(a.num)!, den: l(a.den)! };
      case "sqrt":
        return { kind: "sqrt", index: l(a.index), body: l(a.body)! };
      case "scripts":
        return { kind: "scripts", sup: l(a.sup), sub: l(a.sub) };
      case "delim":
        return a.auto ? { kind: "delim", left: a.left, right: a.right, body: l(a.body)!, auto: true } : { kind: "delim", left: a.left, right: a.right, body: l(a.body)! };
      case "bigop":
        return { kind: "bigop", op: a.op, sub: l(a.sub), sup: l(a.sup) };
      case "accent":
      case "style":
        return { kind: a.kind, cmd: a.cmd, body: l(a.body)! };
      case "matrix":
        return { kind: "matrix", env: a.env, cells: a.cells.map((line) => line.map((c) => l(c)!)), ...(a.rowLabels?.some(Boolean) ? { rowLabels: [...a.rowLabels] } : {}), ...(a.spec !== undefined ? { spec: a.spec } : {}) };
      case "call":
        return { kind: "call", name: a.name, opt: l(a.opt), args: a.args.map((x) => l(x)!) };
      case "reaction":
        return { kind: "reaction", ...(a.scheme ? { scheme: true } : {}), pieces: a.body.children.map((p) => this.pieceData(p)) };
      case "diagram":
        return { kind: "diagram", env: a.env, ...(a.options ? { options: a.options } : {}), cells: a.cells.map((line) => line.map((c) => l(c)!)), arrows: a.arrows.map((x) => ({ ...x, from: [...x.from] as [number, number], to: [...x.to] as [number, number], extra: [...x.extra], label: l(x.label), swapLabel: l(x.swapLabel) })) };
    }
  }

  /** A species as mhchem writes it: `2 H2O(l)`, `SO4^2-`. */
  private speciesText(p: Species): string {
    return latexToChem(this.listLatex(p.formula.children, false));
  }

  /** A piece of a reaction as data. */
  private pieceData(p: Atom): PieceData {
    const text = (row: Row) => row.children.map((c) => (c.kind === "char" ? c.ch : c.kind === "raw" ? c.latex : "")).join("");
    if (p.kind === "species") return { kind: "species", text: this.speciesText(p) };
    if (p.kind === "rxarrow") return { kind: "rxarrow", arrow: p.arrow, above: text(p.above), below: text(p.below) };
    if (p.kind === "raw") return { kind: "chem", latex: p.latex };
    return { kind: "rxtoken", text: p.kind === "rxtoken" ? p.text : "" };
  }

  private snapshot(): string {
    const s: Snapshot = {
      root: this.listSpec(this.root.children),
      cursor: this.path(this.cursor.row),
      offset: this.cursor.offset,
      anchor: this.anchor ? this.path(this.anchor.row) : null,
      anchorOffset: this.anchor?.offset ?? 0,
    };
    return JSON.stringify(s);
  }

  private restore(state: string): void {
    const s = JSON.parse(state) as Snapshot;
    this.root = this.row(null);
    this.adoptInto(this.root, s.root, []);
    const find = (path: [number, number][]) => {
      let row = this.root;
      for (const [i, j] of path) row = childRows(row.children[i])[j];
      return row;
    };
    this.cursor = { row: find(s.cursor), offset: s.offset };
    this.anchor = s.anchor ? { row: find(s.anchor), offset: s.anchorOffset } : null;
  }

  /** How to reach `row` from the root: for each step, the atom's index and the row's index in it. */
  private path(row: Row): [number, number][] {
    const steps: [number, number][] = [];
    for (let r = row; r.parent; r = r.parent.parent!) {
      const owner = r.parent;
      steps.unshift([owner.parent!.children.indexOf(owner), childRows(owner).indexOf(r)]);
    }
    return steps;
  }
}

/** Commands that act on the alignment around them (mathtools, amsmath): never inside a mark. */
/** The matrices a size can be given to: those that are only a grid with delimiters (or none). */
export const MATRIX_SHAPES = new Set(["matrix", "pmatrix", "bmatrix", "Bmatrix", "vmatrix", "Vmatrix"]);

const WORD_LIST = Object.keys(WORDS);

const UNMARKED = /^\\(MoveEqLeft|shortvdotswithin|vdotswithin|Aboxed|intertext|shortintertext|tag\*?|notag|nonumber|label|hline|noalign)\b/;

/** LaTeX with its braces balanced: a `}` without its `{` dropped, the `}` missing at the end added. */
export function balanced(latex: string): string {
  let depth = 0;
  let out = "";
  for (let i = 0; i < latex.length; i++) {
    const c = latex[i];
    if (c === "\\" && i + 1 < latex.length) {
      out += c + latex[++i];
      continue;
    }
    if (c === "{") depth++;
    else if (c === "}") {
      if (depth === 0) continue;
      depth--;
    }
    out += c;
  }
  // a lone backslash at the end (a command begun) is left out
  if (out.endsWith("\\") && !out.endsWith("\\\\")) out = out.slice(0, -1);
  return out + "}".repeat(depth);
}

/** An `&` of the alignment around (outside braces and environments): such LaTeX cannot be inside a
 * mark. A diagram's or a matrix's own `&` (`\begin{tikzcd}…`, `\xymatrix{…}`) can. */
function hasLooseAmpersand(latex: string): boolean {
  let depth = 0;
  let envs = 0;
  for (let i = 0; i < latex.length; i++) {
    const c = latex[i];
    if (c === "\\") {
      if (latex.startsWith("\\begin", i)) envs++;
      else if (latex.startsWith("\\end", i)) envs--;
      i++;
      continue;
    }
    if (c === "{") depth++;
    else if (c === "}") depth--;
    else if (c === "&" && depth <= 0 && envs <= 0) return true;
  }
  return false;
}

/** Commands known to be ordinary symbols: no need to ask contextEquation. */
const KNOWN_ORDINARY: Record<string, true> = {};
for (const n of "alpha beta gamma delta epsilon varepsilon zeta eta theta vartheta iota kappa varkappa lambda mu nu xi omicron pi varpi rho varrho sigma varsigma tau upsilon phi varphi chi psi omega digamma Gamma Delta Theta Lambda Xi Pi Sigma Upsilon Phi Psi Omega infty partial nabla hbar ell emptyset varnothing forall exists nexists neg prime cdots ldots vdots ddots aleph beth gimel daleth angle triangle square".split(" ")) KNOWN_ORDINARY[n] = true;

/** An atom's rows, in the order the cursor visits them from left to right. */
export function childRows(atom: Atom): Row[] {
  switch (atom.kind) {
    case "frac":
      return [atom.num, atom.den];
    case "sqrt":
      return atom.index ? [atom.index, atom.body] : [atom.body];
    case "scripts":
      return [atom.sup, atom.sub].filter((r): r is Row => r !== null);
    case "bigop":
      return [atom.sub, atom.sup].filter((r): r is Row => r !== null);
    case "delim":
    case "accent":
    case "style":
    case "text":
      return [atom.body];
    case "matrix":
      return atom.cells.flat();
    case "call":
      return atom.opt ? [atom.opt, ...atom.args] : atom.args;
    case "diagram":
      return [...atom.cells.flat(), ...atom.arrows.flatMap((a) => [a.label, a.swapLabel].filter((r): r is Row => r !== null))];
    case "reaction":
      return [atom.body];
    case "species":
      return [atom.formula];
    case "rxarrow":
      return [atom.above, atom.below];
    default:
      return [];
  }
}

/** All rows inside an atom, deep, in document order. */
function rowsOf(atom: Atom): Row[] {
  return childRows(atom).flatMap((r) => [r, ...r.children.flatMap(rowsOf)]);
}

/** An atom's rows from top to bottom, for up and down. */
function verticalRows(atom: Atom): Row[] {
  switch (atom.kind) {
    case "call":
      return /^(pdv|dv|fdv)$/.test(atom.name) ? atom.args : [];
    case "rxarrow":
      return [atom.above, atom.below];
    case "frac":
      return [atom.num, atom.den];
    case "scripts":
    case "bigop":
      return [atom.sup, atom.sub].filter((r): r is Row => r !== null);
    case "sqrt":
      return atom.index ? [atom.index, atom.body] : [];
    default:
      return [];
  }
}

/** Something in the row is taller than a line: delimiters around it grow. */
function isTall(row: Row): boolean {
  return row.children.some((a) => a.kind === "frac" || a.kind === "bigop" || a.kind === "matrix" || a.kind === "diagram" || (a.kind === "delim" && isTall(a.body)) || (a.kind === "raw" && /\\(begin|frac|dfrac|sum|int|prod)\b/.test(a.latex)) || ((a.kind === "sqrt" || a.kind === "style" || a.kind === "accent") && childRows(a).some(isTall)));
}

/** Atoms that must be marked as one group, from `atoms[i]`: primes (`''` is one double prime), and a
 * number with a script after it (`10^x`: the exponent is on the whole number) or, where the locale
 * rewrites its point (`decimalComma`), with a point. None if `atoms[i]` starts none. */
export function markedRun(atoms: Atom[], i: number, decimalComma: boolean): Atom[] | null {
  const prime = (a: Atom | undefined) => !!a && a.kind === "symbol" && a.latex === "'";
  if (prime(atoms[i]) && !prime(atoms[i - 1])) {
    let j = i;
    while (prime(atoms[j + 1])) j++;
    return j > i ? atoms.slice(i, j + 1) : null;
  }
  const run = numberRun(atoms, i);
  if (!run || run.length < 2) return null;
  const scripted = atoms[i + run.length]?.kind === "scripts";
  const point = decimalComma && run.some((a) => a.kind === "symbol" && a.latex === ".");
  return scripted || point ? run : null;
}

/** The number starting at `atoms[i]` (digits, and points or commas between digits); none if `atoms[i]` does not start one. */
export function numberRun(atoms: Atom[], i: number): Atom[] | null {
  if (!isDigit(atoms[i]) || isDigit(atoms[i - 1]) || (isSeparator(atoms[i - 1]) && isDigit(atoms[i - 2]))) return null;
  let j = i;
  while (isDigit(atoms[j + 1]) || (isSeparator(atoms[j + 1]) && isDigit(atoms[j + 2]))) j++;
  return atoms.slice(i, j + 1);
}

function isSeparator(a: Atom | undefined): boolean {
  return !!a && a.kind === "symbol" && (a.latex === "." || a.latex === ",");
}

function isDigit(a: Atom | undefined): boolean {
  return !!a && a.kind === "symbol" && /^[0-9]$/.test(a.latex);
}

/** A delimiter command needs a space before a letter (`\langle x`). */
function spaceAfter(d: string): string {
  return /\\[A-Za-z]+$/.test(d) ? " " : "";
}

/** Two pieces of LaTeX one after the other, with a space only where a control word would swallow a letter. */
function join(a: string, b: string): string {
  if (!a) return b;
  return /\\[A-Za-z]+$/.test(a) && /^[A-Za-z]/.test(b) ? `${a} ${b}` : a + b;
}
