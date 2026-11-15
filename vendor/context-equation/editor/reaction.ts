// Reactions (mhchem's \ce) as pieces: species (each a formula edited as math: H_2O, SO_4^{2-}), arrows
// (with what goes over and under them) and signs (+, ↑, ↓), read from the text of a \ce and written back.

/** A piece of a reaction, its texts as written. */
export type PieceData =
  | { kind: "species"; text: string }
  | { kind: "rxarrow"; arrow: string; above: string | null; below: string | null }
  | { kind: "rxtoken"; text: string }
  /** A compound of a scheme: a structure, its LaTeX. */
  | { kind: "chem"; latex: string };

/** chemfig's arrows the editor writes in a scheme. */
export const SCHEME_ARROWS = ["<=>", "<->", "->", "<-", "-/>"];

/** mhchem's arrows, the longest first (so `<=>>` is not read as `<=>`). */
export const REACTION_ARROWS = ["<=>>", "<<=>", "<-->", "<=>", "<->", "->", "<-", "="];
const NUMBER = /^(\d+(?:[.,]\d+)?|\d+\/\d+)$/;
const SIGNS = new Set(["+", "^", "v"]);

/** Splits at spaces outside braces, brackets and `$…$`. */
function tokens(src: string): string[] {
  const out: string[] = [];
  let depth = 0;
  let math = false;
  let cur = "";
  for (let i = 0; i < src.length; i++) {
    const c = src[i];
    if (c === "\\") {
      cur += c + (src[i + 1] ?? "");
      i++;
      continue;
    }
    if (c === "$") math = !math;
    else if (!math && (c === "{" || c === "[")) depth++;
    else if (!math && (c === "}" || c === "]")) depth--;
    if (/\s/.test(c) && depth === 0 && !math) {
      if (cur) out.push(cur);
      cur = "";
      continue;
    }
    cur += c;
  }
  if (cur) out.push(cur);
  return out;
}

/** The text of a bracket group at `s[i]`, and where it ends. */
function bracket(s: string, i: number): { text: string; end: number } | null {
  if (s[i] !== "[") return null;
  let depth = 0;
  for (let j = i; j < s.length; j++) {
    if (s[j] === "\\") {
      j++;
      continue;
    }
    if (s[j] === "[" || s[j] === "{") depth++;
    else if (s[j] === "]" || s[j] === "}") {
      depth--;
      if (depth === 0 && s[j] === "]") return { text: s.slice(i + 1, j), end: j + 1 };
    }
  }
  return null;
}

/** Reads the text of a \ce into pieces; null when the editor cannot take it apart (it stays a block). */
export function readReaction(src: string): PieceData[] | null {
  if (/[&]|\\\\/.test(src)) return null;
  const out: PieceData[] = [];
  const ts = tokens(src);
  for (let k = 0; k < ts.length; k++) {
    const t = ts[k];
    const arrow = REACTION_ARROWS.find((a) => t.startsWith(a));
    if (arrow && (t.length === arrow.length || t[arrow.length] === "[")) {
      let rest = t.slice(arrow.length);
      const a = bracket(rest, 0);
      let above: string | null = null;
      let below: string | null = null;
      if (a) {
        above = a.text;
        rest = rest.slice(a.end);
        const b = bracket(rest, 0);
        if (b) {
          below = b.text;
          rest = rest.slice(b.end);
        }
      }
      if (rest) return null;
      out.push({ kind: "rxarrow", arrow, above, below });
      continue;
    }
    if (SIGNS.has(t)) {
      out.push({ kind: "rxtoken", text: t });
      continue;
    }
    // a coefficient written apart: 5 O2
    if (NUMBER.test(t) && ts[k + 1] && !SIGNS.has(ts[k + 1]) && !REACTION_ARROWS.some((x) => ts[k + 1].startsWith(x))) {
      out.push(species(t, ts[k + 1]));
      k++;
      continue;
    }
    // a coefficient written on: 2H2O
    const m = /^(\d+)([A-Z(\[].*)$/.exec(t);
    if (m) {
      out.push(species(m[1], m[2]));
      continue;
    }
    out.push(species("", t));
  }
  return out;
}

/** The group `{…}` at `s[i]`, and where it ends. */
function group(s: string, i: number): { text: string; end: number } | null {
  if (s[i] !== "{") return null;
  let depth = 0;
  for (let j = i; j < s.length; j++) {
    if (s[j] === "\\") {
      j++;
      continue;
    }
    if (s[j] === "{") depth++;
    else if (s[j] === "}" && --depth === 0) return { text: s.slice(i + 1, j), end: j + 1 };
  }
  return null;
}

/** Reads what is between \schemestart and \schemestop into compounds (\chemfig), arrows (with what goes
 * over and under them) and \+. Anything else (an arrow with options or a name, \ce, a coefficient,
 * \chemmove) is a piece kept as written, so the rest of the scheme is still edited in pieces. */
export function readScheme(src: string): PieceData[] | null {
  const out: PieceData[] = [];
  let i = 0;
  // a piece kept as written: a command with its arguments, or a run of other characters
  const asWritten = (start: number): void => {
    let j = start;
    if (src[j] === "\\") {
      j++;
      while (/[A-Za-z]/.test(src[j] ?? "")) j++;
      if (j === start + 1) j++;
      for (;;) {
        const open = src[j];
        const close = open === "{" ? "}" : open === "[" ? "]" : open === "(" ? ")" : "";
        if (!close) break;
        let depth = 0;
        let k = j;
        for (; k < src.length; k++) {
          if (src[k] === "\\") {
            k++;
            continue;
          }
          if (src[k] === open) depth++;
          else if (src[k] === close && --depth === 0) break;
        }
        j = k + 1;
      }
    } else while (j < src.length && !/[\s\\]/.test(src[j])) j++;
    out.push({ kind: "chem", latex: src.slice(start, j) });
    i = j;
  };
  while (i < src.length) {
    if (/\s/.test(src[i])) {
      i++;
      continue;
    }
    if (src.startsWith("\\chemfig", i) && !/[a-zA-Z*]/.test(src[i + 8] ?? "")) {
      const g = group(src, i + 8);
      if (!g) return null;
      out.push({ kind: "chem", latex: `\\chemfig{${g.text}}` });
      i = g.end;
      continue;
    }
    if (src.startsWith("\\+", i)) {
      out.push({ kind: "rxtoken", text: "+" });
      i += 2;
      continue;
    }
    if (src.startsWith("\\arrow", i) && !/[a-zA-Z]/.test(src[i + 6] ?? "")) {
      const start = i;
      i += 6;
      const g = group(src, i);
      let arrow = "->";
      let above: string | null = null;
      let below: string | null = null;
      if (g) {
        const t = g.text.trim();
        arrow = SCHEME_ARROWS.find((a) => t.startsWith(a)) ?? "";
        if (!arrow) {
          asWritten(start);
          continue;
        }
        let rest = t.slice(arrow.length);
        const a = bracket(rest, 0);
        if (a) {
          above = a.text;
          rest = rest.slice(a.end);
          const b = bracket(rest, 0);
          if (b) {
            below = b.text;
            rest = rest.slice(b.end);
          }
        }
        if (rest.trim()) {
          asWritten(start);
          continue;
        }
        i = g.end;
      }
      // an arrow's own [angle, length…] or a name (a--b): the arrow as written
      if (src[i] === "[" || src[i] === "(" || src[start + 6] === "(") {
        asWritten(start);
        continue;
      }
      // a formula set as mhchem's (\ce{H2SO4}): its text, which the editor writes so again
      const bare = (t: string | null) => (t && /^\\ce\{([^{}]*)\}$/.exec(t.trim())?.[1]) ?? t;
      out.push({ kind: "rxarrow", arrow, above: bare(above), below: bare(below) });
      continue;
    }
    asWritten(i);
  }
  return out;
}

function species(coef: string, rest: string): PieceData {
  return { kind: "species", text: coef ? `${coef} ${rest}` : rest };
}

/** A species as mhchem writes it, as math the editor edits: a number after an element (or a closing
 * parenthesis) is its index, `^` a charge (`2H2O(l)` is `2H_{2}O(l)`, `SO4^2-` is `SO_{4}^{2-}`). */
export function chemToLatex(text: string): string {
  let out = "";
  // after an element or a closing: a number is an index; at the start (a coefficient) or after a sign, a number
  let atom = false;
  for (let i = 0; i < text.length; ) {
    const rest = text.slice(i);
    const command = /^\\(?:[A-Za-z]+|.)/.exec(rest);
    if (command) {
      out += command[0] + (/[A-Za-z]$/.test(command[0]) ? " " : "");
      i += command[0].length;
      atom = true;
      continue;
    }
    const c = text[i];
    if (c === "$") {
      const end = text.indexOf("$", i + 1);
      out += end < 0 ? text.slice(i + 1) : text.slice(i + 1, end);
      i = end < 0 ? text.length : end + 1;
      atom = true;
      continue;
    }
    if (c === "^" || c === "_") {
      const g = group(text, i + 1);
      const run = g ? null : (c === "^" ? /^(?:[0-9]*[+-]|[0-9]+|.)/ : /^(?:[0-9]+|.)/).exec(text.slice(i + 1));
      out += `${c}{${g ? g.text : (run?.[0] ?? "")}}`;
      i = g ? g.end : i + 1 + (run?.[0].length ?? 0);
      atom = true;
      continue;
    }
    const digits = /^[0-9]+/.exec(rest);
    if (digits) {
      out += atom ? `_{${digits[0]}}` : digits[0];
      i += digits[0].length;
      continue;
    }
    if (c !== " ") out += c;
    atom = /[A-Za-z)\]]/.test(c);
    i++;
  }
  return out;
}

/** A species' formula, as the editor writes it (`H_{2}O`), back in mhchem's own way: `H2O`, `SO4^2-`, and a space after a coefficient. */
export function latexToChem(latex: string): string {
  return latex
    // an index before a charge, as chemistry writes it (the editor writes the exponent first)
    .replace(/(?<=[A-Za-z)\]])\^\{([^{}]*)\}_\{([^{}]*)\}/g, "_{$2}^{$1}")
    // a number as index right after an element or a closing (not an isotope's, before its element)
    .replace(/(?<=[A-Za-z)\]])_\{([0-9]+)\}/g, "$1")
    // nothing under an index or a charge (an isotope's numbers, a piece pasted): mhchem's own way
    .replace(/\{\}(?=[_^])/g, "")
    // an empty index goes; an empty charge at the end is the sign of a gas (CO2 ^)
    .replace(/_\{\}/g, "")
    .replace(/\^\{\}$/, " ^")
    .replace(/\^\{([0-9]*[+-])\}/g, "^$1")
    .replace(/^([0-9]+(?:[.,/][0-9]+)?)(?=[A-Za-z(\[])/, "$1 ");
}

/** Writes pieces back as the text of a \ce (a coefficient apart from its formula, as mhchem draws it the same). */
export function writeReaction<T>(pieces: T[], piece: (p: T) => string): string {
  return pieces.map(piece).filter(Boolean).join(" ");
}

/** The text of an arrow with what goes over and under it. */
export function arrowText(arrow: string, above: string, below: string): string {
  if (!above && !below) return arrow;
  return `${arrow}[${above}]${below ? `[${below}]` : ""}`;
}

/** The conditions over or under an arrow as pieces: each character on its own (typed and deleted one
 * by one), but a command with its arguments (`\text{light}`, `\qty{25}{\celsius}`, `\Delta`) and math
 * between `$…$` as one piece, so the cursor never lands inside a command's name. */
export function conditionPieces(text: string): ({ ch: string } | { latex: string })[] {
  const out: ({ ch: string } | { latex: string })[] = [];
  for (let i = 0; i < text.length; ) {
    const c = text[i];
    if (c === "$") {
      const end = text.indexOf("$", i + 1);
      const stop = end < 0 ? text.length : end + 1;
      out.push({ latex: text.slice(i, stop) });
      i = stop;
      continue;
    }
    if (c === "\\" && i + 1 < text.length) {
      let j = i + 1;
      while (/[A-Za-z]/.test(text[j] ?? "")) j++;
      if (j === i + 1) j++;
      // its arguments, {…} and […], with what they hold
      while (text[j] === "{" || text[j] === "[") {
        const g = text[j] === "{" ? group(text, j) : bracket(text, j);
        if (!g) break;
        j = g.end;
      }
      out.push({ latex: text.slice(i, j) });
      i = j;
      continue;
    }
    out.push({ ch: c });
    i++;
  }
  return out;
}
