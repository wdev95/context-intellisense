import { init } from "./renderer.ts";
import { ContextEquationField } from "./field.ts";
import { ContextEquationToolbar } from "./palette.ts";
import "./field.ts";
import "./palette.ts";

const STYLES = `
:host{display:flex;flex-direction:column;gap:6px;height:100%;min-height:0;min-width:0;overflow:hidden;color:var(--vscode-editor-foreground);font:13px var(--vscode-font-family)}
*{box-sizing:border-box;scrollbar-width:none}*::-webkit-scrollbar{display:none}
header{display:flex;align-items:center;gap:4px;flex:none;padding:5px 8px;border-bottom:1px solid var(--vscode-panel-border)}
header .spacer{flex:1}button{font:inherit;color:var(--vscode-button-foreground);background:var(--vscode-button-background);border:1px solid var(--vscode-button-border,transparent);border-radius:2px;padding:4px 9px;cursor:pointer}button.secondary{color:var(--vscode-button-secondaryForeground,var(--vscode-foreground));background:var(--vscode-button-secondaryBackground,transparent)}button:hover{background:var(--vscode-button-hoverBackground)}button.secondary:hover{color:var(--vscode-button-secondaryForeground,var(--vscode-foreground));background:var(--vscode-button-secondaryHoverBackground,var(--vscode-toolbar-hoverBackground))}button:focus-visible{outline:1px solid var(--vscode-focusBorder);outline-offset:1px}
context-equation-toolbar{display:block;flex:none;min-width:0;margin:0 6px;--context-equation-button-size:clamp(30px,10vw,40px)}
context-equation-toolbar[wrap] .tabs{flex-wrap:wrap;overflow:visible}
context-equation-toolbar::part(panel){max-height:none;overflow:hidden}
.stage{position:relative;flex:1;min-height:64px;min-width:0;padding:2px 8px;display:flex;overflow:hidden;align-items:center;justify-content:center}
.stage context-equation-field{display:block;width:100%;height:100%;min-height:0;font-size:2em;--context-equation-padding:0;--context-equation-align:center}
.stage context-equation-field::part(box){padding:0;border:0;border-radius:0;box-shadow:none;background:transparent}
.render-error{position:absolute;inset:8px 12px auto;padding:8px;color:var(--vscode-errorForeground);background:var(--vscode-inputValidation-errorBackground);border:1px solid var(--vscode-inputValidation-errorBorder);white-space:pre-wrap;overflow-wrap:anywhere}
.render-error[hidden]{display:none}
.source{display:grid;flex:none;gap:3px;padding:4px 8px 8px;border-top:1px solid var(--vscode-panel-border)}
.source label{font-size:11px;color:var(--vscode-descriptionForeground)}
.source textarea{width:100%;min-height:38px;max-height:120px;resize:vertical;overflow:auto;padding:5px 7px;border:1px solid var(--vscode-input-border,var(--vscode-panel-border));border-radius:2px;color:var(--vscode-input-foreground);background:var(--vscode-input-background);font:12px/1.4 var(--vscode-editor-font-family,monospace);white-space:pre-wrap;overflow-wrap:anywhere}
@media(max-height:520px){context-equation-toolbar{--context-equation-button-size:30px}.stage{min-height:48px}.source textarea{min-height:30px;max-height:64px}}
`;

export class StandaloneContextEquationEditor extends HTMLElement {
  readonly field: ContextEquationField;
  readonly toolbar: ContextEquationToolbar;
  private readonly renderError: HTMLDivElement;
  private readonly source: HTMLTextAreaElement;
  private readonly initial: { tex: string; display: boolean };
  private initialTex = "";
  private editingSource = false;

  constructor() {
    super();
    this.initial = JSON.parse(document.getElementById("init")!.textContent!);
    const root = this.attachShadow({ mode: "open" });
    root.innerHTML = `<style>${STYLES}</style><header><button class="secondary" data-action="undo" title="Undo" aria-label="Undo">↶</button><button class="secondary" data-action="redo" title="Redo" aria-label="Redo">↷</button><span class="spacer"></span><button class="secondary" data-action="reset" title="Reset equation" aria-label="Reset equation">Reset</button><button data-action="apply">Apply</button></header><context-equation-toolbar wrap locale="en" palettes="common structures calculus matrices greek operators relations arrows sets functions accents"></context-equation-toolbar><div class="stage"><context-equation-field fit locale="en" aria-label="ConTeXt equation"></context-equation-field><div class="render-error" role="alert" hidden></div></div><div class="source"><label for="tex-source">ConTeXt source</label><textarea id="tex-source" spellcheck="false" wrap="soft" aria-label="Generated ConTeXt formula"></textarea></div>`;
    this.field = root.querySelector("context-equation-field")!;
    this.field.setAttribute("value", this.initial.tex);
    this.field.contextEquationOptions = { display: this.initial.display };
    this.toolbar = root.querySelector("context-equation-toolbar")!;
    this.renderError = root.querySelector(".render-error")!;
    this.source = root.querySelector("#tex-source")!;
    this.toolbar.field = this.field;

    root.addEventListener("click", (event) => {
      const button = (event.target as HTMLElement).closest<HTMLButtonElement>("button[data-action]");
      if (!button) return;
      switch (button.dataset.action) {
        case "undo": this.field.run("undo"); break;
        case "redo": this.field.run("redo"); break;
        case "reset": this.dispatchEvent(new Event("reset", { bubbles: true })); break;
        case "apply": this.dispatchEvent(new CustomEvent("apply", { detail: this.field.latex, bubbles: true })); break;
      }
    });
    this.field.addEventListener("input", (event) => this.syncSource((event as CustomEvent<{ latex: string }>).detail.latex));
    this.field.addEventListener("context-equation-render", () => { this.renderError.hidden = true; });
    this.field.addEventListener("context-equation-render-error", (event) => {
      this.renderError.textContent = (event as CustomEvent<string>).detail;
      this.renderError.hidden = false;
    });
    this.source.addEventListener("input", () => {
      if (this.source.value !== this.field.latex) {
        this.editingSource = true;
        this.field.value = this.source.value;
        this.editingSource = false;
      }
    });
    window.addEventListener("message", event => {
      if (event.data?.type === "set") this.setEquation(event.data.tex, true);
      if (event.data?.type === "load") {
        this.field.contextEquationOptions = { ...this.field.contextEquationOptions, display: Boolean(event.data.display) };
        this.setEquation(event.data.tex, true);
      }
    });
    this.addEventListener("reset", () => this.setEquation(this.initialTex));
  }

  async connectedCallback(): Promise<void> {
    try {
      await init();
      this.initialTex = this.initial.tex;
      this.syncSource(this.initial.tex);
      this.field.focus();
    } catch (error) {
      this.renderError.textContent = error instanceof Error ? error.message : String(error);
      this.renderError.hidden = false;
    }
  }

  private setEquation(tex: string, initial = false): void {
    this.field.value = tex;
    this.syncSource(this.field.latex);
    if (initial) this.initialTex = this.field.latex;
  }

  private syncSource(tex: string): void {
    if (this.editingSource) return;
    if (this.source.value !== tex) this.source.value = tex;
    this.source.style.height = "auto";
    this.source.style.height = `${Math.min(this.source.scrollHeight, 120)}px`;
  }
}

if (!customElements.get("context-equation-editor")) customElements.define("context-equation-editor", StandaloneContextEquationEditor);
