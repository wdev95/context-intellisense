// What the editor knows about LaTeX's commands: the class of each symbol (its spacing), the
// structures a command makes, and the delimiters.

/** TeX's atom classes, which set the spacing around a symbol. */
export type AtomClass = "ord" | "bin" | "rel" | "open" | "close" | "punct" | "op" | "inner";

const BIN = "pm mp times div cdot ast star circ bullet cup cap sqcup sqcap vee wedge oplus ominus otimes oslash odot setminus wr amalg dagger ddagger triangleleft triangleright bigtriangleup bigtriangledown land lor";
const REL =
  "le ge leq geq ne neq lt gt approx equiv sim simeq cong propto asymp doteq in ni notin subset supset subseteq supseteq subsetneq supsetneq sqsubseteq sqsupseteq " +
  "to gets rightarrow leftarrow Rightarrow Leftarrow leftrightarrow Leftrightarrow longrightarrow longleftarrow Longrightarrow Longleftarrow longleftrightarrow Longleftrightarrow " +
  "mapsto longmapsto implies impliedby iff uparrow downarrow updownarrow Uparrow Downarrow nearrow searrow swarrow nwarrow hookrightarrow hookleftarrow rightleftharpoons " +
  "perp parallel mid nmid vdash dashv models ll gg prec succ preceq succeq lesssim gtrsim nleq ngeq nsim ncong nsubseteq";
const OP = "sin cos tan cot sec csc sen tg cotg cossec arcsin arccos arctan arcsen arctg sinh cosh tanh coth senh log ln lg exp det dim ker hom arg deg Pr gcd lcm mdc mmc";
const OPEN = "langle lfloor lceil lvert lVert lbrace";
const CLOSE = "rangle rfloor rceil rvert rVert rbrace";

function table(list: string, cls: AtomClass, into: Record<string, AtomClass>): void {
  for (const name of list.split(/\s+/)) if (name) into[name] = cls;
}

/** The class of `\name`, for the commands that are not ordinary symbols. */
export const COMMAND_CLASS: Record<string, AtomClass> = {};
table(BIN, "bin", COMMAND_CLASS);
table(REL, "rel", COMMAND_CLASS);
table(OP, "op", COMMAND_CLASS);
table(OPEN, "open", COMMAND_CLASS);
table(CLOSE, "close", COMMAND_CLASS);
COMMAND_CLASS["{"] = "open";
COMMAND_CLASS["}"] = "close";
COMMAND_CLASS["|"] = "ord";

/** The class of a typed or read character. */
export const CHAR_CLASS: Record<string, AtomClass> = {
  "+": "bin", "-": "bin", "*": "bin", "−": "bin", "±": "bin", "×": "bin", "÷": "bin", "·": "bin",
  "=": "rel", "<": "rel", ">": "rel", "≤": "rel", "≥": "rel", "≠": "rel", "≈": "rel", "→": "rel", "←": "rel", "∈": "rel", ":": "rel",
  "(": "open", "[": "open", ")": "close", "]": "close", "!": "close", "?": "close",
  ",": "punct", ";": "punct",
};

/** What a typed character becomes in LaTeX, when it is not itself. */
export const CHAR_LATEX: Record<string, string> = { "*": "\\cdot", "−": "-", "±": "\\pm", "×": "\\times", "÷": "\\div", "·": "\\cdot", "≤": "\\le", "≥": "\\ge", "≠": "\\ne", "≈": "\\approx", "→": "\\to", "←": "\\gets", "∈": "\\in", "∞": "\\infty" };

/** Commands that take no argument and draw space. */
export const SPACES = new Set([",", ":", ";", "!", " ", "quad", "qquad", "enspace", "thinspace", "medspace", "thickspace"]);

/** Accents and lines over or under one argument. */
export const ACCENTS = new Set(["vec", "hat", "bar", "dot", "ddot", "dddot", "tilde", "acute", "grave", "breve", "check", "mathring", "widehat", "widetilde", "overline", "underline", "overrightarrow", "overleftarrow", "overleftrightarrow", "underrightarrow", "underleftarrow", "overbrace", "underbrace", "vv", "boxed", "cancel", "bcancel", "xcancel", "underleftrightarrow", "overgroup", "undergroup", "xrightarrow", "xleftarrow", "xRightarrow", "xLeftarrow", "xleftrightarrow", "xLeftrightarrow", "xmapsto", "xrightleftharpoons", "sout"]);

/** Alphabets and styles over one argument. */
export const STYLES = new Set(["mathbf", "mathbb", "mathcal", "mathscr", "mathfrak", "mathrm", "mathit", "mathsf", "mathtt", "boldsymbol", "bm"]);

/** Fractions and their relatives: two arguments, one over the other. */
export const FRACTIONS = new Set(["frac", "dfrac", "tfrac", "cfrac", "binom", "dbinom", "tbinom", "overset", "underset", "stackrel", "cancelto"]);

/** Large operators: limits below and above (or as scripts). Those after `|` take only a lower limit when inserted. */
export const LARGE_OPERATORS = new Set(["sum", "prod", "coprod", "int", "iint", "iiint", "oint", "oiint", "bigcup", "bigcap", "bigsqcup", "bigvee", "bigwedge", "bigoplus", "bigotimes", "bigodot", "biguplus", "lim", "limsup", "liminf", "max", "min", "sup", "inf", "argmax", "argmin"]);
export const LOWER_LIMIT_ONLY = new Set(["lim", "limsup", "liminf", "max", "min", "sup", "inf", "argmax", "argmin"]);

/** Environments read as a grid of cells. */
export const MATRICES = new Set(["matrix", "pmatrix", "bmatrix", "Bmatrix", "vmatrix", "Vmatrix", "smallmatrix", "cases", "rcases", "align", "aligned", "gathered", "split"]);

/** Text commands: their argument is text. */
export const TEXTS = new Set(["text", "textrm", "textit", "textbf", "textsf", "texttt", "mbox"]);

/** The delimiter commands `\left` and `\right` accept, with how each is written plainly beside a small body. */
export const DELIMITERS: Record<string, { open: string; close: string }> = {
  "(": { open: "(", close: "(" },
  ")": { open: ")", close: ")" },
  "[": { open: "[", close: "[" },
  "]": { open: "]", close: "]" },
  "\\{": { open: "\\{", close: "\\{" },
  "\\}": { open: "\\}", close: "\\}" },
  "|": { open: "\\lvert", close: "\\rvert" },
  "\\|": { open: "\\lVert", close: "\\rVert" },
  "\\langle": { open: "\\langle", close: "\\langle" },
  "\\rangle": { open: "\\rangle", close: "\\rangle" },
  "\\lfloor": { open: "\\lfloor", close: "\\lfloor" },
  "\\rfloor": { open: "\\rfloor", close: "\\rfloor" },
  "\\lceil": { open: "\\lceil", close: "\\lceil" },
  "\\rceil": { open: "\\rceil", close: "\\rceil" },
  "\\lvert": { open: "\\lvert", close: "\\lvert" },
  "\\rvert": { open: "\\rvert", close: "\\rvert" },
  "\\vert": { open: "\\lvert", close: "\\rvert" },
  "\\lVert": { open: "\\lVert", close: "\\lVert" },
  "\\rVert": { open: "\\rVert", close: "\\rVert" },
  "\\Vert": { open: "\\lVert", close: "\\rVert" },
  ".": { open: "", close: "" },
};

/** The matching closing delimiter of an opening one. */
export const CLOSING: Record<string, string> = { "(": ")", "[": "]", "\\{": "\\}", "|": "|", "\\|": "\\|", "\\langle": "\\rangle", "\\lfloor": "\\rfloor", "\\lceil": "\\rceil", "\\lvert": "\\rvert", "\\lVert": "\\rVert" };

/** The class of `\name` (ordinary if the editor does not know it). */
export function classOfCommand(name: string): AtomClass {
  return COMMAND_CLASS[name] ?? "ord";
}

/** Text characters LaTeX needs escaped inside `\text{…}`. */
export function escapeText(ch: string): string {
  switch (ch) {
    case "\\":
      return "\\textbackslash{}";
    case "{":
    case "}":
    case "#":
    case "$":
    case "%":
    case "&":
    case "_":
      return `\\${ch}`;
    case "^":
      return "\\^{}";
    case "~":
      return "\\~{}";
    default:
      return ch;
  }
}

// ---------- typing as on a calculator ----------

const GREEK = "alpha beta gamma delta epsilon zeta eta theta iota kappa lambda mu nu xi pi rho sigma tau upsilon phi chi psi omega Gamma Delta Theta Lambda Xi Pi Sigma Upsilon Phi Psi Omega";
const FUNCTIONS = "sin cos tan cot sec csc sinh cosh tanh coth arcsin arccos arctan sen log ln exp det gcd lcm max min arg deg dim ker";

/** Words typed in math, without `\`, that become what they name: Greek letters, functions set
 * upright, roots and large operators. Each is LaTeX as a palette's button writes it (`#?` an
 * empty place). */
export const WORDS: Record<string, string> = {
  sqrt: "\\sqrt{#?}",
  cbrt: "\\sqrt[3]{#?}",
  nthroot: "\\sqrt[#?]{#?}",
  sum: "\\sum_{#?}^{#?}",
  prod: "\\prod_{#?}^{#?}",
  int: "\\int",
  lim: "\\lim_{#?}",
  infty: "\\infty",
  infinity: "\\infty",
  // a relation, made a symbol (`x in A`); typed on, `in` becomes `int` or `infinity`
  in: "\\in",
};
for (const w of `${GREEK} ${FUNCTIONS}`.split(" ")) WORDS[w] = `\\${w}`;

/** Two characters typed one after the other that make one symbol (`<=` is ≤), with the symbol made. */
export const COMPOSED: Record<string, string> = { "<=": "\\le", ">=": "\\ge", "!=": "\\ne", "->": "\\to", "=>": "\\Rightarrow", "+-": "\\pm", "-+": "\\mp" };

/** Commands whose arguments are math, each edited in a place of its own (physics,
 * unicode-math, slashed): how many `{…}` they take, at least and at most. An optional `[…]` before
 * them (`\pdv[2]{f}{x}`) is a place too. */
export const CALLS: Record<string, [number, number]> = {
  pdv: [1, 3], dv: [1, 3], dd: [1, 1], fdv: [1, 2],
  ket: [1, 1], bra: [1, 1], braket: [1, 2], ketbra: [2, 2], expval: [1, 2], ev: [1, 2], mel: [3, 3], matrixel: [3, 3],
  comm: [2, 2], acomm: [2, 2], pb: [2, 2], abs: [1, 1], norm: [1, 1], vb: [1, 1], va: [1, 1], vu: [1, 1],
  slashed: [1, 1], symbf: [1, 1], symit: [1, 1], symrm: [1, 1], symsf: [1, 1], symcal: [1, 1],
};

