// From the boxes contextEquation returns for the marked LaTeX to where the cursor is drawn and where a click
// lands. Everything in the SVG's own coordinates (its viewBox, y down).

import { childRows, markedRun, type Atom, type Cursor, type Diagram, type DiagramArrow, type Editor, type Row, type Selection } from "./model.ts";

/** A box as `render(…).boxes` gives it (only the fields used here). */
export interface Box {
  attributes: Record<string, string>;
  x: number;
  y: number;
  width: number;
  height: number;
  baseline: number;
}

export interface Rect {
  x: number;
  y: number;
  width: number;
  height: number;
}

/** The boxes by the id of what they mark (`data-m`). */
export function boxesById(boxes: Box[]): Map<string, Box> {
  const map = new Map<string, Box>();
  for (const b of boxes) {
    const id = b.attributes["data-m"];
    if (id) map.set(id, b);
  }
  return map;
}

function union(rects: Rect[]): Rect | null {
  if (!rects.length) return null;
  const x0 = Math.min(...rects.map((r) => r.x));
  const y0 = Math.min(...rects.map((r) => r.y));
  const x1 = Math.max(...rects.map((r) => r.x + r.width));
  const y1 = Math.max(...rects.map((r) => r.y + r.height));
  return { x: x0, y: y0, width: x1 - x0, height: y1 - y0 };
}

/** Where an atom is drawn. A script has no box of its own (it is where its rows are), nor small
 * delimiters (each side has one, `<id>l` and `<id>r`). */
export function atomRect(atom: Atom, boxes: Map<string, Box>): Rect | null {
  const own = boxes.get(atom.id);
  if (own) return own;
  const inNumber = numberPart(atom, boxes);
  if (inNumber) return inNumber;
  const inPiece = piecePart(atom, boxes);
  if (inPiece) return inPiece;
  const arrow = schemeArrow(atom, boxes);
  if (arrow) return arrow;
  // a structure in a scheme carries no mark around it: where its atoms are
  if (atom.kind === "raw") {
    const atoms = [...boxes.entries()].filter(([id]) => id.startsWith(`${atom.id}a`) && /^\d+$/.test(id.slice(atom.id.length + 1))).map(([, b]) => b);
    if (atoms.length) return union(atoms);
  }
  const parts: Rect[] = [];
  for (const id of [`${atom.id}l`, `${atom.id}r`]) {
    const b = boxes.get(id);
    if (b) parts.push(b);
  }
  for (const r of childRows(atom)) {
    const b = rowRect(r, boxes);
    if (b) parts.push(b);
  }
  return union(parts);
}

/** A digit or separator of a number drawn as one group (`<first id>n`): its share of the group's
 * width, a separator taking half a digit (as in the math fonts, whose digits all have one width). */
function numberPart(atom: Atom, boxes: Map<string, Box>): Rect | null {
  const row = atom.parent;
  if (!row) return null;
  const atoms = row.children;
  const k = atoms.indexOf(atom);
  for (let i = k; i >= 0; i--) {
    const group = boxes.get(`${atoms[i].id}n`);
    if (!group) continue;
    const run = markedRun(atoms, i, true) ?? markedRun(atoms, i, false);
    if (!run || !run.includes(atom)) return null;
    const weight = (a: Atom) => (a.kind === "symbol" && /^[0-9]$/.test(a.latex) ? 1 : 0.5);
    const total = run.reduce((s, a) => s + weight(a), 0);
    const before = run.slice(0, run.indexOf(atom)).reduce((s, a) => s + weight(a), 0);
    const unit = group.width / total;
    return { x: group.x + before * unit, y: group.y, width: weight(atom) * unit, height: group.height };
  }
  return null;
}

/** An arrow of a scheme, which carries no mark of its own: the space between the pieces around it,
 * as high as they are (its labels' boxes, when it has them, widen it). */
function schemeArrow(atom: Atom, boxes: Map<string, Box>): Rect | null {
  const row = atom.parent;
  if (atom.kind !== "rxarrow" || row?.parent?.kind !== "reaction" || !row.parent.scheme || boxes.has(atom.id)) return null;
  const i = row.children.indexOf(atom);
  const near = (step: number) => {
    for (let k = i + step; k >= 0 && k < row.children.length; k += step) {
      const a = row.children[k];
      if (a.kind === "rxarrow") continue;
      const r = boxes.get(a.id) ?? (a.kind === "raw" ? union([...boxes.entries()].filter(([id]) => id.startsWith(`${a.id}a`)).map(([, b]) => b)) : null);
      if (r) return r;
    }
    return null;
  };
  const l = near(-1);
  const r = near(+1);
  const labels = [atom.above, atom.below].map((x) => boxes.get(x.id)).filter((b): b is Box => !!b);
  if (!l && !r && !labels.length) return null;
  // with nothing on one side, an arrow's usual length from the other
  const usual = 1600;
  const x0 = l ? l.x + l.width : labels.length ? Math.min(...labels.map((b) => b.x)) : r!.x - usual;
  const x1 = r ? r.x : labels.length ? Math.max(...labels.map((b) => b.x + b.width)) : x0 + usual;
  const ys = [l, r, ...labels].filter((b): b is Rect => !!b);
  const y0 = Math.min(...ys.map((b) => b.y));
  const y1 = Math.max(...ys.map((b) => b.y + b.height));
  return { x: Math.min(x0, x1), y: y0, width: Math.abs(x1 - x0), height: y1 - y0 };
}

/** Is this atom inside a species of a reaction (its formula, an index, a charge)? */
function inSpecies(atom: Atom): boolean {
  for (let r = atom.parent; r?.parent; r = r.parent.parent) {
    if (r.parent.kind === "species") return true;
    if (r.parent.kind === "reaction") return false;
  }
  return false;
}

/** How wide a piece of chemistry is drawn, in a capital letter's widths (near enough to place a cursor). */
function chemWeight(a: Atom): number {
  const t = a.kind === "symbol" ? a.latex : a.kind === "char" ? a.ch : null;
  if (t !== null) {
    if (/^[A-Z]$/.test(t)) return 1.2;
    if (/^[0-9+\-]$/.test(t)) return 1;
    if (/^[()\[\]]$/.test(t)) return 0.6;
    return 0.85;
  }
  if (a.kind === "scripts") return 0.6 * Math.max(rowWeight(a.sub), rowWeight(a.sup));
  // a command among conditions (\text{light}, $\alpha$): about as wide as what it shows
  if (a.kind === "raw") return Math.max(1, a.latex.replace(/\\(text|mathrm|qty|SI|ce)\b/g, "").replace(/\\[A-Za-z]+/g, "x").replace(/[{}$\\]/g, "").length * 0.85);
  if (a.kind === "delim") return rowWeight(a.body) + 1.2;
  return 1;
}

function rowWeight(r: Row | null): number {
  return r?.children.length ? r.children.reduce((t, a) => t + chemWeight(a), 0) : 0.6;
}

/** A row of chemistry mhchem draws as one group (a species, an arrow's conditions), or a row inside
 * a species: where it is, from the group's box and each piece's width. */
function chemRowRect(row: Row, boxes: Map<string, Box>): Rect | null {
  const owner = row.parent;
  if (owner?.kind === "species" || owner?.kind === "rxarrow") return boxes.get(row.id) ?? null;
  if (!owner || !inSpecies(owner)) return null;
  const r = piecePart(owner, boxes);
  if (!r) return null;
  // an index low, a charge high; the inside of parentheses between them
  if (owner.kind === "scripts") {
    const h = r.height * 0.6;
    return { x: r.x, y: row === owner.sup ? r.y : r.y + r.height - h, width: r.width, height: h };
  }
  const inset = owner.kind === "delim" ? (r.width * 0.6) / chemWeight(owner) : 0;
  return { x: r.x + inset, y: r.y, width: r.width - 2 * inset, height: r.height };
}

/** A piece of a reaction's species or conditions (mhchem draws each as one group): its share of the
 * row's width. */
function piecePart(atom: Atom, boxes: Map<string, Box>): Rect | null {
  const row = atom.parent;
  if (!row || !(inSpecies(atom) || row.parent?.kind === "rxarrow")) return null;
  const box = chemRowRect(row, boxes);
  if (!box) return null;
  const total = row.children.reduce((t, a) => t + chemWeight(a), 0) || 1;
  const before = row.children.slice(0, row.children.indexOf(atom)).reduce((t, a) => t + chemWeight(a), 0);
  const unit = box.width / total;
  return { x: box.x + before * unit, y: box.y, width: chemWeight(atom) * unit, height: box.height };
}

/** Where a row is drawn: its own box, or where its atoms are; an empty cell of a diagram, by its neighbours. */
export function rowRect(row: Row, boxes: Map<string, Box>): Rect | null {
  const r = boxes.get(row.id) ?? union(row.children.map((a) => atomRect(a, boxes)).filter((r): r is Rect => !!r));
  if (r) return r;
  // an empty index or charge in a species, empty parentheses
  const chem = row.parent && inSpecies(row.parent) ? chemRowRect(row, boxes) : null;
  if (chem) return chem;
  const d = row.parent;
  // an empty place over or under an arrow (not drawn): its half of the arrow
  if (d?.kind === "rxarrow") {
    const a = boxes.get(d.id);
    if (!a) return null;
    const half = a.height / 2;
    return { x: a.x, y: row === d.above ? a.y : a.y + half, width: a.width, height: half };
  }
  if (d?.kind === "diagram") {
    for (let i = 0; i < d.cells.length; i++) {
      const j = d.cells[i].indexOf(row);
      if (j >= 0) return cellRect(d, i, j, boxes);
    }
  }
  return null;
}

/** Where a cell of a diagram is: its own box, or (empty, unmarked) at its line's height and its
 * column's place, found from the other cells. */
export function cellRect(d: Diagram, i: number, j: number, boxes: Map<string, Box>): Rect | null {
  const own = (row: Row) => boxes.get(row.id) ?? union(row.children.map((a) => atomRect(a, boxes)).filter((r): r is Rect => !!r));
  const r = own(d.cells[i][j]);
  if (r) return r;
  const rows = d.cells.length;
  const cols = d.cells[0]?.length ?? 0;
  const centre = (axis: "x" | "y", k: number): number | null => {
    const found: number[] = [];
    for (let n = 0; n < (axis === "x" ? rows : cols); n++) {
      const b = own(axis === "x" ? d.cells[n][k] : d.cells[k][n]);
      if (b) found.push(axis === "x" ? b.x + b.width / 2 : b.y + b.height / 2);
    }
    return found.length ? found.reduce((s, v) => s + v, 0) / found.length : null;
  };
  // known centres of every column and line; the missing ones in step with their neighbours
  const fill = (axis: "x" | "y", n: number, k: number): number | null => {
    const known = Array.from({ length: n }, (_, m) => centre(axis, m));
    if (known[k] !== null) return known[k];
    const idx = known.map((v, m) => (v === null ? -1 : m)).filter((m) => m >= 0);
    if (!idx.length) return null;
    // one known: tikz-cd's spacing, about 1.9 em from line to line and 2.4 em from column to column
    if (idx.length === 1) return known[idx[0]]! + (k - idx[0]) * (axis === "y" ? 1911 : 2400);
    const step = (known[idx[idx.length - 1]]! - known[idx[0]]!) / (idx[idx.length - 1] - idx[0]);
    const near = idx.reduce((a, b) => (Math.abs(b - k) < Math.abs(a - k) ? b : a));
    return known[near]! + (k - near) * step;
  };
  const x = fill("x", cols, j);
  const y = fill("y", rows, i);
  if (x === null || y === null) return null;
  return { x: x - 250, y: y - 350, width: 500, height: 700 };
}

/** An arrow of a diagram as a path, from the edge of one cell to the edge of the other (a bend
 * bulges to the side the bend names): a few points along it, for hit-testing and highlighting. */
export function arrowPath(d: Diagram, a: DiagramArrow, boxes: Map<string, Box>): { x: number; y: number }[] | null {
  const A = cellRect(d, a.from[0], a.from[1], boxes);
  const B = cellRect(d, a.to[0], a.to[1], boxes);
  if (!A || !B) return null;
  const ax = A.x + A.width / 2, ay = A.y + A.height / 2;
  const bx = B.x + B.width / 2, by = B.y + B.height / 2;
  const len = Math.hypot(bx - ax, by - ay) || 1;
  const ux = (bx - ax) / len, uy = (by - ay) / len;
  // leave each cell at its edge
  const cut = (r: Rect) => Math.min(Math.abs(r.width / 2 / (ux || 1e-9)), Math.abs(r.height / 2 / (uy || 1e-9))) + 60;
  // `shift left` / `shift right` (twice with `=2`): parallel arrows moved off the line between the cells,
  // tikz-cd's 0.56ex each, to the left of the direction of travel for `left`
  let shift = 0;
  for (const x of a.extra) {
    const m = /^shift (left|right)(?:\s*=\s*([0-9.]+))?$/.exec(x.trim());
    if (m) shift += (m[1] === "left" ? 1 : -1) * 240 * Number(m[2] ?? 1);
  }
  const sx = ax + ux * cut(A) + uy * shift, sy = ay + uy * cut(A) - ux * shift;
  const ex = bx - ux * cut(B) + uy * shift, ey = by - uy * cut(B) - ux * shift;
  // tikz-cd's bend is 30 degrees: the middle moves out by about a quarter of the length, to the left
  // of the direction of travel for a left bend (above a rightward arrow: y grows down)
  const bulge = a.bend ? (a.bend === "left" ? 1 : -1) * len * 0.27 : 0;
  const cx = (sx + ex) / 2 + uy * bulge, cy = (sy + ey) / 2 - ux * bulge;
  const pts: { x: number; y: number }[] = [];
  for (let t = 0; t <= 1.0001; t += 0.1) {
    const m = 1 - t;
    pts.push({ x: m * m * sx + 2 * m * t * cx + t * t * ex, y: m * m * sy + 2 * m * t * cy + t * t * ey });
  }
  return pts;
}

/** The arrow of a diagram near (x, y), if any. */
export function arrowAt(editor: Editor, boxes: Map<string, Box>, x: number, y: number, reach = 160): { diagram: Diagram; index: number } | null {
  let best: { diagram: Diagram; index: number } | null = null;
  let bestDist = reach;
  const visit = (row: Row) => {
    for (const a of row.children) {
      if (a.kind === "diagram")
        a.arrows.forEach((arrow, index) => {
          const pts = arrowPath(a, arrow, boxes);
          if (!pts) return;
          for (let k = 1; k < pts.length; k++) {
            const d = segmentDistance(x, y, pts[k - 1], pts[k]);
            if (d < bestDist) {
              bestDist = d;
              best = { diagram: a, index };
            }
          }
        });
      for (const r of childRows(a)) visit(r);
    }
  };
  visit(editor.root);
  return best;
}

/** The cell of a diagram at (x, y), if any (empty cells too). */
export function cellAt(editor: Editor, boxes: Map<string, Box>, x: number, y: number): { diagram: Diagram; i: number; j: number } | null {
  let found: { diagram: Diagram; i: number; j: number } | null = null;
  const visit = (row: Row) => {
    for (const a of row.children) {
      if (a.kind === "diagram")
        a.cells.forEach((line, i) =>
          line.forEach((_, j) => {
            const r = cellRect(a, i, j, boxes);
            if (r && x >= r.x - 120 && x <= r.x + r.width + 120 && y >= r.y - 120 && y <= r.y + r.height + 120) found = { diagram: a, i, j };
          }),
        );
      for (const r of childRows(a)) visit(r);
    }
  };
  visit(editor.root);
  return found;
}

function segmentDistance(x: number, y: number, p: { x: number; y: number }, q: { x: number; y: number }): number {
  const dx = q.x - p.x, dy = q.y - p.y;
  const t = Math.max(0, Math.min(1, ((x - p.x) * dx + (y - p.y) * dy) / (dx * dx + dy * dy || 1)));
  return Math.hypot(x - (p.x + t * dx), y - (p.y + t * dy));
}

/** Where the selection is: one rectangle over the selected atoms. */
export function selectionRect(selection: Selection | null, boxes: Map<string, Box>): Rect | null {
  if (!selection) return null;
  return union(selection.row.children.slice(selection.start, selection.end).map((a) => atomRect(a, boxes)).filter((r): r is Rect => !!r));
}

/** The cursor as a thin rectangle: at the left edge of the atom after it (or the right edge of the
 * last one), as tall as the symbol beside it; in an empty place, in the middle of the placeholder. */
export function caretRect(cursor: Cursor, boxes: Map<string, Box>, thickness = 40): Rect | null {
  const { row, offset } = cursor;
  const rb = rowRect(row, boxes);
  if (!rb) return null;
  if (row.children.length === 0) return { x: rb.x + rb.width / 2 - thickness / 2, y: rb.y, width: thickness, height: rb.height };
  const after = row.children[offset];
  const before = row.children[offset - 1];
  // an atom without a box (LaTeX that acts on the alignment, kept unmarked): the neighbour's edge
  const leftEdge = after ? atomRect(after, boxes)?.x : undefined;
  const rightOfBefore = before ? (() => {
    const r = atomRect(before, boxes);
    return r ? r.x + r.width : undefined;
  })() : undefined;
  const x = leftEdge ?? rightOfBefore ?? rb.x;
  // the height of a neighbour symbol, so the cursor beside a tall fraction stays the size of a letter
  const neighbour = [before, after].find((a) => a && (a.kind === "symbol" || a.kind === "char")) ?? before ?? after;
  const nb = neighbour ? atomRect(neighbour, boxes) : null;
  const v = nb && (neighbour!.kind === "symbol" || neighbour!.kind === "char") && nb.height > 0 ? nb : rb;
  return { x: x - thickness / 2, y: v.y, width: thickness, height: v.height };
}

function contains(r: Rect, x: number, y: number): boolean {
  return x >= r.x && x <= r.x + r.width && y >= r.y && y <= r.y + r.height;
}

/** The innermost block of LaTeX (a raw atom) drawn at (x, y), if any. */
export function blockAt(editor: Editor, boxes: Map<string, Box>, x: number, y: number): Atom | null {
  let best: Atom | null = null;
  let area = Infinity;
  for (const row of editor.rows())
    for (const a of row.children) {
      if (a.kind !== "raw") continue;
      const r = atomRect(a, boxes);
      if (r && contains(r, x, y) && r.width * r.height < area) {
        best = a;
        area = r.width * r.height;
      }
    }
  return best;
}

/** Where a click at (x, y) puts the cursor: in the innermost row whose box holds the point (the
 * formula itself if none), before the first atom whose middle is right of the point. */
export function cursorAt(editor: Editor, boxes: Map<string, Box>, x: number, y: number): Cursor {
  let best: Row = editor.root;
  let bestArea = Infinity;
  for (const row of editor.rows()) {
    const r = rowRect(row, boxes);
    if (!r || !contains(r, x, y) || row === editor.root) continue;
    const area = r.width * r.height;
    if (area <= bestArea) {
      best = row;
      bestArea = area;
    }
  }
  const children = best.children;
  let offset = children.length;
  for (let i = 0; i < children.length; i++) {
    const r = atomRect(children[i], boxes);
    if (r && x < r.x + r.width / 2) {
      offset = i;
      break;
    }
  }
  return { row: best, offset };
}
