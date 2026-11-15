// Commutative diagrams as data: a grid of cells and the arrows between them, read from tikz-cd,
// amscd (`CD`) and `\xymatrix`, and written back in the same notation (or in tikz-cd, when the
// diagram grows past what its notation can say). What the editor does not know of an arrow (a
// `shift left`, a label's `very near start`) stays as written.

/** How an arrow is drawn. */
export type ArrowStyle = "plain" | "dashed" | "dotted" | "hook" | "twohead" | "double" | "equal" | "mapsto" | "squiggly";

export const ARROW_STYLES: ArrowStyle[] = ["plain", "dashed", "dotted", "hook", "twohead", "double", "equal", "mapsto", "squiggly"];

/** An arrow, `T` being how its labels are kept (LaTeX while reading, rows in the editor). */
export interface ArrowData<T> {
  from: [number, number];
  to: [number, number];
  /** The label on the arrow's usual side (above a rightward arrow), and on the other side. */
  label: T | null;
  swapLabel: T | null;
  /** Options of each label kept as written (`description`, `very near start`). */
  labelExtra: string;
  swapExtra: string;
  style: ArrowStyle;
  bend: "left" | "right" | null;
  /** The arrow's options the editor does not know (`shift left`, `phantom`), as written. */
  extra: string[];
}

export type DiagramEnv = "tikzcd" | "CD" | "xymatrix";

export interface DiagramData<T> {
  env: DiagramEnv;
  /** tikz-cd's options for the whole diagram (`[row sep=small]`), kept as written. */
  options?: string;
  cells: T[][];
  arrows: ArrowData<T>[];
}

const TIKZ_STYLE: Record<string, ArrowStyle> = { dashed: "dashed", dotted: "dotted", hook: "hook", "two heads": "twohead", Rightarrow: "double", equal: "equal", "maps to": "mapsto", squiggly: "squiggly" };
const STYLE_TIKZ: Record<ArrowStyle, string> = { plain: "", dashed: "dashed", dotted: "dotted", hook: "hook", twohead: "two heads", double: "Rightarrow", equal: "equal", mapsto: "maps to", squiggly: "squiggly" };
const XY_STYLE: Record<string, ArrowStyle> = { "->": "plain", "-->": "dashed", ".>": "dotted", "^{(}->": "hook", "->>": "twohead", "=>": "double", "=": "equal", "|->": "mapsto", "~>": "squiggly" };
const STYLE_XY: Record<ArrowStyle, string> = { plain: "", dashed: "-->", dotted: ".>", hook: "^{(}->", twohead: "->>", double: "=>", equal: "=", mapsto: "|->", squiggly: "~>" };

// ---------- splitting ----------

/** Splits `src` at the separators found outside braces, brackets and quotes. */
function splitTop(src: string, separators: string[]): { part: string; sep: string }[] {
  const out: { part: string; sep: string }[] = [];
  let depth = 0;
  let quote = false;
  let start = 0;
  for (let i = 0; i < src.length; i++) {
    const c = src[i];
    if (c === "\\") {
      // a separator may itself start with a backslash (\\)
      const sep = separators.find((s) => src.startsWith(s, i));
      if (sep && depth === 0 && !quote) {
        out.push({ part: src.slice(start, i), sep });
        i += sep.length - 1;
        start = i + 1;
        continue;
      }
      i++;
      continue;
    }
    if (c === '"') quote = !quote;
    else if (!quote && (c === "{" || c === "[")) depth++;
    else if (!quote && (c === "}" || c === "]")) depth--;
    else if (depth === 0 && !quote) {
      const sep = separators.find((s) => src.startsWith(s, i));
      if (sep) {
        out.push({ part: src.slice(start, i), sep });
        i += sep.length - 1;
        start = i + 1;
      }
    }
  }
  out.push({ part: src.slice(start), sep: "" });
  return out;
}

/** Rows of cells, split at `&` and `\\`. */
function grid(body: string): string[][] {
  const rows: string[][] = [[]];
  for (const { part, sep } of splitTop(body, ["\\\\", "&"])) {
    rows[rows.length - 1].push(part.trim());
    if (sep === "\\\\") rows.push([]);
  }
  // a trailing \\ leaves an empty last line
  const last = rows[rows.length - 1];
  if (rows.length > 1 && last.length === 1 && last[0] === "") rows.pop();
  const width = Math.max(...rows.map((r) => r.length));
  for (const r of rows) while (r.length < width) r.push("");
  return rows;
}

/** The text of a group that starts at `src[i]` (`{` or `[`), and where it ends. */
function group(src: string, i: number): { text: string; end: number } | null {
  const open = src[i];
  const close = open === "{" ? "}" : open === "[" ? "]" : "";
  if (!close) return null;
  let depth = 0;
  let quote = false;
  for (let j = i; j < src.length; j++) {
    const c = src[j];
    if (c === "\\") {
      j++;
      continue;
    }
    if (c === '"' && open === "[") quote = !quote;
    if (quote) continue;
    if (c === open) depth++;
    else if (c === close && --depth === 0) return { text: src.slice(i + 1, j), end: j + 1 };
  }
  return null;
}

/** The offset (rows, columns) of a tikz-cd or xymatrix direction such as `drr`. */
function direction(dir: string): [number, number] | null {
  if (!/^[udlr]+$/.test(dir)) return null;
  let di = 0;
  let dj = 0;
  for (const c of dir) {
    if (c === "u") di--;
    else if (c === "d") di++;
    else if (c === "l") dj--;
    else dj++;
  }
  return [di, dj];
}

function directionText(from: [number, number], to: [number, number]): string {
  const di = to[0] - from[0];
  const dj = to[1] - from[1];
  return (di < 0 ? "u" : "d").repeat(Math.abs(di)) + (dj < 0 ? "l" : "r").repeat(Math.abs(dj));
}

// ---------- reading ----------

/** Reads a diagram, or null if something in it is beyond what the editor can edit (it then stays a block of LaTeX). */
export function readDiagram(latex: string): DiagramData<string> | null {
  const s = latex.trim();
  let m = /^\\begin\{tikzcd\}(\[[^\]]*\])?([\s\S]*)\\end\{tikzcd\}$/.exec(s);
  if (m) {
    const d = readTikz(m[2]);
    return d && m[1] ? { ...d, options: m[1] } : d;
  }
  m = /^\\begin\{CD\}([\s\S]*)\\end\{CD\}$/.exec(s);
  if (m) return readCD(m[1]);
  m = /^\\xymatrix\s*\{([\s\S]*)\}$/.exec(s);
  if (m) return readXy(m[1]);
  return null;
}

function emptyArrow(from: [number, number], to: [number, number]): ArrowData<string> {
  return { from, to, label: null, swapLabel: null, labelExtra: "", swapExtra: "", style: "plain", bend: null, extra: [] };
}

function readTikz(body: string): DiagramData<string> | null {
  const raw = grid(body);
  const arrows: ArrowData<string>[] = [];
  const cells: string[][] = [];
  for (let i = 0; i < raw.length; i++) {
    cells.push([]);
    for (let j = 0; j < raw[i].length; j++) {
      let text = raw[i][j];
      let content = "";
      // the arrows of a cell come out of it; the rest is its content
      for (;;) {
        const at = text.search(/\\(arrow|ar)(?![A-Za-z])/);
        if (at < 0) break;
        content += text.slice(0, at);
        let k = at + (text.startsWith("\\arrow", at) ? 6 : 3);
        while (text[k] === " ") k++;
        const g = text[k] === "[" ? group(text, k) : null;
        if (!g) return null;
        const arrow = tikzArrow(g.text, [i, j]);
        if (!arrow) return null;
        arrows.push(arrow);
        text = text.slice(g.end);
      }
      cells[i].push((content + text).trim());
    }
  }
  if (!arrows.every((a) => a.to[0] >= 0 && a.to[1] >= 0 && a.to[0] < cells.length && a.to[1] < cells[0].length)) return null;
  return { env: "tikzcd", cells, arrows };
}

function tikzArrow(options: string, from: [number, number]): ArrowData<string> | null {
  const parts = splitTop(options, [","]).map((p) => p.part.trim()).filter(Boolean);
  const dirIndex = parts.findIndex((p) => direction(p));
  if (dirIndex < 0) return null;
  const d = direction(parts[dirIndex])!;
  const a = emptyArrow(from, [from[0] + d[0], from[1] + d[1]]);
  parts.splice(dirIndex, 1);
  for (const p of parts) {
    if (p.startsWith('"')) {
      // "label", "label"', "label" description
      const end = p.indexOf('"', 1);
      if (end < 0) return null;
      const text = p.slice(1, end);
      let rest = p.slice(end + 1).trim();
      const swap = rest.startsWith("'");
      if (swap) rest = rest.slice(1).trim();
      if (swap ? a.swapLabel !== null : a.label !== null) {
        a.extra.push(p);
        continue;
      }
      if (swap) {
        a.swapLabel = text;
        a.swapExtra = rest;
      } else {
        a.label = text;
        a.labelExtra = rest;
      }
    } else if (TIKZ_STYLE[p] && a.style === "plain") a.style = TIKZ_STYLE[p];
    else if (p === "bend left" || p === "bend right") a.bend = p === "bend left" ? "left" : "right";
    else a.extra.push(p);
  }
  return a;
}

function readCD(body: string): DiagramData<string> | null {
  const lines = splitTop(body, ["\\\\"]).map((l) => l.part.trim());
  if (lines.length && !lines[lines.length - 1]) lines.pop();
  const cells: string[][] = [];
  const arrows: ArrowData<string>[] = [];
  // lines alternate: objects with horizontal arrows, then a line of vertical arrows
  for (let n = 0; n < lines.length; n++) {
    const tokens = cdTokens(lines[n]);
    if (!tokens) return null;
    if (n % 2 === 0) {
      const row: string[] = [];
      let text = "";
      for (const t of tokens) {
        if (typeof t === "string") {
          text += t;
          continue;
        }
        row.push(text.trim());
        text = "";
        const i = cells.length;
        const j = row.length - 1;
        if (t.kind === ">" || t.kind === "=") arrows.push(cdArrow([i, j], [i, j + 1], t));
        else if (t.kind === "<") arrows.push(cdArrow([i, j + 1], [i, j], t));
        else if (t.kind !== ".") return null;
      }
      row.push(text.trim());
      cells.push(row);
    } else {
      const i = cells.length - 1;
      let j = 0;
      for (const t of tokens) {
        if (typeof t === "string") {
          if (t.trim()) return null;
          continue;
        }
        if (t.kind === "V" || t.kind === "|") arrows.push(cdArrow([i, j], [i + 1, j], t));
        else if (t.kind === "A") arrows.push(cdArrow([i + 1, j], [i, j], t));
        else if (t.kind !== ".") return null;
        j++;
      }
    }
  }
  const width = Math.max(...cells.map((r) => r.length));
  for (const r of cells) while (r.length < width) r.push("");
  return { env: "CD", cells, arrows };
}

type CdToken = { kind: string; first: string; second: string };

/** The text and the `@…` arrows of a line of amscd. */
function cdTokens(line: string): (string | CdToken)[] | null {
  const out: (string | CdToken)[] = [];
  let text = "";
  for (let i = 0; i < line.length; i++) {
    const c = line[i];
    if (c === "\\") {
      text += c + (line[i + 1] ?? "");
      i++;
      continue;
    }
    if (c === "{") {
      const g = group(line, i);
      if (!g) return null;
      text += line.slice(i, g.end);
      i = g.end - 1;
      continue;
    }
    if (c !== "@") {
      text += c;
      continue;
    }
    const kind = line[i + 1];
    if (text) out.push(text);
    text = "";
    if (kind === "=" || kind === "|" || kind === ".") {
      out.push({ kind, first: "", second: "" });
      i += 1;
      continue;
    }
    if (!">V<A".includes(kind)) return null;
    // @>first>second>: two labels, each up to the same character
    const parts: string[] = [];
    let j = i + 2;
    for (let n = 0; n < 2; n++) {
      let depth = 0;
      let k = j;
      for (; k < line.length; k++) {
        if (line[k] === "\\") {
          k++;
          continue;
        }
        if (line[k] === "{") depth++;
        else if (line[k] === "}") depth--;
        else if (line[k] === kind && depth === 0) break;
      }
      if (k >= line.length) return null;
      parts.push(line.slice(j, k));
      j = k + 1;
    }
    out.push({ kind, first: parts[0], second: parts[1] });
    i = j - 1;
  }
  if (text) out.push(text);
  return out;
}

function cdArrow(from: [number, number], to: [number, number], t: CdToken): ArrowData<string> {
  const a = emptyArrow(from, to);
  if (t.kind === "=" || t.kind === "|") a.style = "equal";
  // amscd: above, then below (left, then right for vertical arrows). tikz-cd's usual side is the left
  // of the direction of travel: above a rightward arrow, below a leftward one, right of a downward
  // one, left of an upward one; the other side is the swapped label
  const swapped = t.kind === "<" || t.kind === "V";
  const first = t.first.trim() || null;
  const second = t.second.trim() || null;
  if (swapped) {
    a.swapLabel = first;
    a.label = second;
  } else {
    a.label = first;
    a.swapLabel = second;
  }
  return a;
}

function readXy(body: string): DiagramData<string> | null {
  const raw = grid(body);
  const arrows: ArrowData<string>[] = [];
  const cells: string[][] = [];
  for (let i = 0; i < raw.length; i++) {
    cells.push([]);
    for (let j = 0; j < raw[i].length; j++) {
      let text = raw[i][j];
      let content = "";
      for (;;) {
        const at = text.search(/\\(ar|xymatrix|rtwocell|ltwocell|uppertwocell|lowertwocell|compositemap|twocell)(?![A-Za-z])/);
        if (at < 0) break;
        if (!text.startsWith("\\ar", at) || /^\\ar[A-Za-z]/.test(text.slice(at))) return null;
        content += text.slice(0, at);
        const parsed = xyArrow(text.slice(at + 3), [i, j]);
        if (!parsed) return null;
        arrows.push(parsed.arrow);
        text = parsed.rest;
      }
      cells[i].push((content + text).trim());
    }
  }
  if (!arrows.every((a) => a.to[0] >= 0 && a.to[1] >= 0 && a.to[0] < cells.length && a.to[1] < cells[0].length)) return null;
  return { env: "xymatrix", cells, arrows };
}

function xyArrow(src: string, from: [number, number]): { arrow: ArrowData<string>; rest: string } | null {
  let s = src.trimStart();
  let style: ArrowStyle = "plain";
  let bend: "left" | "right" | null = null;
  // @{style} and @/^/ or @/_/
  for (;;) {
    if (s.startsWith("@{")) {
      const g = group(s, 1);
      if (!g || !(g.text in XY_STYLE)) return null;
      style = XY_STYLE[g.text];
      s = s.slice(g.end).trimStart();
    } else if (s.startsWith("@/^/") || s.startsWith("@/_/")) {
      bend = s[2] === "^" ? "left" : "right";
      s = s.slice(4).trimStart();
    } else break;
  }
  if (s[0] !== "[") return null;
  const g = group(s, 0);
  const d = g && direction(g.text);
  if (!g || !d) return null;
  s = s.slice(g.end);
  const a = emptyArrow(from, [from[0] + d[0], from[1] + d[1]]);
  a.style = style;
  a.bend = bend;
  // ^{label} above, _{label} below
  for (;;) {
    const t = s.trimStart();
    if (t[0] !== "^" && t[0] !== "_") break;
    let label: string;
    let k = 1;
    while (t[k] === " ") k++;
    if (t[k] === "{") {
      const lg = group(t, k);
      if (!lg) return null;
      label = lg.text;
      k = lg.end;
    } else if (t[k] === "\\") {
      const m = /^\\[A-Za-z]+|^\\./.exec(t.slice(k))!;
      label = m[0];
      k += m[0].length;
    } else {
      label = t[k] ?? "";
      k++;
    }
    if (t[0] === "^") a.label = label;
    else a.swapLabel = label;
    s = t.slice(k);
  }
  if (/^\s*\|/.test(s)) return null;
  return { arrow: a, rest: s };
}

// ---------- writing ----------

/** What a notation can draw: amscd only straight neighbours, plain or equal, without bends or extras. */
export function fitsEnv(d: DiagramData<unknown>): DiagramEnv {
  if (d.env === "CD") {
    const ok = d.arrows.every((a) => {
      const di = Math.abs(a.to[0] - a.from[0]);
      const dj = Math.abs(a.to[1] - a.from[1]);
      return di + dj === 1 && (a.style === "plain" || a.style === "equal") && !a.bend && !a.extra.length && !a.labelExtra && !a.swapExtra;
    });
    // and at most one arrow between two neighbours, as amscd has one place for it
    const places = new Set(d.arrows.map((a) => [Math.min(a.from[0], a.to[0]), Math.min(a.from[1], a.to[1]), a.from[0] === a.to[0] ? "h" : "v"].join()));
    return ok && places.size === d.arrows.length ? "CD" : "tikzcd";
  }
  if (d.env === "xymatrix") return d.arrows.every((a) => !a.extra.length && !a.labelExtra && !a.swapExtra) ? "xymatrix" : "tikzcd";
  return "tikzcd";
}

/** Writes a diagram, its cells and labels already written as LaTeX (`cell`, `label`). */
export function writeDiagram<T>(d: DiagramData<T>, cell: (c: T, i: number, j: number) => string, label: (l: T) => string): string {
  const env = fitsEnv(d);
  const rows = d.cells;
  if (env === "CD") return writeCD(d, cell, label);
  const lines = rows.map((line, i) =>
    line
      .map((c, j) => {
        const content = cell(c, i, j);
        const out = d.arrows.filter((a) => a.from[0] === i && a.from[1] === j).map((a) => (env === "xymatrix" ? xyText(a, label) : tikzText(a, label)));
        return [content, ...out].filter(Boolean).join(" ");
      })
      .join(" & ")
      .replace(/ {2,}/g, " ")
      .trim(),
  );
  return env === "xymatrix" ? `\\xymatrix{${lines.join(" \\\\ ")}}` : `\\begin{tikzcd}${d.options ?? ""}${lines.join(" \\\\ ")}\\end{tikzcd}`;
}

function tikzText<T>(a: ArrowData<T>, label: (l: T) => string): string {
  const opts = [directionText(a.from, a.to)];
  const lbl = (t: T | null, swap: boolean, extra: string) => {
    if (t === null) return;
    const text = label(t);
    if (!text) return;
    opts.push(`"${text}"${swap ? "'" : ""}${extra ? ` ${extra}` : ""}`);
  };
  lbl(a.label, false, a.labelExtra);
  lbl(a.swapLabel, true, a.swapExtra);
  if (a.style !== "plain") opts.push(STYLE_TIKZ[a.style]);
  if (a.bend) opts.push(`bend ${a.bend}`);
  opts.push(...a.extra);
  return `\\arrow[${opts.join(", ")}]`;
}

function xyText<T>(a: ArrowData<T>, label: (l: T) => string): string {
  let s = "\\ar";
  if (a.style !== "plain") s += `@{${STYLE_XY[a.style]}}`;
  if (a.bend) s += a.bend === "left" ? "@/^/" : "@/_/";
  s += `[${directionText(a.from, a.to)}]`;
  const up = a.label === null ? "" : label(a.label);
  const down = a.swapLabel === null ? "" : label(a.swapLabel);
  if (up) s += `^{${up}}`;
  if (down) s += `_{${down}}`;
  return s;
}

function writeCD<T>(d: DiagramData<T>, cell: (c: T, i: number, j: number) => string, label: (l: T) => string): string {
  const L = (t: T | null) => (t === null ? "" : label(t));
  const lines: string[] = [];
  const height = d.cells.length;
  const width = d.cells[0]?.length ?? 0;
  for (let i = 0; i < height; i++) {
    let line = "";
    for (let j = 0; j < width; j++) {
      line += ` ${cell(d.cells[i][j], i, j)} `;
      if (j === width - 1) break;
      const a = d.arrows.find((x) => x.from[0] === i && x.to[0] === i && Math.min(x.from[1], x.to[1]) === j);
      if (!a) line += "@.";
      else if (a.style === "equal") line += "@=";
      else if (a.to[1] > a.from[1]) line += `@>${L(a.label)}>${L(a.swapLabel)}>`;
      else line += `@<${L(a.swapLabel)}<${L(a.label)}<`;
    }
    lines.push(line.trim());
    if (i === height - 1) break;
    const verticals: string[] = [];
    for (let j = 0; j < width; j++) {
      const a = d.arrows.find((x) => x.from[1] === j && x.to[1] === j && Math.min(x.from[0], x.to[0]) === i);
      if (!a) verticals.push("@.");
      else if (a.style === "equal") verticals.push("@|");
      else if (a.to[0] > a.from[0]) verticals.push(`@V${L(a.swapLabel)}V${L(a.label)}V`);
      else verticals.push(`@A${L(a.label)}A${L(a.swapLabel)}A`);
    }
    lines.push(verticals.join(" "));
  }
  return `\\begin{CD} ${lines.join(" \\\\ ")} \\end{CD}`;
}
