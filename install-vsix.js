#!/usr/bin/env node

const fs = require('fs');
const path = require('path');
const { commandWorks, fail, run, spawn } = require('./scripts/cli-utils');

/** Prevents VS Code's configuration registry race during live extension installs. */
function isTargetVsCodeRunning(preferInsiders) {
  if (process.platform !== 'win32') {
    return false;
  }

  const imageName = preferInsiders ? 'Code - Insiders.exe' : 'Code.exe';
  const result = spawn('tasklist.exe', ['/FO', 'CSV', '/NH', '/FI', `IMAGENAME eq ${imageName}`], {
    encoding: 'utf8',
    windowsHide: true
  });
  return result.status === 0 && result.stdout.split(/\r?\n/).some((line) =>
    line.toLowerCase().startsWith(`"${imageName.toLowerCase()}"`)
  );
}

function pickLatestVsix(extensionRoot, publisher, name) {
  const preferredPrefix = `${publisher}.${name}-`;
  const namePrefix = `${name}-`;
  const files = fs.readdirSync(extensionRoot)
    .filter((file) => {
      if (!file.endsWith('.vsix')) {
        return false;
      }
      return file.startsWith(preferredPrefix) || file.startsWith(namePrefix);
    })
    .map((file) => ({
      file,
      fullPath: path.join(extensionRoot, file),
      mtimeMs: fs.statSync(path.join(extensionRoot, file)).mtimeMs
    }))
    .sort((a, b) => b.mtimeMs - a.mtimeMs);

  return files.length > 0 ? files[0].fullPath : '';
}

function resolveCodeCli(preferInsiders = false) {
  const candidates = preferInsiders
    ? ['code-insiders', 'code', 'codium']
    : ['code', 'code-insiders', 'codium'];

  if (process.platform === 'win32') {
    const localAppData = process.env.LOCALAPPDATA || '';
    const programFiles = process.env['ProgramFiles'] || 'C:\\Program Files';
    const programFilesX86 = process.env['ProgramFiles(x86)'] || 'C:\\Program Files (x86)';

    const stableCandidates = [
      path.join(localAppData, 'Programs', 'Microsoft VS Code', 'bin', 'code.cmd'),
      path.join(programFiles, 'Microsoft VS Code', 'bin', 'code.cmd'),
      path.join(programFilesX86, 'Microsoft VS Code', 'bin', 'code.cmd')
    ];
    const insidersCandidates = [
      path.join(localAppData, 'Programs', 'Microsoft VS Code Insiders', 'bin', 'code-insiders.cmd'),
      path.join(programFiles, 'Microsoft VS Code Insiders', 'bin', 'code-insiders.cmd'),
      path.join(programFilesX86, 'Microsoft VS Code Insiders', 'bin', 'code-insiders.cmd')
    ];
    candidates.unshift(...(preferInsiders
      ? [...insidersCandidates, ...stableCandidates]
      : [...stableCandidates, ...insidersCandidates]));
  }

  for (const candidate of candidates) {
    if (commandWorks(candidate)) {
      return candidate;
    }
  }

  fail("Could not find VS Code CLI ('code', 'code-insiders', or 'codium').");
}

function main() {
  const extensionRoot = path.resolve(__dirname);
  const packageJsonPath = path.join(extensionRoot, 'package.json');
  if (!fs.existsSync(packageJsonPath)) {
    fail(`package.json not found: ${packageJsonPath}`);
  }

  let pkg;
  try {
    pkg = JSON.parse(fs.readFileSync(packageJsonPath, 'utf8'));
  } catch (error) {
    fail(`Invalid package.json: ${error.message}`);
  }

  if (!pkg.publisher || !pkg.name) {
    fail('package.json must contain publisher and name.');
  }

  const explicitArgument = process.argv.slice(2).find((argument) => !argument.startsWith('-'));
  const explicitPath = explicitArgument ? path.resolve(explicitArgument) : '';
  const vsixPath = explicitPath || pickLatestVsix(extensionRoot, pkg.publisher, pkg.name);
  if (!vsixPath || !fs.existsSync(vsixPath)) {
    fail('VSIX file not found. Build first or pass an explicit path as argument.');
  }

  const preferInsiders = process.argv.includes('--insiders');
  if (isTargetVsCodeRunning(preferInsiders)) {
    const targetName = preferInsiders ? 'VS Code Insiders' : 'VS Code';
    fail(`${targetName} is running. Close it before installing the VSIX; live installation can leave the Settings configuration registry stale.`);
  }

  const codeCli = resolveCodeCli(preferInsiders);
  const targetName = preferInsiders ? 'VS Code Insiders' : 'VS Code';
  console.log(`Installing VSIX into ${targetName}: ${vsixPath}`);
  const ok = run(codeCli, ['--install-extension', vsixPath, '--force'], extensionRoot, {
    NODE_OPTIONS: '--no-deprecation'
  });

  if (!ok) {
    fail('VSIX installation failed.');
  }
}

main();
