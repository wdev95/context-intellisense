import initialize, { renderEquation } from "../../../media/context-equation-wasm.js";

export interface Options { display?: boolean; locale?: string; strict?: boolean; font?: number; fallbackFonts?: number[] }
export interface Result {
  svg: string;
  speech: string;
  tree: unknown;
  warnings: { rule: string; description: string }[];
  boxes: { attributes: Record<string, string>; x: number; y: number; width: number; height: number; baseline: number; level: number }[];
}

let initialized: Promise<unknown> | undefined;
export const init = (): Promise<unknown> => initialized ??= initialize();
export function render(source: string, options: Options = {}): Result {
  return JSON.parse(renderEquation(source, options.display ?? true)) as Result;
}
