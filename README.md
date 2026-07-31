> [!WARNING]
> The code for this extension was generated entirely by AI ("vibe coding"). Review and use it at your own discretion.

# ConTeXt IntelliSense

ConTeXt IntelliSense for VS Code, focused on practical authoring support for ConTeXt projects.

Source code: [github.com/wdev95/context-intellisense](https://github.com/wdev95/context-intellisense)

## Features

- Syntax highlighting for ConTeXt-related file types
- Command completion based on ConTeXt XML command metadata
- Signature help for command arguments
- Key/value hints for setup-style commands
- Configurable TeX/ConTeXt root path for automatic metadata and compiler discovery
- Run buttons for current file and configured main file
- Main file marker in Explorer via file decoration

## Configuration

On first start, if no valid TeX root path can be resolved automatically, the extension opens a setup dialog and lets you pick your ConTeXt / TeX installation folder.

You can re-open this setup anytime from the command palette with:

- `ConTeXt IntelliSense: Configure TeX Root Path`
- `ConTeXt IntelliSense: Configure Main File`

Set the following option in VS Code settings:

- `contextIntellisense.texRootPath`: Absolute path to the root of your ConTeXt / TeX installation. The extension finds `context-en.xml` and the ConTeXt compiler automatically below this folder.
- `contextIntellisense.mainFilePath`: Absolute path to the main `.tex` file to compile from the main-file command.

Example (JSON settings):

```json
{
  "contextIntellisense.texRootPath": "C:/path/to/texroot",
  "contextIntellisense.mainFilePath": "C:/path/to/prd_thesis.tex"
}
```

The editor provides two run actions for ConTeXt `.tex` files:

- `Compile Current File`
- `Compile Main File`

After a successful compile, the generated PDF is opened in a split editor when a PDF viewer extension is installed. Otherwise the file is opened with the operating system's default PDF application.

## Install (From Release VSIX)

1. Download the latest VSIX from the GitHub Release.
2. Open VS Code command palette.
3. Run: `Extensions: Install from VSIX...`
4. Select the downloaded VSIX file.

## Install (CLI Helper)

You can also use the helper script included in this repository:

```bash
node install-vsix.js
```

It automatically picks the newest matching VSIX in the project folder and installs it via the VS Code CLI.

## Local Build

```bash
node build-vsix.js
```

Behavior:

- Bumps patch version in `package.json`
- Builds a new VSIX package
- Reverts version bump automatically if build fails

## Automated GitHub Release

The workflow in `.github/workflows/release-vsix.yml` can be started manually from GitHub Actions. It requires the desired release version as an explicit `X.X.X` input (for example `1.2.3`).

It performs:

1. Validation and application of the manually entered version
2. Commit + tag (`v<version>`)
3. GitHub Release creation
4. Upload of release assets:
   - generated VSIX
   - `install-vsix.js`

## Requirements

- VS Code 1.80+
- Node.js 20+ recommended
