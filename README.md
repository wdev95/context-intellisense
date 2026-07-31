# ConTeXt IntelliSense

ConTeXt IntelliSense for VS Code, focused on practical authoring support for ConTeXt projects.

## Features

- Syntax highlighting for ConTeXt-related file types
- Command completion based on ConTeXt XML command metadata
- Signature help for command arguments
- Key/value hints for setup-style commands
- Configurable XML source path for completion data

## Configuration

Set the following option in VS Code settings:

- `contextIntellisense.xmlPath`: Absolute path to `context-en.xml` used to generate completion and signature data.

Example (JSON settings):

```json
{
  "contextIntellisense.xmlPath": "C:/path/to/context-en.xml"
}
```

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

The workflow in `.github/workflows/release-vsix.yml` can be started manually from GitHub Actions.

It performs:

1. Patch version bump via `build-vsix.js`
2. Commit + tag (`v<version>`)
3. GitHub Release creation
4. Upload of release assets:
   - generated VSIX
   - `install-vsix.js`

## Requirements

- VS Code 1.80+
- Node.js 20+ recommended
