> \[!WARNING\]  
> The code for this extension was generated entirely by AI ("vibe coding"). Review and use it at your own discretion.

# ConTeXt IntelliSense

ConTeXt IntelliSense for VS Code, focused on practical authoring support for ConTeXt projects.

Source code: [github.com/wdev95/context-intellisense](https://github.com/wdev95/context-intellisense)

## Features

*   Syntax highlighting for ConTeXt-related file types
*   Command completion based on ConTeXt XML command metadata
*   Signature help for command arguments
*   Key/value hints for setup-style commands
*   Configurable TeX/ConTeXt root path for automatic metadata and compiler discovery
*   Run buttons for current file and configured main file
*   Main file marker in Explorer via file decoration

## Project-defined commands

Add a `%D` XML docstring anywhere in a workspace TeX source. The fragment uses the same `cd:*` syntax as ConTeXt's interface files:

```tex
%D <cd:command name="ref" level="document" category="references">
%D   <cd:arguments>
%D     <cd:keywords>
%D       <cd:constant type="cd:reference"/>
%D     </cd:keywords>
%D   </cd:arguments>
%D </cd:command>
\def\ref[#1:#2]{\csname ref#1\endcsname[#1:#2]}
```

The extension reads contiguous `%D` blocks containing `<cd:command>` from workspace TeX sources and merges them with the official interface data; the block need not be adjacent to a macro definition. `cd:resolve` and `cd:inherit` work as in ConTeXt's XML. Argument types also resolve workspace collections: if `\definesym[name=sym]` and `\addsym[sym][alpha]` declare a collection, `<cd:constant type="cd:sym"/>` offers its keys. This lets dynamic commands such as `\sym` declare their argument semantics without command-specific extension code.

## Configuration

On first start, the extension looks for a `context` executable on `PATH`. When it finds a ConTeXt distribution containing `i-context.xml`, it saves that distribution's TeX root to `contextIntellisense.texRootPath`. If it cannot resolve a root, it offers a folder picker. You can change the saved root later with `ConTeXt IntelliSense: Configure TeX Root Path`.

You can re-open this setup anytime from the command palette with:

*   `ConTeXt IntelliSense: Configure TeX Root Path`
*   `ConTeXt IntelliSense: Configure Main File`

Set the following option in VS Code settings:

*   `contextIntellisense.texRootPath`: Absolute path to the root of your ConTeXt / TeX installation. The extension resolves `i-context.xml` and the ConTeXt compiler automatically below this folder.
*   `contextIntellisense.mainFilePath`: Workspace-relative path to the main `.tex` file to compile from the main-file command.
*   `contextIntellisense.openPdfAfterCompile`: Opens the generated PDF automatically after a successful compile (default: `true`).
*   `contextIntellisense.synctex`: Selects `off`, `doubleclick` (default), or `rightclick` for editor-side SyncTeX forward search.
*   `contextIntellisense.synctexmode`: Selects `API` (default) for the ConTeXt IntelliSense integration or `bridge` for the Academic PDF Viewer's built-in SyncTeX bridge (which is currently not supported as it only works with LaTeX).

The optional PDF integration targets [Academic PDF Viewer](https://marketplace.visualstudio.com/items?itemName=ovolab-veritas.academic-pdf-viewer) (`ovolab-veritas.academic-pdf-viewer`). If it is installed, the extension opens and refreshes its custom editor. The extension does not declare it as a mandatory dependency; without it, PDFs are opened with the operating system's default application.

After the TeX root is configured for the first time, the extension offers a shortcut to open the Academic PDF Viewer directly in VS Code's Extensions view.

Example (JSON settings):

```
{
  "contextIntellisense.texRootPath": "C:/path/to/texroot",
  "contextIntellisense.mainFilePath": "prd_thesis.tex",
  "contextIntellisense.synctex": "doubleclick",
  "contextIntellisense.synctexmode": "API"
}
```

The editor provides two run actions for ConTeXt `.tex` files:

*   `Compile Current File`
*   `Compile Main File`

After a successful compile, the generated PDF is opened automatically by default. Disable `contextIntellisense.openPdfAfterCompile` to turn this off. An already open PDF in the Academic PDF Viewer is refreshed in place; for the system default PDF viewer, the extension invokes the registered PDF handler after each compile.

## Install (From Release VSIX)

1.  Download the latest VSIX from the GitHub Release.
2.  Open VS Code command palette.
3.  Run: `Extensions: Install from VSIX...`
4.  Select the downloaded VSIX file.

## Install (CLI Helper)

You can also use the helper script included in this repository:

```
node install-vsix.js
```

It automatically picks the newest matching VSIX in the project folder and installs it via the VS Code CLI.

## Local Build

```
node build-vsix.js
```

Behavior:

*   Bumps patch version in `package.json`
*   Builds a new VSIX package
*   Reverts version bump automatically if build fails

## Automated GitHub Release

The workflow in `.github/workflows/release-vsix.yml` can be started manually from GitHub Actions. It requires the desired release version as an explicit `X.X.X` input (for example `1.2.3`).

It performs:

1.  Validation and application of the manually entered version
2.  Commit + tag (`v<version>`)
3.  GitHub Release creation
4.  Upload of release assets:
    *   generated VSIX
    *   `install-vsix.js`

## Requirements

*   VS Code 1.80+
*   Node.js 24+ for local builds and releases
