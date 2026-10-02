#!/usr/bin/env node

const fs = require('fs');
const path = require('path');
const { commandWorks, fail, run, spawn } = require('./scripts/cli-utils');

function resolveFirstWorkingCommand(candidates, args = ['--version'], extraEnv = {}) {
  for (const candidate of candidates) {
    if (candidate && commandWorks(candidate, args, extraEnv)) {
      return candidate;
    }
  }
  return '';
}

function listVsixFiles(extensionRoot) {
  return fs.readdirSync(extensionRoot)
    .filter((file) => file.endsWith('.vsix'))
    .map((file) => ({
      file,
      fullPath: path.join(extensionRoot, file),
      mtimeMs: fs.statSync(path.join(extensionRoot, file)).mtimeMs
    }));
}

function bumpPatch(version) {
  const match = /^(\d+)\.(\d+)\.(\d+)$/.exec(version);
  if (!match) {
    fail('package.json version must follow MAJOR.MINOR.PATCH (for example 1.0.0).');
  }

  const major = Number(match[1]);
  const minor = Number(match[2]);
  const patch = Number(match[3]);
  return `${major}.${minor}.${patch + 1}`;
}

function main() {
  const extensionRoot = path.resolve(__dirname);
  const isWindows = process.platform === 'win32';
  const nodeDir = path.dirname(process.execPath);
  const commandEnv = {
    PATH: `${nodeDir}${path.delimiter}${process.env.PATH || ''}`
  };
  const packageJsonPath = path.join(extensionRoot, 'package.json');

  if (!fs.existsSync(packageJsonPath)) {
    fail(`package.json not found: ${packageJsonPath}`);
  }

  const originalText = fs.readFileSync(packageJsonPath, 'utf8');
  let pkg;
  try {
    pkg = JSON.parse(originalText);
  } catch (error) {
    fail(`Invalid package.json: ${error.message}`);
  }

  const previousVersion = String(pkg.version || '').trim();
  if (!pkg.publisher || !pkg.name || !previousVersion) {
    fail('package.json must contain publisher, name, and version.');
  }

  const nextVersion = bumpPatch(previousVersion);
  pkg.version = nextVersion;
  fs.writeFileSync(packageJsonPath, `${JSON.stringify(pkg, null, 2)}\n`, 'utf8');
  console.log(`Version bump: ${previousVersion} -> ${nextVersion}`);

  const beforeVsix = new Set(listVsixFiles(extensionRoot).map((entry) => entry.file));

  let buildSucceeded = false;
  try {
    const localVsce = path.join(extensionRoot, 'node_modules', '.bin', isWindows ? 'vsce.cmd' : 'vsce');
    const bundledNpx = path.join(nodeDir, isWindows ? 'npx.cmd' : 'npx');

    const vsceCommand = resolveFirstWorkingCommand([localVsce, 'vsce'], ['--version'], commandEnv);
    const npxCommand = resolveFirstWorkingCommand([bundledNpx, 'npx'], ['--version'], commandEnv);

    if (vsceCommand) {
      buildSucceeded = run(vsceCommand, ['package', '--allow-missing-repository'], extensionRoot, commandEnv);
    } else if (npxCommand) {
      buildSucceeded = run(npxCommand, ['--yes', '@vscode/vsce', 'package', '--allow-missing-repository'], extensionRoot, commandEnv);
    } else {
      fail("Could not find 'vsce' or 'npx'. Install Node.js and run: npm i -g @vscode/vsce");
    }
  } finally {
    if (!buildSucceeded) {
      // Roll back version bump if packaging fails.
      fs.writeFileSync(packageJsonPath, originalText, 'utf8');
      console.log(`Build failed. Reverted package.json version to ${previousVersion}.`);
    }
  }

  if (!buildSucceeded) {
    fail('VSIX packaging failed.');
  }

  const afterVsix = listVsixFiles(extensionRoot);
  const created = afterVsix
    .filter((entry) => !beforeVsix.has(entry.file))
    .sort((a, b) => b.mtimeMs - a.mtimeMs);

  if (created.length > 0) {
    console.log(`VSIX created: ${created[0].fullPath}`);
    return;
  }

  const fallbackName = `${pkg.name}-${nextVersion}.vsix`;
  const fallbackPath = path.join(extensionRoot, fallbackName);
  if (fs.existsSync(fallbackPath)) {
    console.log(`VSIX created: ${fallbackPath}`);
    return;
  }

  console.log('VSIX build command succeeded, but no new VSIX file was detected.');
}

main();
