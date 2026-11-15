const path = require('path');
const { spawnSync } = require('child_process');
const esbuild = require('esbuild');
const fs = require('fs');

const root = path.resolve(__dirname, '..');
const targetDir = path.join(root, 'out', 'context-equation');
const media = path.join(root, 'media');
const wasm = path.join(targetDir, 'wasm32-unknown-unknown', 'wasm', 'context_equation_wasm.wasm');
const cargoBin = path.join(process.env.USERPROFILE || process.env.HOME || '', '.cargo', 'bin');
const env = { ...process.env, CARGO_TARGET_DIR: targetDir, PATH: `${cargoBin}${path.delimiter}${process.env.PATH || ''}` };

if (process.platform === 'win32') {
  const sdkRoot = [path.join(env.WindowsSdkDir || '', 'Lib'), env.WindowsSdkDir, 'C:\\Program Files (x86)\\Windows Kits\\10\\Lib']
    .find((folder) => folder && fs.existsSync(folder));
  const sdkVersion = env.WindowsSDKVersion?.replace(/[\\/]$/, '') || fs.readdirSync(sdkRoot).filter((name) => /^10\./.test(name)).sort().at(-1);
  const vswhere = 'C:\\Program Files (x86)\\Microsoft Visual Studio\\Installer\\vswhere.exe';
  const vsRoot = fs.existsSync(vswhere)
    ? spawnSync(vswhere, ['-latest', '-products', '*', '-property', 'installationPath'], { encoding: 'utf8' }).stdout?.trim() || ''
    : '';
  const installations = ['C:\\Program Files\\Microsoft Visual Studio', 'C:\\Program Files (x86)\\Microsoft Visual Studio']
    .flatMap((root) => fs.existsSync(root) ? fs.readdirSync(root).flatMap((version) => {
      const versionRoot = path.join(root, version);
      return fs.readdirSync(versionRoot).map((edition) => path.join(versionRoot, edition));
    }) : []);
  const toolsRoots = [env.VCToolsInstallDir, ...[vsRoot, ...installations].filter(Boolean).flatMap((installation) => {
    const root = path.join(installation, 'VC', 'Tools', 'MSVC');
    return fs.existsSync(root) ? fs.readdirSync(root).map((version) => path.join(root, version)) : [];
  })];
  const vcLib = toolsRoots.flatMap((root) => ['x64', path.join('onecore', 'x64')]
    .map((subdir) => path.join(root || '', 'lib', subdir)))
    .find((folder) => folder && fs.existsSync(path.join(folder, 'msvcrt.lib')));
  const sdkLibs = ['ucrt', 'um'].map((kind) => path.join(sdkRoot, sdkVersion || '', kind, 'x64'));
  env.LIB = [...new Set([...(env.LIB || '').split(path.delimiter), vcLib, ...sdkLibs]
    .filter((folder) => folder && fs.existsSync(folder)))].join(path.delimiter);
}

function run(command, args) {
  const result = spawnSync(command, args, { cwd: root, env, stdio: 'inherit' });
  if (result.error) throw result.error;
  if (result.status !== 0) throw new Error(`${command} exited with ${result.status}`);
}

run('cargo', ['build', '--manifest-path', path.join(root, 'vendor', 'context-equation', 'Cargo.toml'), '--package', 'context-equation-wasm', '--profile', 'wasm', '--target', 'wasm32-unknown-unknown']);
if (!require('fs').existsSync(wasm)) throw new Error(`WebAssembly build output missing: ${wasm}`);

fs.mkdirSync(media, { recursive: true });
run('wasm-bindgen', [wasm, '--target', 'web', '--out-dir', media, '--out-name', 'context-equation-wasm']);
fs.mkdirSync(path.join(media, 'context-equation-node'), { recursive: true });
run('wasm-bindgen', [wasm, '--target', 'nodejs', '--out-dir', path.join(media, 'context-equation-node'), '--out-name', 'context_equation_wasm']);

esbuild.buildSync({
  entryPoints: [path.join(root, 'src', 'math-editor-webview.js')],
  outfile: path.join(media, 'context-equation-editor.js'),
  bundle: true,
  platform: 'browser',
  format: 'esm',
  target: 'es2022',
  legalComments: 'none'
});

console.log('Built the standalone ConTeXt equation renderer and graphical editor.');
