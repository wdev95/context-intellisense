import "../vendor/context-equation/editor/standalone.ts";

const vscode = acquireVsCodeApi();
const editor = document.createElement("context-equation-editor");
document.body.append(editor);
editor.addEventListener("apply", event => vscode.postMessage({ type: "apply", tex: event.detail }));
editor.addEventListener("cancel", () => vscode.postMessage({ type: "cancel" }));
