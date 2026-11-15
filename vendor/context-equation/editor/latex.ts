// Reads LaTeX into the editor's structures. What it does not know stays as written, in a raw atom
// that is drawn as is and can be moved and deleted whole. In a snippet (a palette's insert), `#?` is
// an empty place and `#0` where the selection goes.

import { readDiagram, type ArrowData, type DiagramEnv } from "./diagram.ts";
import { readReaction, readScheme } from "./reaction.ts";
import {
  ACCENTS,
  CALLS,
  CHAR_CLASS,
  FRACTIONS,
  LARGE_OPERATORS,
  MATRICES,
  SPACES,
  STYLES,
  TEXTS,
  classOfCommand,
  type AtomClass,
} from "./symbols.ts";

/** An atom as data: no id, no parent. What the reader makes and the clipboard and undo keep. */
export type AtomSpec =
  | { kind: "symbol"; latex: string; cls: AtomClass }
  | { kind: "char"; ch: string }
  | { kind: "text"; cmd: string; body: AtomSpec[] }
  | { kind: "frac"; cmd: string; num: AtomSpec[]; den: AtomSpec[] }
  | { kind: "sqrt"; index: AtomSpec[] | null; body: AtomSpec[] }
  | { kind: "scripts"; sup: AtomSpec[] | null; sub: AtomSpec[] | null }
  /** `auto`: plain delimiters while what they hold is small, `\left…\right` when it is tall (typed ones); else always `\left…\right` (read ones). */
  | { kind: "delim"; left: string; right: string; body: AtomSpec[]; auto?: boolean }
  | { kind: "bigop"; op: string; sub: AtomSpec[] | null; sup: AtomSpec[] | null }
  | { kind: "accent"; cmd: string; body: AtomSpec[] }
  | { kind: "style"; cmd: string; body: AtomSpec[] }
  /** `spec`: an array's columns (`{cc|c}`), as written. */
  | { kind: "matrix"; env: string; cells: AtomSpec[][][]; rowLabels?: (string | null)[]; spec?: string }
  /** A command whose arguments are math (`\pdv{f}{x}`, `\ket{\psi}`), each a place; `opt` its `[…]`. */
  | { kind: "call"; name: string; opt: AtomSpec[] | null; args: AtomSpec[][] }
  | { kind: "raw"; latex: string }
  /** A reaction (mhchem's \ce): pieces, their texts as characters. */
  | { kind: "reaction"; scheme?: boolean; pieces: ({ kind: "species"; text: string } | { kind: "rxarrow"; arrow: string; above: string | null; below: string | null } | { kind: "rxtoken"; text: string } | { kind: "chem"; latex: string })[] }
  /** A commutative diagram: cells and arrows, labels as atoms. */
  | { kind: "diagram"; env: DiagramEnv; options?: string; cells: AtomSpec[][][]; arrows: ArrowData<AtomSpec[]>[] }
  | { kind: "command"; name: string }
  /** `#0` in a snippet: where the selection goes. */
  | { kind: "selection" };

class Reader {
  private pos = 0;
  private readonly src: string;
  private readonly auto: boolean;
  constructor(src: string, auto = false) {
    this.src = src;
    this.auto = auto;
  }

  private peek(n = 0): string {
    return this.src[this.pos + n] ?? "";
  }

  private atEnd(): boolean {
    return this.pos >= this.src.length;
  }

  private skipSpaces(): void {
    for (;;) {
      const c = this.peek();
      if (c === " " || c === "\n" || c === "\t" || c === "\r") this.pos++;
      else if (c === "%") while (!this.atEnd() && this.peek() !== "\n") this.pos++;
      else return;
    }
  }

  /** `\name` (letters) or `\x` (one other character), after the backslash. */
  private commandName(): string {
    const start = this.pos;
    if (/[A-Za-z]/.test(this.peek())) {
      while (/[A-Za-z]/.test(this.peek())) this.pos++;
      return this.src.slice(start, this.pos);
    }
    this.pos++;
    return this.src.slice(start, this.pos);
  }

  /** The text up to the brace that closes the one just read, kept as written. */
  private rawGroup(open = "{", close = "}"): string {
    let depth = 1;
    const start = this.pos;
    while (!this.atEnd()) {
      const c = this.peek();
      if (c === "\\") {
        this.pos += 2;
        continue;
      }
      if (c === open) depth++;
      if (c === close && --depth === 0) {
        const s = this.src.slice(start, this.pos);
        this.pos++;
        return s;
      }
      this.pos++;
    }
    return this.src.slice(start);
  }

  /** Skips a stray closing brace; false if there is none. */
  skipBrace(): boolean {
    if (this.peek() !== "}") return false;
    this.pos++;
    return true;
  }

  /** A list of atoms up to the end, a closing brace, or one of `stops` (`&`, `\\`, `\right`, `\end`). */
  list(stops: string[] = []): AtomSpec[] {
    const out: AtomSpec[] = [];
    for (;;) {
      this.skipSpaces();
      if (this.atEnd() || this.peek() === "}") return out;
      if (stops.includes("&") && this.peek() === "&") return out;
      if (this.peek() === "\\") {
        const save = this.pos;
        this.pos++;
        const name = this.commandName();
        this.pos = save;
        if ((name === "\\" && stops.includes("\\\\")) || (name === "right" && stops.includes("\\right")) || (name === "end" && stops.includes("\\end")) || stops.includes(`\\${name}`)) return out;
      }
      this.step(out);
    }
  }

  /** One argument: a group, or a single token. */
  private argument(): AtomSpec[] {
    this.skipSpaces();
    if (this.peek() === "{") {
      this.pos++;
      const l = this.list();
      if (this.peek() === "}") this.pos++;
      return l;
    }
    if (this.atEnd()) return [];
    const out: AtomSpec[] = [];
    this.atom(out);
    return out;
  }

  /** Reads one thing and the scripts after it into `out`. */
  private step(out: AtomSpec[]): void {
    const c = this.peek();
    if (c === "^" || c === "_") {
      this.pos++;
      this.script(out, c === "^" ? "sup" : "sub");
      return;
    }
    if (c === "{") {
      // a group adds its atoms in place; an empty one stays, as the base of a script ({}_{a}, R^{a}{}_{b})
      this.pos++;
      const inner = this.list();
      if (this.peek() === "}") this.pos++;
      if (inner.length) out.push(...inner);
      else out.push({ kind: "symbol", latex: "{}", cls: "ord" });
      return;
    }
    this.atom(out);
  }

  private script(out: AtomSpec[], which: "sup" | "sub"): void {
    const arg = this.argument();
    const last = out[out.length - 1];
    if (last && last.kind === "bigop" && last[which] === null) {
      last[which] = arg;
      return;
    }
    if (last && last.kind === "scripts" && last[which] === null) {
      last[which] = arg;
      return;
    }
    out.push({ kind: "scripts", sup: which === "sup" ? arg : null, sub: which === "sub" ? arg : null });
  }

  /** One atom (no scripts) into `out`. */
  private atom(out: AtomSpec[]): void {
    const c = this.peek();
    if (c === "#" && (this.peek(1) === "?" || this.peek(1) === "0")) {
      this.pos += 2;
      if (this.peek(-1) === "0") out.push({ kind: "selection" });
      return;
    }
    if (c === "\\") {
      this.pos++;
      this.command(this.commandName(), out);
      return;
    }
    this.pos++;
    if (c === "~") return void out.push({ kind: "symbol", latex: "~", cls: "ord" });
    if (c === "&" || c === "$") return;
    if (c === "'") return void out.push({ kind: "symbol", latex: "'", cls: "ord" });
    out.push({ kind: "symbol", latex: c, cls: CHAR_CLASS[c] ?? "ord" });
  }

  private command(name: string, out: AtomSpec[]): void {
    if (name.startsWith("start") && MATRICES.has(name.slice(5))) return void out.push(this.matrix(name.slice(5), true));
    if (FRACTIONS.has(name)) {
      const num = this.argument();
      const den = this.argument();
      out.push({ kind: "frac", cmd: `\\${name}`, num, den });
      return;
    }
    if (name === "sqrt") {
      this.skipSpaces();
      let index: AtomSpec[] | null = null;
      if (this.peek() === "[") {
        this.pos++;
        index = new Reader(this.rawGroup("[", "]"), this.auto).list();
      }
      out.push({ kind: "sqrt", index, body: this.argument() });
      return;
    }
    if (LARGE_OPERATORS.has(name)) {
      out.push({ kind: "bigop", op: `\\${name}`, sub: null, sup: null });
      this.skipSpaces();
      // \limits and \nolimits: contextEquation's defaults are kept
      for (const w of ["\\limits", "\\nolimits"]) if (this.src.startsWith(w, this.pos) && !/[A-Za-z]/.test(this.src[this.pos + w.length] ?? "")) this.pos += w.length;
      return;
    }
    if (name === "left") {
      const left = this.delimiter();
      const body = this.list(["\\right"]);
      let right = ".";
      if (this.src.startsWith("\\right", this.pos)) {
        this.pos += 6;
        right = this.delimiter();
      }
      out.push(this.auto ? { kind: "delim", left, right, body, auto: true } : { kind: "delim", left, right, body });
      return;
    }
    if (name === "begin") {
      this.skipSpaces();
      const save = this.pos;
      const env = this.peek() === "{" ? (this.pos++, this.rawGroup()) : "";
      if (MATRICES.has(env)) return void out.push(this.matrix(env));
      // an array: its columns as written (the lines of \hline stay in their cells), its cells edited
      if (env === "array") {
        this.skipSpaces();
        const spec = this.peek() === "{" ? (this.pos++, this.rawGroup()) : "";
        return void out.push({ ...(this.matrix("array") as { kind: "matrix"; env: string; cells: AtomSpec[][][] }), spec });
      }
      // anything else stays as written, up to its \end; a diagram the editor can edit becomes one
      const end = this.src.indexOf(`\\end{${env}}`, this.pos);
      const stop = end < 0 ? this.src.length : end + `\\end{${env}}`.length;
      const latex = `\\begin${this.src.slice(save, stop)}`;
      this.pos = stop;
      out.push((env === "tikzcd" || env === "CD" ? diagramSpec(latex, this.auto) : null) ?? { kind: "raw", latex });
      return;
    }
    if (TEXTS.has(name)) {
      this.skipSpaces();
      if (this.peek() !== "{") return void out.push({ kind: "raw", latex: `\\${name}` });
      this.pos++;
      const raw = this.rawGroup();
      const chars = readText(raw);
      out.push(chars ? { kind: "text", cmd: `\\${name}`, body: chars } : { kind: "raw", latex: `\\${name}{${raw}}` });
      return;
    }
    // an operator name is one symbol: contextEquation reads its argument as letters
    if (name === "operatorname") {
      const star = this.peek() === "*" ? (this.pos++, "*") : "";
      this.skipSpaces();
      const arg = this.peek() === "{" ? (this.pos++, this.rawGroup()) : "";
      // with a star, its scripts are limits, as a large operator's
      if (star) return void out.push({ kind: "bigop", op: `\\operatorname*{${arg}}`, sub: null, sup: null });
      return void out.push({ kind: "symbol", latex: `\\operatorname{${arg}}`, cls: "op" });
    }
    // \not before a relation is one symbol with it
    if (name === "not") {
      this.skipSpaces();
      let next = "";
      if (this.peek() === "\\") {
        this.pos++;
        next = `\\${this.commandName()}`;
      } else if (!this.atEnd()) next = this.src[this.pos++];
      return void out.push({ kind: "symbol", latex: `\\not${/^\\[A-Za-z]/.test(next) ? "" : " "}${next}`, cls: "rel" });
    }
    // \color{c} colors the rest of its group; \textcolor{c}{x}, its argument
    if (name === "color" || name === "textcolor") {
      this.skipSpaces();
      const color = this.peek() === "{" ? (this.pos++, this.rawGroup()) : "";
      const body = name === "color" ? this.list(["&", "\\\\", "\\right", "\\end"]) : this.argument();
      return void out.push({ kind: "style", cmd: `\\${name}{${color}}`, body });
    }
    // a reaction the editor can take apart becomes pieces; any other stays a block, as written
    if (name === "ce") {
      this.skipSpaces();
      if (this.peek() === "{") {
        const start = this.pos;
        this.pos++;
        const inner = this.rawGroup();
        const pieces = /^\s*#[?0]?\s*$/.test(inner) ? [] : readReaction(inner);
        out.push(pieces ? { kind: "reaction", pieces } : { kind: "raw", latex: `\\ce${this.src.slice(start, this.pos)}` });
        return;
      }
    }
    if (name === "xymatrix") {
      this.skipSpaces();
      if (this.peek() === "{") {
        const start = this.pos;
        this.pos++;
        this.rawGroup();
        const latex = `\\xymatrix${this.src.slice(start, this.pos)}`;
        out.push(diagramSpec(latex, this.auto) ?? { kind: "raw", latex });
        return;
      }
    }
    // a scheme of chemfig is one block, as written
    if (name === "schemestart") {
      const start = this.pos;
      const end = this.src.indexOf("\\schemestop", this.pos);
      let stop = end < 0 ? this.src.length : end + "\\schemestop".length;
      // the curved arrows of \chemmove point into the scheme: they go with it
      const after = /^\s*\\chemmove(\[[^\]]*\])?/.exec(this.src.slice(stop));
      if (after) {
        this.pos = stop + after[0].length;
        if (this.peek() === "{") {
          this.pos++;
          this.rawGroup();
        }
        stop = this.pos;
      }
      // compounds, arrows and + only: a scheme of pieces; anything else (\chemmove, names) one block
      const inner = end < 0 ? null : this.src.slice(start, end);
      const pieces = !after && inner !== null ? readScheme(inner) : null;
      out.push(pieces ? { kind: "reaction", scheme: true, pieces } : { kind: "raw", latex: `\\schemestart${this.src.slice(start, stop)}` });
      this.pos = stop;
      return;
    }
    // a command with math arguments (\pdv{f}{x}, \xrightarrow[below]{above}): each argument a place
    const call = CALLS[name] ?? (ACCENTS.has(name) && name.startsWith("x") && this.peek() === "[" ? [1, 1] : null);
    if (call) {
      const save = this.pos;
      const opt = this.peek() === "[" ? (this.pos++, new Reader(this.rawGroup("[", "]"), this.auto).list()) : null;
      const args: AtomSpec[][] = [];
      while (args.length < call[1] && this.peek() === "{") {
        this.pos++;
        args.push(new Reader(this.rawGroup(), this.auto).list());
      }
      if (args.length >= call[0]) return void out.push({ kind: "call", name, opt, args });
      this.pos = save;
    }
    if (ACCENTS.has(name)) return void out.push({ kind: "accent", cmd: `\\${name}`, body: this.argument() });
    if (STYLES.has(name)) return void out.push({ kind: "style", cmd: `\\${name}`, body: this.argument() });
    if (SPACES.has(name)) return void out.push({ kind: "symbol", latex: `\\${name}`, cls: "ord" });
    if (name === "\\") return void out.push({ kind: "raw", latex: "\\\\" });
    if (/^[{}|#$%&_]$/.test(name)) return void out.push({ kind: "symbol", latex: `\\${name}`, cls: classOfCommand(name) });
    // a starred command the editor does not know (\dv*, \abs*): with what follows, as written
    if (this.peek() === "*") {
      this.pos++;
      const args = this.rawArguments();
      // \dv*xt: two single-token arguments
      const tokens = args ? "" : this.rawTokens(2);
      out.push({ kind: "raw", latex: `\\${name}*${args}${tokens}` });
      return;
    }
    // an unknown command right before an argument keeps its arguments, as written
    if (!(name in KNOWN_NO_ARGUMENT) && (this.peek() === "{" || this.peek() === "[")) {
      out.push({ kind: "raw", latex: `\\${name}${this.rawArguments()}` });
      return;
    }
    out.push({ kind: "symbol", latex: `\\${name}`, cls: classOfCommand(name) });
  }

  /** The `{…}` and `[…]` right after a command, as written. */
  private rawArguments(): string {
    let latex = "";
    while (this.peek() === "{" || this.peek() === "[") {
      const open = this.peek();
      const close = open === "{" ? "}" : "]";
      this.pos++;
      latex += `${open}${this.rawGroup(open, close)}${close}`;
    }
    return latex;
  }

  /** Up to `n` single tokens (a letter, a digit or a command), as written. */
  private rawTokens(n: number): string {
    let out = "";
    for (let i = 0; i < n; i++) {
      this.skipSpaces();
      if (this.atEnd() || /[}&^_]/.test(this.peek())) break;
      if (this.peek() === "\\") {
        const start = this.pos;
        this.pos++;
        this.commandName();
        out += this.src.slice(start, this.pos);
      } else {
        const c = this.src[this.pos++];
        // a letter right after a command name would join it: \alpha t is not \alphat
        out += /\\[A-Za-z]+$/.test(out) && /[A-Za-z]/.test(c) ? ` ${c}` : c;
      }
    }
    return out;
  }

  private delimiter(): string {
    this.skipSpaces();
    if (this.peek() === "\\") {
      this.pos++;
      return `\\${this.commandName()}`;
    }
    const c = this.peek();
    this.pos++;
    return c || ".";
  }

  private matrix(env: string, context = false): AtomSpec {
    const cells: AtomSpec[][][] = [[]];
    const rowLabels: (string | null)[] = [];
    const column = context ? "\\NC" : "&";
    const row = context ? "\\NR" : "\\\\";
    const end = context ? `\\stop${env}` : "\\end";
    let contextRowStart = context;
    for (;;) {
      this.skipSpaces();
      if (contextRowStart && this.src.startsWith(column, this.pos)) {
        this.pos += column.length;
        contextRowStart = false;
        continue;
      }
      const cell = this.list([column, row, end]);
      cells[cells.length - 1].push(cell);
      contextRowStart = false;
      this.skipSpaces();
      if (this.src.startsWith(column, this.pos)) {
        this.pos += column.length;
        continue;
      }
      if (this.src.startsWith(row, this.pos)) {
        this.pos += row.length;
        // Context row labels are metadata, not formula content. Keep them with their row.
        if (this.peek() === "[") {
          this.pos++;
          const value = this.rawGroup("[", "]");
          if (context) rowLabels[rowLabels.length] = value;
        }
        if (context) rowLabels.push(null);
        cells.push([]);
        contextRowStart = context;
        continue;
      }
      if (this.src.startsWith(end, this.pos)) {
        this.pos += end.length;
        this.skipSpaces();
        if (!context && this.peek() === "{") {
          this.pos++;
          this.rawGroup();
        }
      }
      break;
    }
    // a trailing \\ leaves an empty last line
    const last = cells[cells.length - 1];
    if (cells.length > 1 && last.length === 1 && last[0].length === 0) {
      cells.pop();
      if (context) rowLabels.pop();
    }
    const width = Math.max(...cells.map((r) => r.length));
    for (const r of cells) while (r.length < width) r.push([]);
    while (context && rowLabels.length < cells.length) rowLabels.push(null);
    return { kind: "matrix", env, cells, ...(context ? { rowLabels } : {}) };
  }
}

/** A diagram as atoms, or null if the editor cannot edit it (it stays a block of LaTeX). */
function diagramSpec(latex: string, auto: boolean): AtomSpec | null {
  const d = readDiagram(latex);
  if (!d) return null;
  const read = (s: string) => new Reader(s, auto).list();
  return {
    kind: "diagram",
    env: d.env,
    ...(d.options ? { options: d.options } : {}),
    cells: d.cells.map((line) => line.map(read)),
    arrows: d.arrows.map((a) => ({ ...a, label: a.label === null ? null : read(a.label), swapLabel: a.swapLabel === null ? null : read(a.swapLabel), extra: [...a.extra] })),
  };
}

/** Commands that take no argument, even right before a brace (`\alpha{}` is α then a group). */
const KNOWN_NO_ARGUMENT: Record<string, true> = {};
for (const n of "alpha beta gamma delta epsilon varepsilon zeta eta theta vartheta iota kappa lambda mu nu xi pi varpi rho varrho sigma varsigma tau upsilon phi varphi chi psi omega Gamma Delta Theta Lambda Xi Pi Sigma Upsilon Phi Psi Omega infty partial nabla hbar ell emptyset varnothing forall exists nexists neg prime cdots ldots vdots ddots dots".split(" ")) KNOWN_NO_ARGUMENT[n] = true;

/** The characters of a `\text{…}` argument, or null if it holds math or a command the editor does not know. */
function readText(raw: string): AtomSpec[] | null {
  const out: AtomSpec[] = [];
  for (let i = 0; i < raw.length; i++) {
    const c = raw[i];
    if (c === "#" && (raw[i + 1] === "0" || raw[i + 1] === "?")) {
      if (raw[i + 1] === "0") out.push({ kind: "selection" });
      i++;
      continue;
    }
    if (c === "$") return null;
    if (c === "\\") {
      const rest = raw.slice(i + 1);
      const m = /^(textbackslash|[{}#$%&_ ]|\^\{\}|~\{\})(\{\})?/.exec(rest);
      if (!m) return null;
      const name = m[1];
      out.push({ kind: "char", ch: name === "textbackslash" ? "\\" : name === "^{}" ? "^" : name === "~{}" ? "~" : name });
      i += m[0].length;
      continue;
    }
    if (c === "{" || c === "}") continue;
    out.push({ kind: "char", ch: c === "~" ? " " : c });
  }
  return out;
}

/** Reads LaTeX (or a snippet with `#?` and `#0`) into atoms. With `autoDelimiters`, `\left…\right`
 * pairs grow only when what they hold is tall (what the editor's own buttons and keys insert). */
export function readLatex(latex: string, options: { autoDelimiters?: boolean } = {}): AtomSpec[] {
  const r = new Reader(latex, options.autoDelimiters ?? false);
  const out: AtomSpec[] = [];
  // a stray closing brace is skipped, not the end of everything
  for (;;) {
    out.push(...r.list());
    if (!r.skipBrace()) return out;
  }
}
