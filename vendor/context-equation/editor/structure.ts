import { chemToLatex, latexToChem } from "./reaction.ts";

// Structures (chemfig's \chemfig{…}) edited in place: contextEquation reads the code into atoms and bonds and
// says where each is written (its span); every edit here is a splice of the code at those places, so
// what the person wrote stays as written everywhere else.

/** An atom: its label as written (empty for a carbon at a vertex) and where it is written. */
export interface ChemAtom { label: string; span: [number, number] | null }
/** A bond: its atoms, its kind (contextEquation's name), its direction in degrees, where it is written. */
export interface ChemBond { from: number; to: number; kind: string; angle: number; span: [number, number] | null }
export interface Graph { atoms: ChemAtom[]; bonds: ChemBond[] }

/** The bonds the editor makes, by name, and how chemfig writes them. */
export const BOND_SIGNS: Record<string, string> = { single: "-", double: "=", triple: "~", wedge: ">", hash: ">:", hollow: ">|", wedge_back: "<", hash_back: "<:", hollow_back: "<|", double_left: "=^", double_right: "=_" };
const SIGN = /^(?:<:|>:|<\||>\||=\^|=_|[-=~<>])/;

/** The structure contextEquation read from `\chemfig{body}`: from its tree (the first structure in it). */
export function readGraph(tree: unknown): Graph | null {
  let found: Graph | null = null;
  const walk = (v: unknown): void => {
    if (found || !v || typeof v !== "object") return;
    const o = v as Record<string, unknown>;
    if (o.kind === "structure" && Array.isArray(o.atoms) && Array.isArray(o.bonds)) {
      found = {
        atoms: (o.atoms as { label?: string; span?: [number, number] }[]).map((a) => ({ label: a.label ?? "", span: a.span ?? null })),
        bonds: (o.bonds as { from: number; to: number; kind: string; angle?: number; span?: [number, number] }[]).map((b) => ({ from: b.from, to: b.to, kind: b.kind, angle: b.angle ?? 0, span: b.span ?? null })),
      };
      return;
    }
    for (const x of Object.values(o)) walk(x);
  };
  walk(tree);
  return found;
}

/** Can every atom be found in the code? (Not after a submolecule, which rewrites it.) */
export function editable(g: Graph): boolean {
  return g.atoms.every((a) => a.span !== null);
}

const chars = (s: string) => Array.from(s);

function splice(body: string, at: number, end: number, text: string): string {
  const c = chars(body);
  c.splice(at, end - at, ...chars(text));
  return c.join("");
}

/** The code with a mark around each atom's label (an empty one at a vertex), for the editor to find them. */
export function markBody(body: string, g: Graph, id: (k: number) => string): string {
  const c = chars(body);
  const order = g.atoms.map((a, k) => ({ a, k })).filter((x) => x.a.span).sort((x, y) => y.a.span![0] - x.a.span![0]);
  const seen = new Set<number>();
  for (const { a, k } of order) {
    const [s, e] = a.span!;
    // a label read twice (a ring closing on its start names it again) is marked once
    if (seen.has(s)) continue;
    seen.add(s);
    const label = c.slice(s, e).join("");
    c.splice(s, e - s, ...chars(`\\htmlData{m=${id(k)}}{${label}}`));
  }
  return c.join("");
}

/** A new label for atom `k`. */
export function setLabel(body: string, g: Graph, k: number, label: string): string {
  const span = g.atoms[k]?.span;
  if (!span) return body;
  // a label next to a bond sign or a ring needs nothing around it; one with a space would end at it
  return splice(body, span[0], span[1], label.replace(/\s+/g, ""));
}

/** chemfig's absolute angle for a direction in degrees. */
function angleArg(angle: number): string {
  const a = Math.round(((angle % 360) + 540) % 360 - 180);
  return `[:${a}]`;
}

/** A bond from atom `k` in direction `angle`, to a new atom labelled `label` (empty: a carbon at a
 * vertex), written as a branch right after the atom's label. Returns the code and where the new label
 * starts, to find the new atom by. */
export function addBond(body: string, g: Graph, k: number, angle: number, kind = "single", label = ""): { body: string; at: number } {
  const span = g.atoms[k]?.span;
  if (!span) return { body, at: -1 };
  const sign = `${BOND_SIGNS[kind] ?? "-"}${angleArg(angle)}`;
  // the last atom of the main chain takes the bond on; any other a branch
  const c = chars(body);
  const last = !c.slice(span[1]).join("").trim();
  const text = last ? `${sign}${label}` : `(${sign}${label})`;
  return { body: splice(body, span[1], span[1], text), at: span[1] + (last ? 0 : 1) + chars(sign).length };
}

/** A bond's kind changed in place (its angle and the rest of its `[…]` kept). */
export function setBondKind(body: string, g: Graph, b: number, kind: string): string {
  const span = g.bonds[b]?.span;
  if (!span) return body;
  const c = chars(body);
  const text = c.slice(span[0], span[1]).join("");
  const m = SIGN.exec(text);
  if (!m) return body;
  return splice(body, span[0], span[0] + chars(m[0]).length, BOND_SIGNS[kind] ?? "-");
}

/** Removes atom `k` with the bond that reaches it, when nothing hangs from it (a leaf); otherwise
 * only clears its label (a carbon at a vertex). The first atom is never removed. */
export function removeAtom(body: string, g: Graph, k: number): string {
  const atom = g.atoms[k];
  if (!atom?.span) return body;
  const bonds = g.bonds.filter((b) => b.from === k || b.to === k);
  const incoming = g.bonds.find((b) => b.to === k && b.span);
  if (k === 0 || bonds.length !== 1 || !incoming?.span || incoming.span[1] > atom.span[0]) return setLabel(body, g, k, "");
  let start = incoming.span[0];
  let end = atom.span[1];
  const c = chars(body);
  // a branch left empty goes too: (-X)
  if (c[start - 1] === "(" && c[end] === ")") {
    start--;
    end++;
  }
  return splice(body, start, end, "");
}

/** A ring hanging from atom `k`: `*6(------)`, aromatic `*6(-=-=-=)`; cyclopentane `*5(-----)`. */
export function addRing(body: string, g: Graph, k: number, size: number, aromatic: boolean): string {
  const span = g.atoms[k]?.span;
  if (!span) return body;
  let bonds = "";
  for (let i = 0; i < size; i++) bonds += aromatic && i % 2 === 1 ? "=" : "-";
  const ring = `*${size}(${bonds})`;
  // an atom with bonds: the ring hangs from a new bond, away from them (on the atom itself it would lie over them)
  const touching = g.bonds.filter((b) => b.from === k || b.to === k);
  // with no bond, or only the one that reaches it, chemfig turns the ring the bond's way: on the atom
  if (!touching.length || (touching.length === 1 && touching[0].to === k)) return splice(body, span[1], span[1], ring);
  let sx = 0;
  let sy = 0;
  for (const b of touching) {
    const a = ((b.from === k ? b.angle : b.angle + 180) * Math.PI) / 180;
    sx += Math.cos(a);
    sy += Math.sin(a);
  }
  // bonds that cancel out (a chain through the atom): across them
  const away = Math.hypot(sx, sy) < 0.3 ? touching[0].angle + 90 : (Math.atan2(-sy, -sx) * 180) / Math.PI;
  return addBond(body, g, k, Math.round(away / 30) * 30, "single", ring).body;
}

/** The atom whose label starts at `at` (after an edit), or -1. */
export function atomAt(g: Graph, at: number): number {
  return g.atoms.findIndex((a) => a.span && a.span[0] === at);
}

/** A label as typed in a free box, made safe to write in chemfig's code: the characters that would
 * end it (bonds, branches, rings, spaces) go, except a sign right after `^` (a charge, `O^-`). */
export function cleanLabel(text: string): string {
  let out = "";
  for (const ch of chars(text)) {
    if (/[-+]/.test(ch) && out.endsWith("^")) out += ch;
    else if (!/[-=~<>()*!?@\[\],\s]/.test(ch)) out += ch;
  }
  return out;
}

/** A label as typed (as chemistry is written: `NH3`, `NH4^+`, `H_3C`) as chemfig's code writes it:
 * a number after an element is its index, `^` a charge, each a group of its own (`NH_3`, `NH_4^+`),
 * so nothing typed half-way (`NH_`) takes in the bond after it. */
export function chemfigLabel(text: string): string {
  return chemToLatex(text)
    .replace(/_\{([^{}])\}/g, "_$1")
    .replace(/\^\{([^{}])\}/g, "^$1");
}

/** A label of chemfig's code as it is typed back: `NH_3` is `NH3`, `NH_4^+` is `NH4^+`. */
export function labelText(label: string): string {
  // an index or a charge begun and still empty stays begun (`O^`, then `O^-`)
  return latexToChem(label.replace(/_([0-9])/g, "_{$1}").replace(/\^([^{}\s])/g, "^{$1}").replace(/([_^])\{\}/g, "$1\u0000")).replace(/\u0000/g, "");
}
