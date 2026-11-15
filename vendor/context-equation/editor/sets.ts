// The palettes that come with the editor. Each button inserts a snippet of LaTeX (`#?` an empty
// place, `#0` where the selection goes) or runs an action; contextEquation draws it from the same snippet.

import type { Words } from "./shared.ts";

export interface PaletteItem {
  /** LaTeX to insert at the cursor: `#?` is an empty place (a whole argument), `#0` where the selection goes. */
  insert?: string;
  /** An action of the field instead: `sup`, `sub`, `addRow`, `moveLeft`, `undo`… (see `ContextEquationField.run`). */
  action?: string;
  /** The formula drawn on the button (LaTeX); by default the snippet, its places drawn empty. */
  preview?: string;
  /** Text on the button instead of a drawing. */
  label?: string;
  /** An icon on the button instead of a drawing (see `icon`). */
  icon?: string;
  /** The button's name, shown as its tooltip and read by screen readers. */
  title: Words;
  /** The name typed after `\` that does the same, for the command menu. */
  command?: string;
  /** Opens a picker instead: `matrix`, a grid to choose a matrix's lines and columns. */
  picker?: "matrix";
  /** Takes two columns (long formulas). */
  wide?: boolean;
}

export interface PaletteSet {
  id: string;
  title: Words;
  items: PaletteItem[];
}

const t = (en: string, pt: string): Words => ({ en, "pt-BR": pt });

/** A symbol: its command inserts it and names it in the command menu. */
const sym = (latex: string, en: string, pt: string): PaletteItem => ({ insert: latex, title: t(en, pt), command: /^\\[A-Za-z]+$/.test(latex) ? latex.slice(1) : undefined });
/** A structure with empty places. */
const tpl = (insert: string, en: string, pt: string, extra: Partial<PaletteItem> = {}): PaletteItem => ({ insert, title: t(en, pt), ...extra });
/** A whole formula, ready to edit. */
const formula = (latex: string, en: string, pt: string): PaletteItem => ({ insert: latex, title: t(en, pt), wide: true });
/** A one-argument command over the selection (accents, alphabets, arrows with a label). */
const over = (name: string, en: string, pt: string, preview?: string): PaletteItem => ({ insert: `\\${name}{#0}`, preview, title: t(en, pt), command: name });

export const COMMON: PaletteSet = {
  id: "common",
  title: { en: "Common" },
  items: [
    tpl("\\frac{#0}{#?}", "Fraction", "Fração", { command: "frac" }),
    tpl("\\sqrt{#0}", "Square root", "Raiz quadrada", { command: "sqrt" }),
    { action: "sup", preview: "#?^{#?}", title: t("Exponent", "Expoente") },
    { action: "sub", preview: "#?_{#?}", title: t("Index", "Índice") },
    sym("+", "Plus", "Mais"), sym("-", "Minus", "Menos"),
    sym("\\times", "Times", "Vezes"), sym("\\div", "Divided by", "Dividido por"),
    sym("=", "Equals", "Igual"), sym("\\pi", "Pi", "Pi"),
    tpl("\\sum_{#?}^{#?}#0", "Sum", "Somatório", { preview: "\\sum_{#?}^{#?}", command: "sum" }),
    tpl("\\int_{#?}^{#?}#0", "Integral", "Integral", { preview: "\\int_{#?}^{#?}", command: "int" }),
    tpl("\\lim_{#?}#0", "Limit", "Limite", { preview: "\\lim_{#?}", command: "lim" }),
    tpl("\\left(#0\\right)", "Parentheses", "Parênteses", { preview: "\\left(#?\\right)" }),
    tpl("\\startpmatrix#?\\NC#?\\NR#?\\NC#?\\stoppmatrix", "2×2 matrix", "Matriz 2×2"),
    tpl("\\vec{#0}", "Vector", "Vetor", { command: "vec" }),
    tpl("\\text{#0}", "Text", "Texto", { preview: "\\text{abc}", command: "text" })
  ]
};

export const STRUCTURES: PaletteSet = {
  id: "structures",
  title: t("Structures", "Estruturas"),
  items: [
    tpl("\\frac{#0}{#?}", "Fraction", "Fração", { command: "frac" }),
    tpl("\\dfrac{#0}{#?}", "Display fraction", "Fração grande", { command: "dfrac" }),
    tpl("\\tfrac{#0}{#?}", "Small fraction", "Fração pequena", { command: "tfrac" }),
    tpl("{#0}/{#?}", "Slanted fraction", "Fração na linha", { preview: "#?/#?" }),
    tpl("\\cfrac{#0}{#?}", "Continued fraction", "Fração contínua", { command: "cfrac" }),
    tpl("\\sqrt{#0}", "Square root", "Raiz quadrada", { command: "sqrt" }),
    tpl("\\sqrt[3]{#0}", "Cube root", "Raiz cúbica"),
    tpl("\\sqrt[#?]{#0}", "n-th root", "Raiz de índice n", { command: "root" }),
    { action: "sup", preview: "#?^{#?}", title: t("Exponent", "Expoente"), command: "sup" },
    { action: "sub", preview: "#?_{#?}", title: t("Index", "Índice") },
    tpl("^{#?}_{#?}", "Exponent and index", "Expoente e índice", { preview: "#?^{#?}_{#?}" }),
    tpl("{}^{#?}_{#?}#0", "Scripts on the left", "Índices à esquerda", { preview: "{}^{#?}_{#?}#?" }),
    tpl("\\overset{#?}{#0}", "Over", "Sobrescrito em cima", { command: "overset" }),
    tpl("\\underset{#?}{#0}", "Under", "Subscrito embaixo", { command: "underset" }),
    tpl("\\binom{#0}{#?}", "Binomial coefficient", "Número binomial", { command: "binom" }),
    tpl("\\left(#0\\right)", "Parentheses", "Parênteses", { preview: "\\left(#?\\right)", command: "paren" }),
    tpl("\\left[#0\\right]", "Brackets", "Colchetes", { preview: "\\left[#?\\right]", command: "brackets" }),
    tpl("\\left\\{#0\\right\\}", "Braces", "Chaves", { preview: "\\left\\{#?\\right\\}", command: "braces" }),
    tpl("\\left|#0\\right|", "Absolute value", "Módulo", { preview: "\\left|#?\\right|", command: "abs" }),
    tpl("\\left\\|#0\\right\\|", "Norm", "Norma", { preview: "\\left\\|#?\\right\\|", command: "norm" }),
    tpl("\\left\\langle #0\\right\\rangle", "Angle brackets", "Chevrons", { preview: "\\left\\langle #?\\right\\rangle", command: "angle" }),
    tpl("\\left\\lfloor #0\\right\\rfloor", "Floor", "Piso", { preview: "\\left\\lfloor #?\\right\\rfloor", command: "floor" }),
    tpl("\\left\\lceil #0\\right\\rceil", "Ceiling", "Teto", { preview: "\\left\\lceil #?\\right\\rceil", command: "ceil" }),
    tpl("\\left[#0\\right)", "Half-open interval", "Intervalo semiaberto", { preview: "\\left[#?\\right)" }),
    tpl("\\left]#0\\right[", "Open interval (pt-BR)", "Intervalo aberto", { preview: "\\left]#?\\right[" }),
    tpl("\\left.#0\\right|", "Evaluation bar", "Barra de avaliação", { preview: "\\left.#?\\right|_{#?}" }),
    tpl("\\text{#0}", "Text", "Texto", { preview: "\\text{abc}", command: "text" }),
    over("boxed", "Box", "Caixa"),
  ],
};

export const CALCULUS: PaletteSet = {
  id: "calculus",
  title: t("Calculus", "Cálculo"),
  items: [
    tpl("\\sum_{#?}^{#?}#0", "Sum", "Somatório", { preview: "\\sum_{#?}^{#?}", command: "sum" }),
    tpl("\\sum_{#?}#0", "Sum below", "Somatório embaixo", { preview: "\\sum_{#?}" }),
    tpl("\\prod_{#?}^{#?}#0", "Product", "Produtório", { preview: "\\prod_{#?}^{#?}", command: "prod" }),
    tpl("\\coprod_{#?}^{#?}#0", "Coproduct", "Coproduto", { preview: "\\coprod_{#?}^{#?}", command: "coprod" }),
    tpl("\\int_{#?}^{#?}#0", "Integral", "Integral", { preview: "\\int_{#?}^{#?}", command: "int" }),
    tpl("\\int #0\\,dx", "Indefinite integral", "Integral indefinida", { preview: "\\int #?\\,dx" }),
    tpl("\\iint_{#?}#0", "Double integral", "Integral dupla", { preview: "\\iint_{#?}", command: "iint" }),
    tpl("\\iiint_{#?}#0", "Triple integral", "Integral tripla", { preview: "\\iiint_{#?}", command: "iiint" }),
    tpl("\\oint_{#?}#0", "Contour integral", "Integral de linha", { preview: "\\oint_{#?}", command: "oint" }),
    tpl("\\oiint_{#?}#0", "Surface integral", "Integral de superfície", { preview: "\\oiint_{#?}", command: "oiint" }),
    tpl("\\lim_{#?}#0", "Limit", "Limite", { preview: "\\lim_{x\\to a}", command: "lim" }),
    tpl("\\lim_{x\\to\\infty}#0", "Limit at infinity", "Limite no infinito", { preview: "\\lim_{x\\to\\infty}" }),
    tpl("\\lim_{x\\to 0^{+}}#0", "Limit from the right", "Limite à direita", { preview: "\\lim_{x\\to 0^{+}}" }),
    tpl("\\limsup_{#?}#0", "Upper limit", "Limite superior", { preview: "\\limsup_{#?}", command: "limsup" }),
    tpl("\\liminf_{#?}#0", "Lower limit", "Limite inferior", { preview: "\\liminf_{#?}", command: "liminf" }),
    tpl("\\frac{d}{dx}#0", "Derivative", "Derivada", { preview: "\\frac{d}{dx}" }),
    tpl("\\frac{dy}{dx}", "dy/dx", "dy/dx"),
    tpl("\\frac{d^{2}}{dx^{2}}#0", "Second derivative", "Derivada segunda", { preview: "\\frac{d^{2}}{dx^{2}}" }),
    tpl("\\frac{d^{2}y}{dx^{2}}", "d²y/dx²", "d²y/dx²"),
    tpl("\\frac{\\partial}{\\partial x}#0", "Partial derivative", "Derivada parcial", { preview: "\\frac{\\partial}{\\partial x}" }),
    tpl("\\frac{\\partial f}{\\partial x}", "∂f/∂x", "∂f/∂x"),
    tpl("\\frac{\\partial^{2} f}{\\partial x^{2}}", "Second partial", "Parcial segunda"),
    tpl("\\frac{\\partial^{2} f}{\\partial x\\,\\partial y}", "Mixed partial", "Parcial mista"),
    tpl("f'(x)", "f prime", "f linha"),
    tpl("f''(x)", "f second", "f duas linhas"),
    tpl("\\dot{#0}", "Time derivative", "Derivada no tempo", { command: "dot" }),
    tpl("\\ddot{#0}", "Second time derivative", "Derivada segunda no tempo", { command: "ddot" }),
    tpl("\\left.#0\\right|_{#?}^{#?}", "Evaluated between", "Avaliado entre", { preview: "\\left.#?\\right|_{#?}^{#?}" }),
    sym("\\partial", "Partial", "Parcial"),
    sym("\\nabla", "Nabla", "Nabla"),
    tpl("\\nabla\\cdot", "Divergence", "Divergente"),
    tpl("\\nabla\\times", "Curl", "Rotacional"),
    tpl("\\nabla^{2}", "Laplacian", "Laplaciano"),
    sym("\\infty", "Infinity", "Infinito"),
    sym("\\to", "Tends to", "Tende a"),
    sym("\\Delta", "Delta", "Delta"),
    sym("\\mathrm{d}", "Upright d", "d reto"),
    sym("\\,", "Thin space", "Espaço fino"),
  ],
};

const grid = (env: string, rows: number, cols: number) => `\\start${env}${Array.from({ length: rows }, () => Array.from({ length: cols }, () => "#?").join("\\NC")).join("\\NR")}\\stop${env}`;

export const MATRICES: PaletteSet = {
  id: "matrices",
  title: t("Matrices", "Matrizes"),
  items: [
    { picker: "matrix", icon: "table", label: "m × n", title: t("Matrix of any size (or the size of this one)", "Matriz de qualquer tamanho (ou o tamanho desta)"), wide: true },
    tpl(grid("pmatrix", 2, 2), "2×2 matrix", "Matriz 2×2", { command: "pmatrix" }),
    tpl(grid("pmatrix", 3, 3), "3×3 matrix", "Matriz 3×3"),
    tpl(grid("pmatrix", 2, 3), "2×3 matrix", "Matriz 2×3"),
    tpl(grid("pmatrix", 3, 2), "3×2 matrix", "Matriz 3×2"),
    tpl(grid("pmatrix", 1, 2), "Row vector", "Vetor linha"),
    tpl(grid("pmatrix", 1, 3), "Row vector (3)", "Vetor linha (3)"),
    tpl(grid("pmatrix", 2, 1), "Column vector", "Vetor coluna"),
    tpl(grid("pmatrix", 3, 1), "Column vector (3)", "Vetor coluna (3)"),
    tpl(grid("bmatrix", 2, 2), "Matrix in brackets", "Matriz entre colchetes", { command: "bmatrix" }),
    tpl(grid("bmatrix", 3, 3), "3×3 in brackets", "3×3 entre colchetes"),
    tpl(grid("Bmatrix", 2, 2), "Matrix in braces", "Matriz entre chaves", { command: "Bmatrix" }),
    tpl(grid("vmatrix", 2, 2), "Determinant", "Determinante", { command: "vmatrix" }),
    tpl(grid("vmatrix", 3, 3), "3×3 determinant", "Determinante 3×3"),
    tpl(grid("Vmatrix", 2, 2), "Double bars", "Barras duplas", { command: "Vmatrix" }),
    tpl(grid("matrix", 2, 2), "Without delimiters", "Sem delimitadores", { command: "matrix" }),
    tpl("\\begin{pmatrix}1&0\\\\0&1\\end{pmatrix}", "Identity 2×2", "Identidade 2×2"),
    tpl("\\begin{pmatrix}1&0&0\\\\0&1&0\\\\0&0&1\\end{pmatrix}", "Identity 3×3", "Identidade 3×3"),
    tpl("\\begin{pmatrix}#?&\\cdots&#?\\\\\\vdots&\\ddots&\\vdots\\\\#?&\\cdots&#?\\end{pmatrix}", "n×n matrix", "Matriz n×n"),
    tpl(grid("cases", 2, 2), "Cases", "Casos", { command: "cases" }),
    tpl(grid("cases", 3, 2), "Three cases", "Três casos"),
    tpl("\\begin{cases}#?\\\\#?\\end{cases}", "System of equations", "Sistema de equações"),
    tpl("\\begin{cases}#?\\\\#?\\\\#?\\end{cases}", "System of three", "Sistema de três"),
    tpl(grid("aligned", 2, 2), "Aligned lines", "Linhas alinhadas", { command: "aligned" }),
    tpl(grid("aligned", 3, 2), "Three aligned lines", "Três linhas alinhadas"),
    { action: "addRow", icon: "rowAdd", title: t("Add a line below", "Adicionar linha abaixo") },
    { action: "addRowAbove", icon: "rowAdd", title: t("Add a line above", "Adicionar linha acima") },
    { action: "addColumn", icon: "columnAdd", title: t("Add a column to the right", "Adicionar coluna à direita") },
    { action: "addColumnLeft", icon: "columnAdd", title: t("Add a column to the left", "Adicionar coluna à esquerda") },
    { action: "removeRow", icon: "rowRemove", title: t("Remove this line", "Remover esta linha") },
    { action: "removeColumn", icon: "columnRemove", title: t("Remove this column", "Remover esta coluna") },
  ],
};

const GREEK: [string, string, string][] = [
  ["alpha", "alpha", "alfa"], ["beta", "beta", "beta"], ["gamma", "gamma", "gama"], ["delta", "delta", "delta"], ["epsilon", "epsilon", "épsilon"], ["varepsilon", "epsilon (variant)", "épsilon (variante)"],
  ["zeta", "zeta", "zeta"], ["eta", "eta", "eta"], ["theta", "theta", "teta"], ["vartheta", "theta (variant)", "teta (variante)"], ["iota", "iota", "iota"], ["kappa", "kappa", "capa"],
  ["varkappa", "kappa (variant)", "capa (variante)"], ["lambda", "lambda", "lambda"], ["mu", "mu", "mi"], ["nu", "nu", "ni"], ["xi", "xi", "csi"], ["omicron", "omicron", "ômicron"],
  ["pi", "pi", "pi"], ["varpi", "pi (variant)", "pi (variante)"], ["rho", "rho", "rô"], ["varrho", "rho (variant)", "rô (variante)"], ["sigma", "sigma", "sigma"], ["varsigma", "final sigma", "sigma final"],
  ["tau", "tau", "tau"], ["upsilon", "upsilon", "ípsilon"], ["phi", "phi", "fi"], ["varphi", "phi (variant)", "fi (variante)"], ["chi", "chi", "qui"], ["psi", "psi", "psi"],
  ["omega", "omega", "ômega"], ["digamma", "digamma", "digama"],
  ["Gamma", "Gamma", "Gama maiúsculo"], ["Delta", "Delta", "Delta maiúsculo"], ["Theta", "Theta", "Teta maiúsculo"], ["Lambda", "Lambda", "Lambda maiúsculo"], ["Xi", "Xi", "Csi maiúsculo"], ["Pi", "Pi", "Pi maiúsculo"],
  ["Sigma", "Sigma", "Sigma maiúsculo"], ["Upsilon", "Upsilon", "Ípsilon maiúsculo"], ["Phi", "Phi", "Fi maiúsculo"], ["Psi", "Psi", "Psi maiúsculo"], ["Omega", "Omega", "Ômega maiúsculo"],
  ["varGamma", "Gamma (italic)", "Gama itálico"], ["varDelta", "Delta (italic)", "Delta itálico"], ["varTheta", "Theta (italic)", "Teta itálico"], ["varLambda", "Lambda (italic)", "Lambda itálico"],
  ["varPi", "Pi (italic)", "Pi itálico"], ["varSigma", "Sigma (italic)", "Sigma itálico"], ["varPhi", "Phi (italic)", "Fi itálico"], ["varPsi", "Psi (italic)", "Psi itálico"], ["varOmega", "Omega (italic)", "Ômega itálico"],
];

export const GREEK_LETTERS: PaletteSet = { id: "greek", title: t("Greek", "Gregas"), items: GREEK.map(([name, en, pt]) => sym(`\\${name}`, en, pt)) };

const HEBREW: PaletteItem[] = [sym("\\aleph", "Aleph", "Álef"), sym("\\beth", "Beth", "Bet"), sym("\\gimel", "Gimel", "Guímel"), sym("\\daleth", "Daleth", "Dálet")];

export const OPERATORS: PaletteSet = {
  id: "operators",
  title: t("Operators", "Operadores"),
  items: [
    sym("+", "Plus", "Mais"), sym("-", "Minus", "Menos"), sym("\\pm", "Plus or minus", "Mais ou menos"), sym("\\mp", "Minus or plus", "Menos ou mais"),
    sym("\\times", "Times", "Vezes"), sym("\\div", "Divided by", "Dividido por"), sym("\\cdot", "Dot", "Ponto"), sym("/", "Slash", "Barra"),
    sym("\\ast", "Asterisk", "Asterisco"), sym("\\star", "Star", "Estrela"), sym("\\circ", "Composition", "Composição"), sym("\\bullet", "Bullet", "Ponto cheio"),
    sym("\\cup", "Union", "União"), sym("\\cap", "Intersection", "Interseção"), sym("\\setminus", "Set difference", "Diferença"), sym("\\uplus", "Disjoint union", "União disjunta"),
    sym("\\sqcup", "Square union", "União quadrada"), sym("\\sqcap", "Square intersection", "Interseção quadrada"), sym("\\vee", "Or", "Ou"), sym("\\wedge", "And", "E"),
    sym("\\oplus", "Direct sum", "Soma direta"), sym("\\ominus", "Circled minus", "Menos circulado"), sym("\\otimes", "Tensor product", "Produto tensorial"), sym("\\oslash", "Circled slash", "Barra circulada"),
    sym("\\odot", "Circled dot", "Ponto circulado"), sym("\\bigcirc", "Big circle", "Círculo grande"), sym("\\dagger", "Dagger", "Adaga"), sym("\\ddagger", "Double dagger", "Adaga dupla"),
    sym("\\amalg", "Amalgamation", "Amálgama"), sym("\\wr", "Wreath product", "Produto entrelaçado"), sym("\\diamond", "Diamond", "Losango"), sym("\\triangleleft", "Left triangle", "Triângulo à esquerda"),
    sym("\\triangleright", "Right triangle", "Triângulo à direita"), sym("!", "Factorial", "Fatorial"), sym("\\%", "Percent", "Por cento"), sym("\\perthousand", "Per thousand", "Por mil"),
    tpl("\\bigcup_{#?}^{#?}#0", "Big union", "União grande", { preview: "\\bigcup_{#?}^{#?}", command: "bigcup" }),
    tpl("\\bigcap_{#?}^{#?}#0", "Big intersection", "Interseção grande", { preview: "\\bigcap_{#?}^{#?}", command: "bigcap" }),
    tpl("\\bigoplus_{#?}^{#?}#0", "Big direct sum", "Soma direta grande", { preview: "\\bigoplus_{#?}^{#?}", command: "bigoplus" }),
    tpl("\\bigotimes_{#?}^{#?}#0", "Big tensor product", "Produto tensorial grande", { preview: "\\bigotimes_{#?}^{#?}", command: "bigotimes" }),
    tpl("\\bigvee_{#?}#0", "Big or", "Ou grande", { preview: "\\bigvee_{#?}", command: "bigvee" }),
    tpl("\\bigwedge_{#?}#0", "Big and", "E grande", { preview: "\\bigwedge_{#?}", command: "bigwedge" }),
  ],
};

export const RELATIONS: PaletteSet = {
  id: "relations",
  title: t("Relations", "Relações"),
  items: [
    sym("=", "Equals", "Igual"), sym("\\ne", "Not equal", "Diferente"), sym("<", "Less than", "Menor que"), sym(">", "Greater than", "Maior que"),
    sym("\\le", "Less or equal", "Menor ou igual"), sym("\\ge", "Greater or equal", "Maior ou igual"), sym("\\leqslant", "Less or equal (slanted)", "Menor ou igual (inclinado)"), sym("\\geqslant", "Greater or equal (slanted)", "Maior ou igual (inclinado)"),
    sym("\\ll", "Much less", "Muito menor"), sym("\\gg", "Much greater", "Muito maior"), sym("\\nless", "Not less", "Não menor"), sym("\\ngtr", "Not greater", "Não maior"),
    sym("\\nleq", "Not less or equal", "Não menor ou igual"), sym("\\ngeq", "Not greater or equal", "Não maior ou igual"), sym("\\lesssim", "Less or about", "Menor ou aproximadamente"), sym("\\gtrsim", "Greater or about", "Maior ou aproximadamente"),
    sym("\\approx", "Approximately", "Aproximadamente"), sym("\\approxeq", "Approximately equal", "Aproximadamente igual"), sym("\\equiv", "Equivalent", "Equivalente"), sym("\\not\\equiv", "Not equivalent", "Não equivalente"),
    sym("\\sim", "Similar", "Semelhante"), sym("\\nsim", "Not similar", "Não semelhante"), sym("\\simeq", "Similar or equal", "Semelhante ou igual"), sym("\\cong", "Congruent", "Congruente"),
    sym("\\ncong", "Not congruent", "Não congruente"), sym("\\doteq", "Approaches", "Aproxima-se de"), sym("\\triangleq", "Defined as", "Definido como"), sym("\\coloneqq", "Is defined as", "É definido como"),
    sym("\\propto", "Proportional", "Proporcional"), sym("\\asymp", "Asymptotic", "Assintótico"), sym("\\prec", "Precedes", "Precede"), sym("\\succ", "Succeeds", "Sucede"),
    sym("\\preceq", "Precedes or equal", "Precede ou igual"), sym("\\succeq", "Succeeds or equal", "Sucede ou igual"), sym("\\mid", "Divides", "Divide"), sym("\\nmid", "Does not divide", "Não divide"),
    sym("\\parallel", "Parallel", "Paralelo"), sym("\\nparallel", "Not parallel", "Não paralelo"), sym("\\perp", "Perpendicular", "Perpendicular"), sym("\\models", "Models", "Modela"),
    sym("\\vdash", "Proves", "Prova"), sym("\\dashv", "Is proved by", "É provado por"),
  ],
};

export const ARROWS: PaletteSet = {
  id: "arrows",
  title: t("Arrows", "Setas"),
  items: [
    sym("\\to", "Right arrow", "Seta para a direita"), sym("\\gets", "Left arrow", "Seta para a esquerda"), sym("\\leftrightarrow", "Both ways", "Seta dupla"), sym("\\uparrow", "Up arrow", "Seta para cima"),
    sym("\\downarrow", "Down arrow", "Seta para baixo"), sym("\\updownarrow", "Up and down", "Para cima e para baixo"), sym("\\nearrow", "North-east", "Nordeste"), sym("\\searrow", "South-east", "Sudeste"),
    sym("\\swarrow", "South-west", "Sudoeste"), sym("\\nwarrow", "North-west", "Noroeste"), sym("\\Rightarrow", "Implies", "Implica"), sym("\\Leftarrow", "Is implied by", "É implicado por"),
    sym("\\Leftrightarrow", "If and only if", "Se e somente se"), sym("\\Uparrow", "Double up", "Dupla para cima"), sym("\\Downarrow", "Double down", "Dupla para baixo"), sym("\\nRightarrow", "Does not imply", "Não implica"),
    sym("\\longrightarrow", "Long arrow", "Seta longa"), sym("\\longleftarrow", "Long left arrow", "Seta longa para a esquerda"), sym("\\longleftrightarrow", "Long both ways", "Seta longa dupla"), sym("\\Longrightarrow", "Long implies", "Implica (longa)"),
    sym("\\Longleftarrow", "Long implied by", "É implicado por (longa)"), sym("\\Longleftrightarrow", "Long if and only if", "Se e somente se (longa)"), sym("\\mapsto", "Maps to", "Leva em"), sym("\\longmapsto", "Long maps to", "Leva em (longa)"),
    sym("\\hookrightarrow", "Injection", "Injeção"), sym("\\hookleftarrow", "Hook left", "Gancho à esquerda"), sym("\\twoheadrightarrow", "Surjection", "Sobrejeção"), sym("\\rightsquigarrow", "Leads to", "Leva a"),
    sym("\\rightharpoonup", "Right harpoon", "Arpão à direita"), sym("\\leftharpoonup", "Left harpoon", "Arpão à esquerda"), sym("\\rightleftharpoons", "Equilibrium", "Equilíbrio"), sym("\\rightleftarrows", "Opposite arrows", "Setas opostas"),
    sym("\\circlearrowright", "Clockwise", "Sentido horário"), sym("\\circlearrowleft", "Counterclockwise", "Sentido anti-horário"),
    over("xrightarrow", "Arrow with label", "Seta com rótulo", "\\xrightarrow{#?}"), over("xleftarrow", "Left arrow with label", "Seta à esquerda com rótulo", "\\xleftarrow{#?}"),
    over("xRightarrow", "Implies with label", "Implica com rótulo", "\\xRightarrow{#?}"), over("xleftrightarrow", "Both ways with label", "Seta dupla com rótulo", "\\xleftrightarrow{#?}"),
    over("xmapsto", "Maps to with label", "Leva em com rótulo", "\\xmapsto{#?}"), over("xrightleftharpoons", "Equilibrium with label", "Equilíbrio com rótulo", "\\xrightleftharpoons{#?}"),
  ],
};

export const SETS: PaletteSet = {
  id: "sets",
  title: t("Sets and logic", "Conjuntos e lógica"),
  items: [
    sym("\\in", "Belongs to", "Pertence"), sym("\\notin", "Does not belong", "Não pertence"), sym("\\ni", "Contains", "Contém o elemento"), sym("\\subset", "Subset", "Está contido"),
    sym("\\supset", "Superset", "Contém"), sym("\\subseteq", "Subset or equal", "Contido ou igual"), sym("\\supseteq", "Superset or equal", "Contém ou igual"), sym("\\subsetneq", "Proper subset", "Contido propriamente"),
    sym("\\supsetneq", "Proper superset", "Contém propriamente"), sym("\\nsubseteq", "Not a subset", "Não está contido"), sym("\\emptyset", "Empty set", "Conjunto vazio"), sym("\\varnothing", "Empty set (variant)", "Conjunto vazio (variante)"),
    sym("\\complement", "Complement", "Complementar"), sym("\\mathcal{P}", "Power set", "Conjunto das partes"), tpl("\\left\\{#0\\right\\}", "Set", "Conjunto", { preview: "\\left\\{#?\\right\\}" }), tpl("\\left\\{#0 \\mid #?\\right\\}", "Set builder", "Conjunto por propriedade", { preview: "\\{x \\mid P(x)\\}" }),
    sym("\\mathbb{N}", "Natural numbers", "Naturais"), sym("\\mathbb{Z}", "Integers", "Inteiros"), sym("\\mathbb{Q}", "Rationals", "Racionais"), sym("\\mathbb{R}", "Reals", "Reais"),
    sym("\\mathbb{C}", "Complex numbers", "Complexos"), sym("\\mathbb{I}", "Irrationals", "Irracionais"), sym("\\mathbb{P}", "Primes", "Primos"), sym("\\mathbb{H}", "Quaternions", "Quatérnios"),
    tpl("\\mathbb{R}^{#?}", "Real space", "Espaço real"), tpl("\\mathbb{N}^{*}", "Positive naturals", "Naturais sem o zero"), tpl("\\mathbb{R}_{+}", "Non-negative reals", "Reais não negativos"), tpl("\\mathbb{Z}_{#?}", "Integers modulo", "Inteiros módulo"),
    sym("\\forall", "For all", "Para todo"), sym("\\exists", "There exists", "Existe"), sym("\\nexists", "There is no", "Não existe"), tpl("\\exists!", "There is exactly one", "Existe um único"),
    sym("\\neg", "Not", "Não"), sym("\\land", "And", "E"), sym("\\lor", "Or", "Ou"), sym("\\oplus", "Exclusive or", "Ou exclusivo"),
    sym("\\implies", "Implies", "Implica"), sym("\\iff", "If and only if", "Se e somente se"), sym("\\top", "True", "Verdadeiro"), sym("\\bot", "False", "Falso"),
    sym("\\therefore", "Therefore", "Portanto"), sym("\\because", "Because", "Porque"), sym("\\vdash", "Proves", "Prova"), sym("\\models", "Satisfies", "Satisfaz"),
    ...HEBREW,
    sym("\\ldots", "Dots", "Reticências"), sym("\\cdots", "Centered dots", "Reticências centradas"), sym("\\vdots", "Vertical dots", "Reticências verticais"), sym("\\ddots", "Diagonal dots", "Reticências diagonais"),
  ],
};

export const FUNCTIONS: PaletteSet = {
  id: "functions",
  title: t("Functions", "Funções"),
  items: [
    sym("\\sin", "Sine", "Seno"), sym("\\cos", "Cosine", "Cosseno"), sym("\\tan", "Tangent", "Tangente"), sym("\\cot", "Cotangent", "Cotangente"),
    sym("\\sec", "Secant", "Secante"), sym("\\csc", "Cosecant", "Cossecante"), sym("\\arcsin", "Arcsine", "Arco seno"), sym("\\arccos", "Arccosine", "Arco cosseno"),
    sym("\\arctan", "Arctangent", "Arco tangente"), sym("\\arccot", "Arccotangent", "Arco cotangente"), sym("\\arcsec", "Arcsecant", "Arco secante"), sym("\\arccsc", "Arccosecant", "Arco cossecante"),
    sym("\\sinh", "Hyperbolic sine", "Seno hiperbólico"), sym("\\cosh", "Hyperbolic cosine", "Cosseno hiperbólico"), sym("\\tanh", "Hyperbolic tangent", "Tangente hiperbólica"), sym("\\coth", "Hyperbolic cotangent", "Cotangente hiperbólica"),
    sym("\\sech", "Hyperbolic secant", "Secante hiperbólica"), sym("\\csch", "Hyperbolic cosecant", "Cossecante hiperbólica"), sym("\\operatorname{sen}", "Sine (sen)", "Seno (sen)"), sym("\\operatorname{tg}", "Tangent (tg)", "Tangente (tg)"),
    sym("\\operatorname{cotg}", "Cotangent (cotg)", "Cotangente (cotg)"), sym("\\operatorname{cossec}", "Cosecant (cossec)", "Cossecante (cossec)"), sym("\\operatorname{arcsen}", "Arcsine (arcsen)", "Arco seno (arcsen)"), sym("\\operatorname{arctg}", "Arctangent (arctg)", "Arco tangente (arctg)"),
    sym("\\exp", "Exponential", "Exponencial"), tpl("e^{#?}", "e to the", "e elevado a"), sym("\\log", "Logarithm", "Logaritmo"), tpl("\\log_{#?}#0", "Logarithm in base", "Logaritmo na base", { preview: "\\log_{#?}" }),
    sym("\\ln", "Natural logarithm", "Logaritmo natural"), sym("\\lg", "Base-10 logarithm", "Logaritmo decimal"), sym("\\det", "Determinant", "Determinante"), sym("\\dim", "Dimension", "Dimensão"),
    sym("\\ker", "Kernel", "Núcleo"), sym("\\hom", "Homomorphisms", "Homomorfismos"), sym("\\arg", "Argument", "Argumento"), sym("\\deg", "Degree", "Grau"),
    sym("\\gcd", "Greatest common divisor", "Máximo divisor comum"), sym("\\lcm", "Least common multiple", "Mínimo múltiplo comum"), sym("\\operatorname{mdc}", "gcd (mdc)", "mdc"), sym("\\operatorname{mmc}", "lcm (mmc)", "mmc"),
    sym("\\Pr", "Probability", "Probabilidade"), sym("\\operatorname{tr}", "Trace", "Traço"), sym("\\operatorname{sgn}", "Sign", "Sinal"), sym("\\Re", "Real part", "Parte real"),
    sym("\\Im", "Imaginary part", "Parte imaginária"), sym("\\operatorname{erf}", "Error function", "Função erro"),
    tpl("\\max_{#?}#0", "Maximum", "Máximo", { preview: "\\max_{#?}", command: "max" }), tpl("\\min_{#?}#0", "Minimum", "Mínimo", { preview: "\\min_{#?}", command: "min" }),
    tpl("\\sup_{#?}#0", "Supremum", "Supremo", { preview: "\\sup_{#?}", command: "sup_" }), tpl("\\inf_{#?}#0", "Infimum", "Ínfimo", { preview: "\\inf_{#?}", command: "inf" }),
    tpl("\\operatorname*{arg\\,max}_{#?}#0", "Arg max", "Arg máx", { preview: "\\operatorname*{arg\\,max}_{#?}" }), tpl("\\operatorname*{arg\\,min}_{#?}#0", "Arg min", "Arg mín", { preview: "\\operatorname*{arg\\,min}_{#?}" }),
    tpl("f\\left(#0\\right)", "Function of", "Função de", { preview: "f(#?)" }), tpl("g\\left(#0\\right)", "g of", "g de", { preview: "g(#?)" }),
    tpl("f^{-1}\\left(#0\\right)", "Inverse function", "Função inversa", { preview: "f^{-1}(#?)" }), tpl("\\left(f\\circ g\\right)\\left(#0\\right)", "Composition", "Composta", { preview: "(f\\circ g)(#?)" }),
    tpl("f:A\\to B", "f from A to B", "f de A em B"), tpl("x\\mapsto #?", "x maps to", "x leva em"),
  ],
};

export const ACCENTS: PaletteSet = {
  id: "accents",
  title: t("Accents and styles", "Acentos e estilos"),
  items: [
    over("vec", "Vector", "Vetor"), over("hat", "Hat", "Chapéu"), over("bar", "Bar", "Barra"), over("dot", "Dot", "Ponto"),
    over("ddot", "Two dots", "Dois pontos"), over("dddot", "Three dots", "Três pontos"), over("tilde", "Tilde", "Til"), over("check", "Check", "Caron"),
    over("breve", "Breve", "Bráquia"), over("acute", "Acute", "Agudo"), over("grave", "Grave", "Grave"), over("mathring", "Ring", "Anel"),
    over("widehat", "Wide hat", "Chapéu largo", "\\widehat{AB}"), over("widetilde", "Wide tilde", "Til largo", "\\widetilde{AB}"), over("overline", "Line over", "Linha em cima"), over("underline", "Line under", "Linha embaixo"),
    over("overrightarrow", "Arrow over", "Seta em cima"), over("overleftarrow", "Left arrow over", "Seta à esquerda em cima"), over("overleftrightarrow", "Double arrow over", "Seta dupla em cima"), over("underrightarrow", "Arrow under", "Seta embaixo"),
    tpl("\\overbrace{#0}^{#?}", "Brace over", "Chave em cima"), tpl("\\underbrace{#0}_{#?}", "Brace under", "Chave embaixo"), over("overgroup", "Arc over", "Arco em cima", "\\overgroup{AB}"), over("boxed", "Box", "Caixa"),
    over("cancel", "Cancel", "Cancelar"), over("bcancel", "Cancel (back)", "Cancelar (invertido)"), over("xcancel", "Cross out", "Riscar em X"), tpl("\\cancelto{#?}{#0}", "Cancel to", "Cancelar para", { preview: "\\cancelto{0}{#?}" }),
    over("mathbf", "Bold", "Negrito", "\\mathbf{Ab}"), over("boldsymbol", "Bold symbol", "Símbolo em negrito", "\\boldsymbol{\\alpha b}"), over("mathit", "Italic", "Itálico", "\\mathit{Ab}"), over("mathrm", "Upright", "Reto", "\\mathrm{Ab}"),
    over("mathsf", "Sans serif", "Sem serifa", "\\mathsf{Ab}"), over("mathtt", "Typewriter", "Máquina de escrever", "\\mathtt{Ab}"), over("mathbb", "Blackboard bold", "Duplo", "\\mathbb{AB}"), over("mathcal", "Calligraphic", "Caligráfico", "\\mathcal{AB}"),
    over("mathscr", "Script", "Manuscrito", "\\mathscr{AB}"), over("mathfrak", "Fraktur", "Fraktur", "\\mathfrak{Ab}"),
    tpl("\\textcolor{red}{#0}", "Red", "Vermelho", { preview: "\\textcolor{red}{\\blacksquare}" }), tpl("\\textcolor{blue}{#0}", "Blue", "Azul", { preview: "\\textcolor{blue}{\\blacksquare}" }),
    tpl("\\textcolor{green}{#0}", "Green", "Verde", { preview: "\\textcolor{green}{\\blacksquare}" }), tpl("\\textcolor{orange}{#0}", "Orange", "Laranja", { preview: "\\textcolor{orange}{\\blacksquare}" }),
    tpl("\\textcolor{purple}{#0}", "Purple", "Roxo", { preview: "\\textcolor{purple}{\\blacksquare}" }), tpl("\\textcolor{gray}{#0}", "Gray", "Cinza", { preview: "\\textcolor{gray}{\\blacksquare}" }),
  ],
};

export const GEOMETRY: PaletteSet = {
  id: "geometry",
  title: t("Geometry", "Geometria"),
  items: [
    sym("\\angle", "Angle", "Ângulo"), sym("\\measuredangle", "Measured angle", "Ângulo medido"), sym("\\sphericalangle", "Spherical angle", "Ângulo esférico"), sym("\\triangle", "Triangle", "Triângulo"),
    sym("\\square", "Square", "Quadrado"), sym("\\lozenge", "Rhombus", "Losango"), sym("\\bigcirc", "Circle", "Circunferência"), sym("^{\\circ}", "Degree", "Grau"),
    tpl("#?^{\\circ}", "Degrees", "Graus", { preview: "30^{\\circ}" }), tpl("{#?}'", "Minutes", "Minutos", { preview: "15'" }), sym("\\perp", "Perpendicular", "Perpendicular"), sym("\\parallel", "Parallel", "Paralelo"),
    sym("\\cong", "Congruent", "Congruente"), sym("\\sim", "Similar", "Semelhante"), tpl("\\overline{#0}", "Segment", "Segmento", { preview: "\\overline{AB}" }), tpl("\\overrightarrow{#0}", "Vector AB", "Vetor AB", { preview: "\\overrightarrow{AB}" }),
    tpl("\\overleftrightarrow{#0}", "Line", "Reta", { preview: "\\overleftrightarrow{AB}" }), tpl("\\overgroup{#0}", "Arc", "Arco", { preview: "\\overgroup{AB}" }), tpl("\\angle #?", "Angle at", "Ângulo em", { preview: "\\angle ABC" }), tpl("\\triangle #?", "Triangle named", "Triângulo de nome", { preview: "\\triangle ABC" }),
    tpl("\\left|\\overline{#0}\\right|", "Length", "Comprimento", { preview: "\\left|\\overline{AB}\\right|" }), sym("\\pi", "Pi", "Pi"), tpl("\\pi r^{2}", "Area of a circle", "Área do círculo"), tpl("2\\pi r", "Circumference", "Comprimento da circunferência"),
    tpl("\\frac{b\\,h}{2}", "Area of a triangle", "Área do triângulo"), tpl("a^{2}+b^{2}=c^{2}", "Pythagoras", "Pitágoras"), tpl("\\frac{4}{3}\\pi r^{3}", "Volume of a sphere", "Volume da esfera"), tpl("\\sin^{2}\\theta+\\cos^{2}\\theta=1", "Pythagorean identity", "Relação fundamental"),
  ],
};

export const PHYSICS: PaletteSet = {
  id: "physics",
  title: t("Physics", "Física"),
  items: [
    sym("\\hbar", "h-bar", "h cortado"), sym("\\ell", "Script l", "l cursivo"), sym("\\nabla", "Nabla", "Nabla"), sym("\\partial", "Partial", "Parcial"),
    tpl("\\vec{F}", "Force", "Força"), tpl("\\vec{v}", "Velocity", "Velocidade"), tpl("\\vec{a}", "Acceleration", "Aceleração"), tpl("\\vec{E}", "Electric field", "Campo elétrico"),
    tpl("\\vec{B}", "Magnetic field", "Campo magnético"), tpl("\\hat{\\imath}", "Unit i", "Unitário i"), tpl("\\hat{\\jmath}", "Unit j", "Unitário j"), tpl("\\hat{k}", "Unit k", "Unitário k"),
    tpl("\\dot{x}", "Velocity (dot)", "Velocidade (ponto)"), tpl("\\ddot{x}", "Acceleration (dots)", "Aceleração (pontos)"), tpl("\\left|#0\\right\\rangle", "Ket", "Ket", { preview: "\\left|\\psi\\right\\rangle" }), tpl("\\left\\langle #0\\right|", "Bra", "Bra", { preview: "\\left\\langle\\psi\\right|" }),
    tpl("\\left\\langle #0\\right\\rangle", "Expected value", "Valor esperado", { preview: "\\left\\langle A\\right\\rangle" }), tpl("\\Delta #?", "Variation", "Variação", { preview: "\\Delta t" }), tpl("\\frac{\\Delta #?}{\\Delta t}", "Rate", "Taxa", { preview: "\\frac{\\Delta s}{\\Delta t}" }), sym("^{\\circ}\\mathrm{C}", "Degrees Celsius", "Graus Celsius"),
    sym("\\Omega", "Ohm", "Ohm"), sym("\\mu", "Micro", "Micro"), tpl("\\mathrm{m/s}", "Meters per second", "Metros por segundo"), tpl("\\mathrm{m/s^{2}}", "Meters per second squared", "Metros por segundo ao quadrado"),
    tpl("\\mathrm{kg}", "Kilogram", "Quilograma"), tpl("\\mathrm{N}", "Newton", "Newton"), tpl("\\mathrm{J}", "Joule", "Joule"), tpl("\\mathrm{W}", "Watt", "Watt"),
    tpl("\\times 10^{#?}", "Times ten to the", "Vezes dez elevado a", { preview: "\\times 10^{#?}" }), tpl("F=m\\,a", "Newton's second law", "Segunda lei de Newton"), tpl("E=m\\,c^{2}", "Mass-energy", "Massa e energia"), tpl("v=v_{0}+a\\,t", "Uniform acceleration", "MRUV"),
    tpl("s=s_{0}+v_{0}t+\\frac{1}{2}a\\,t^{2}", "Position", "Função horária"), tpl("E_{c}=\\frac{m\\,v^{2}}{2}", "Kinetic energy", "Energia cinética"), tpl("U=R\\,i", "Ohm's law", "Lei de Ohm"), tpl("P\\,V=n\\,R\\,T", "Ideal gas", "Gás ideal"),
  ],
};

export const CHEMISTRY: PaletteSet = {
  id: "chemistry",
  title: t("Chemistry", "Química"),
  items: [
    { insert: "\\ce{#?}", preview: "\\ce{A + B -> C}", title: t("Reaction: type species, + and ->; ( gives the state, [ the conditions", "Reação: digite espécies, + e ->; ( dá o estado, [ as condições"), command: "ce", wide: true },
    { insert: "\\chemfig{}", preview: "\\chemfig{H-C(-[2]H)(-[6]H)-OH}", title: t("Structure: type the atom, Alt+arrow or drag for bonds", "Estrutura: digite o átomo, Alt+seta ou arraste para ligações"), command: "chemfig", wide: true },
    { insert: "\\chemfig{*6(-=-=(-R)-=)}", preview: "\\chemfig{*6(-=-=(-R)-=)}", title: t("Benzene ring with a substituent R", "Anel benzênico com substituinte R"), wide: true },
    { insert: "\\chemfig{*6(---(-R)--)}", preview: "\\chemfig{*6(---(-R)--)}", title: t("Cyclohexane with a substituent R", "Ciclo-hexano com substituinte R"), wide: true },
    { insert: "\\chemfig{*5(---(-R)-)}", preview: "\\chemfig{*5(---(-R)-)}", title: t("Cyclopentane with a substituent R", "Ciclopentano com substituinte R"), wide: true },
    { insert: "\\chemfig{R-[:30]-[:-30]-[:30]R}", preview: "\\chemfig{R-[:30]-[:-30]-[:30]R}", title: t("Carbon chain", "Cadeia carbônica"), wide: true },
    { insert: "\\chemfig{R-C(=[2]O)-R}", preview: "\\chemfig{R-C(=[2]O)-R}", title: t("Carbonyl (ketone)", "Carbonila (cetona)"), wide: true },
    { insert: "\\chemfig{R-C(=[1]O)-[7]OH}", preview: "\\chemfig{R-C(=[1]O)-[7]OH}", title: t("Carboxylic acid", "Ácido carboxílico"), wide: true },
    { insert: "\\chemfig{R-C(=[1]O)-[7]O-R}", preview: "\\chemfig{R-C(=[1]O)-[7]O-R}", title: t("Ester", "Éster"), wide: true },
    { insert: "\\chemfig{R-OH}", preview: "\\chemfig{R-OH}", title: t("Alcohol", "Álcool"), wide: true },
    { insert: "\\chemfig{R-O-R}", preview: "\\chemfig{R-O-R}", title: t("Ether", "Éter"), wide: true },
    { insert: "\\chemfig{R-N(-[2]R)-R}", preview: "\\chemfig{R-N(-[2]R)-R}", title: t("Amine", "Amina"), wide: true },
    { insert: "\\chemfig{R-C(=[1]O)-[7]N(-[6]R)-R}", preview: "\\chemfig{R-C(=[1]O)-[7]N(-[6]R)-R}", title: t("Amide", "Amida"), wide: true },
    { insert: "\\schemestart \\chemfig{} \\schemestop", preview: "\\schemestart \\chemfig{A} \\arrow \\chemfig{B} \\schemestop", title: t("Reaction scheme: structures and arrows; -> between them, [ for the conditions", "Esquema de reação: estruturas e setas; -> entre elas, [ para as condições"), command: "schemestart", wide: true },
    { insert: "\\schemestart \\chemfig{*6(-=-=(-R)-=)} \\arrow{->[][]} \\chemfig{*6(-=-=(-R)-=)} \\schemestop", preview: "\\schemestart \\chemfig{*6(-=-=(-R)-=)} \\arrow{->[$\\square$][$\\square$]} \\chemfig{*6(-=-=(-R)-=)} \\schemestop", title: t("Scheme: ring to ring with conditions", "Esquema: anel a anel com condições"), wide: true },
    { action: "block:\\pu{‸}", preview: "\\pu{123 kJ/mol}", title: t("Physical unit (mhchem \\pu)", "Unidade física (mhchem \\pu)"), command: "pu", wide: true },
    tpl("\\ce{H2O}", "Water", "Água"), tpl("\\ce{CO2}", "Carbon dioxide", "Dióxido de carbono"), tpl("\\ce{O2}", "Oxygen", "Oxigênio"), tpl("\\ce{H2SO4}", "Sulfuric acid", "Ácido sulfúrico"),
    tpl("\\ce{NaCl}", "Sodium chloride", "Cloreto de sódio"), tpl("\\ce{CH4}", "Methane", "Metano"), tpl("\\ce{NH3}", "Ammonia", "Amônia"), tpl("\\ce{C6H12O6}", "Glucose", "Glicose"),
    tpl("\\ce{H+}", "Proton", "Próton"), tpl("\\ce{OH-}", "Hydroxide", "Hidróxido"), tpl("\\ce{SO4^2-}", "Sulfate", "Sulfato"), tpl("\\ce{Fe^3+}", "Iron(III)", "Ferro(III)"),
    tpl("\\ce{->}", "Reaction arrow", "Seta de reação"), tpl("\\ce{<=>}", "Equilibrium", "Equilíbrio"), tpl("\\ce{<->}", "Resonance", "Ressonância"), tpl("\\ce{->[\\Delta]}", "Heated", "Com aquecimento"), tpl("\\ce{->[][]}", "Arrow with conditions", "Seta com condições", { preview: "\\ce{->[\\htmlData{empty=1}{$\\square$}][\\htmlData{empty=1}{$\\square$}]}" }), tpl("\\ce{<=>[][]}", "Equilibrium with conditions", "Equilíbrio com condições", { preview: "\\ce{<=>[\\htmlData{empty=1}{$\\square$}][\\htmlData{empty=1}{$\\square$}]}" }), tpl("\\ce{<-}", "Reverse arrow", "Seta inversa"), tpl("\\ce{+}", "Plus", "Mais"),
    tpl("\\ce{^}", "Gas released", "Gás liberado", { preview: "\\uparrow" }), tpl("\\ce{v}", "Precipitate", "Precipitado", { preview: "\\downarrow" }), tpl("\\ce{2H2 + O2 -> 2H2O}", "Water synthesis", "Síntese da água", { wide: true }), tpl("\\ce{CH4 + 2O2 -> CO2 + 2H2O}", "Combustion of methane", "Combustão do metano", { wide: true }),
    tpl("\\ce{H2O <=> H+ + OH-}", "Water self-ionization", "Autoionização da água", { wide: true }), tpl("\\ce{^{14}_{6}C}", "Isotope", "Isótopo"), tpl("\\mathrm{pH}=-\\log\\left[\\mathrm{H^{+}}\\right]", "pH", "pH", { wide: true }), tpl("\\Delta H", "Enthalpy change", "Variação de entalpia"),
  ],
};

const SQUARE = '\\begin{tikzcd} A \\arrow[r, "f"] \\arrow[d, "g"\'] & B \\arrow[d, "h"] \\\\ C \\arrow[r, "k"\'] & D \\end{tikzcd}';
const TRIANGLE = '\\begin{tikzcd} A \\arrow[r, "f"] \\arrow[dr, "h"\'] & B \\arrow[d, "g"] \\\\ & C \\end{tikzcd}';
const PULLBACK = '\\begin{tikzcd} P \\arrow[r] \\arrow[d] \\arrow[dr, phantom, "\\lrcorner", very near start] & X \\arrow[d, "f"] \\\\ Y \\arrow[r, "g"\'] & Z \\end{tikzcd}';

export const DIAGRAMS: PaletteSet = {
  id: "diagrams",
  title: t("Diagrams", "Diagramas"),
  items: [
    { insert: "\\begin{tikzcd}#?\\end{tikzcd}", preview: "\\begin{tikzcd} A \\arrow[r] & B \\end{tikzcd}", title: t("New diagram: type in a cell, Alt+arrow draws an arrow", "Diagrama novo: digite na célula, Alt+seta cria uma seta"), command: "tikzcd", wide: true },
    { insert: SQUARE, preview: SQUARE, title: t("Commutative square", "Quadrado comutativo"), wide: true },
    { insert: TRIANGLE, preview: TRIANGLE, title: t("Commutative triangle", "Triângulo comutativo"), wide: true },
    { insert: PULLBACK, preview: PULLBACK, title: t("Pullback", "Pullback"), wide: true },
    { insert: "\\begin{tikzcd} A \\arrow[r, shift left, \"f\"] \\arrow[r, shift right, \"g\"\'] & B \\end{tikzcd}", preview: "\\begin{tikzcd} A \\arrow[r, shift left, \"f\"] \\arrow[r, shift right, \"g\"\'] & B \\end{tikzcd}", title: t("Parallel arrows", "Setas paralelas"), wide: true },
    { insert: "\\begin{CD} A @>f>> B \\\\ @VgVV @VVhV \\\\ C @>k>> D \\end{CD}", preview: "\\begin{CD} A @>f>> B \\\\ @VgVV @VVhV \\\\ C @>k>> D \\end{CD}", title: t("Square (amscd)", "Quadrado (amscd)"), command: "CD", wide: true },
    { insert: "\\xymatrix{A \\ar[r]^{f} \\ar[d]_{g} & B \\ar[d]^{h} \\\\ C \\ar[r]_{k} & D}", preview: "\\xymatrix{A \\ar[r]^{f} \\ar[d]_{g} & B \\ar[d]^{h} \\\\ C \\ar[r]_{k} & D}", title: t("Square (xymatrix)", "Quadrado (xymatrix)"), command: "xymatrix", wide: true },
    tpl("0\\to A\\xrightarrow{f}B\\xrightarrow{g}C\\to 0", "Short exact sequence", "Sequência exata curta", { wide: true }),
    tpl("A\\xrightarrow{#?}B", "Arrow with a label", "Seta com rótulo"),
    tpl("A\\xleftarrow{#?}B", "Left arrow with a label", "Seta à esquerda com rótulo"),
    tpl("f:A\\to B", "Map", "Aplicação"),
    sym("\\hookrightarrow", "Injection", "Injeção"),
    sym("\\twoheadrightarrow", "Surjection", "Sobrejeção"),
    sym("\\cong", "Isomorphic", "Isomorfo"),
    sym("\\circ", "Composition", "Composição"),
  ],
};

export const STATISTICS: PaletteSet = {
  id: "statistics",
  title: t("Statistics", "Estatística"),
  items: [
    tpl("\\bar{x}", "Sample mean", "Média amostral"), tpl("\\hat{p}", "Estimated proportion", "Proporção estimada"), tpl("\\mu", "Population mean", "Média populacional"), tpl("\\sigma", "Standard deviation", "Desvio padrão"),
    tpl("\\sigma^{2}", "Variance", "Variância"), tpl("s^{2}", "Sample variance", "Variância amostral"), tpl("P\\left(#0\\right)", "Probability", "Probabilidade", { preview: "P(A)" }), tpl("P\\left(A\\mid B\\right)", "Conditional probability", "Probabilidade condicional"),
    tpl("E\\left[#0\\right]", "Expectation", "Esperança", { preview: "E[X]" }), tpl("\\operatorname{Var}\\left(#0\\right)", "Variance of", "Variância de", { preview: "\\operatorname{Var}(X)" }), tpl("\\operatorname{Cov}\\left(#0\\right)", "Covariance", "Covariância", { preview: "\\operatorname{Cov}(X,Y)" }), tpl("\\binom{n}{k}", "Combinations", "Combinações"),
    tpl("n!", "Factorial", "Fatorial"), tpl("A_{n,p}", "Arrangements", "Arranjos"), tpl("C_{n,p}", "Combinations (C)", "Combinações (C)"), tpl("X\\sim N\\left(\\mu,\\sigma^{2}\\right)", "Normally distributed", "Distribuição normal", { wide: true }),
    tpl("\\frac{1}{n}\\sum_{i=1}^{n}x_{i}", "Mean", "Média", { wide: true }), tpl("\\sqrt{\\frac{\\sum\\left(x_{i}-\\bar{x}\\right)^{2}}{n-1}}", "Sample standard deviation", "Desvio padrão amostral", { wide: true }), tpl("z=\\frac{x-\\mu}{\\sigma}", "z-score", "Escore z", { wide: true }), tpl("\\binom{n}{k}p^{k}\\left(1-p\\right)^{n-k}", "Binomial probability", "Probabilidade binomial", { wide: true }),
    tpl("\\frac{1}{\\sigma\\sqrt{2\\pi}}e^{-\\frac{\\left(x-\\mu\\right)^{2}}{2\\sigma^{2}}}", "Normal density", "Densidade normal", { wide: true }), tpl("\\chi^{2}", "Chi-squared", "Qui-quadrado"), tpl("H_{0}", "Null hypothesis", "Hipótese nula"), tpl("\\alpha", "Significance level", "Nível de significância"),
  ],
};

export const FORMULAS: PaletteSet = {
  id: "formulas",
  title: t("Formulas", "Fórmulas"),
  items: [
    formula("x=\\frac{-b\\pm\\sqrt{b^{2}-4ac}}{2a}", "Quadratic formula", "Fórmula de Bhaskara"),
    formula("\\Delta=b^{2}-4ac", "Discriminant", "Discriminante"),
    formula("a^{2}+b^{2}=c^{2}", "Pythagorean theorem", "Teorema de Pitágoras"),
    formula("\\left(a+b\\right)^{2}=a^{2}+2ab+b^{2}", "Square of a sum", "Quadrado da soma"),
    formula("a^{2}-b^{2}=\\left(a+b\\right)\\left(a-b\\right)", "Difference of squares", "Diferença de quadrados"),
    formula("e^{i\\pi}+1=0", "Euler's identity", "Identidade de Euler"),
    formula("e^{i\\theta}=\\cos\\theta+i\\sin\\theta", "Euler's formula", "Fórmula de Euler"),
    formula("a_{n}=a_{1}+\\left(n-1\\right)r", "Arithmetic progression", "Termo geral da PA"),
    formula("S_{n}=\\frac{\\left(a_{1}+a_{n}\\right)n}{2}", "Sum of an arithmetic progression", "Soma da PA"),
    formula("a_{n}=a_{1}\\,q^{n-1}", "Geometric progression", "Termo geral da PG"),
    formula("S_{n}=\\frac{a_{1}\\left(q^{n}-1\\right)}{q-1}", "Sum of a geometric progression", "Soma da PG"),
    formula("\\log_{a}b=\\frac{\\log_{c}b}{\\log_{c}a}", "Change of base", "Mudança de base"),
    formula("\\sin\\left(a+b\\right)=\\sin a\\cos b+\\sin b\\cos a", "Sine of a sum", "Seno da soma"),
    formula("\\frac{a}{\\sin A}=\\frac{b}{\\sin B}=\\frac{c}{\\sin C}", "Law of sines", "Lei dos senos"),
    formula("a^{2}=b^{2}+c^{2}-2bc\\cos A", "Law of cosines", "Lei dos cossenos"),
    formula("d=\\sqrt{\\left(x_{2}-x_{1}\\right)^{2}+\\left(y_{2}-y_{1}\\right)^{2}}", "Distance between points", "Distância entre pontos"),
    formula("y-y_{0}=m\\left(x-x_{0}\\right)", "Line through a point", "Equação da reta"),
    formula("\\left(x-a\\right)^{2}+\\left(y-b\\right)^{2}=r^{2}", "Circle", "Equação da circunferência"),
    formula("f'\\left(x\\right)=\\lim_{h\\to 0}\\frac{f\\left(x+h\\right)-f\\left(x\\right)}{h}", "Derivative (definition)", "Definição de derivada"),
    formula("\\int_{a}^{b}f\\left(x\\right)\\,dx=F\\left(b\\right)-F\\left(a\\right)", "Fundamental theorem of calculus", "Teorema fundamental do cálculo"),
    formula("\\sum_{n=1}^{\\infty}\\frac{1}{n^{2}}=\\frac{\\pi^{2}}{6}", "Basel problem", "Problema da Basileia"),
    formula("\\left(x+y\\right)^{n}=\\sum_{k=0}^{n}\\binom{n}{k}x^{k}y^{n-k}", "Binomial theorem", "Binômio de Newton"),
    formula("\\det\\begin{pmatrix}a&b\\\\c&d\\end{pmatrix}=ad-bc", "2×2 determinant", "Determinante 2×2"),
    formula("M=C\\left(1+i\\right)^{t}", "Compound interest", "Juros compostos"),
  ],
};

/** Every palette, in the order a toolbar shows them. */
export const PALETTES: PaletteSet[] = [COMMON, STRUCTURES, CALCULUS, MATRICES, GREEK_LETTERS, OPERATORS, RELATIONS, ARROWS, SETS, FUNCTIONS, ACCENTS];

/** The formula drawn on an item's button: its preview, or its snippet, with empty places drawn as □. */
export function previewLatex(item: PaletteItem): string {
  let latex = item.preview ?? item.insert ?? (item.action?.startsWith("block:") ? item.action.slice(6).replace("‸", "") : "");
  if (/^[\^_]/.test(latex)) latex = `#?${latex}`;
  return latex.replace(/#[?0]/g, "\\htmlData{empty=1}{\\square}");
}

/** Every command a palette names, for the command menu: the name, what it does, how it looks. */
export function commandCatalog(sets: PaletteSet[] = PALETTES): { name: string; item: PaletteItem }[] {
  const seen = new Set<string>();
  const out: { name: string; item: PaletteItem }[] = [];
  for (const set of sets)
    for (const item of set.items) {
      if (!item.command || seen.has(item.command)) continue;
      seen.add(item.command);
      out.push({ name: item.command, item });
    }
  return out;
}

/** Every item of these palettes whose name (in any language) or command contains `query`. */
export function searchPalettes(query: string, sets: PaletteSet[] = PALETTES): PaletteItem[] {
  const fold = (s: string) => s.normalize("NFD").replace(/[\u0300-\u036f]/g, "").toLowerCase();
  const q = fold(query.trim().replace(/^\\/, ""));
  if (!q) return [];
  const seen = new Set<string>();
  const out: PaletteItem[] = [];
  for (const set of sets)
    for (const item of set.items) {
      const names = typeof item.title === "string" ? [item.title] : Object.values(item.title);
      const hay = [...names, item.command ?? "", item.insert ?? ""].map(fold);
      const key = item.insert ?? item.action ?? item.picker ?? "";
      if (seen.has(key) || !hay.some((h) => h.includes(q))) continue;
      seen.add(key);
      out.push(item);
    }
  return out;
}
