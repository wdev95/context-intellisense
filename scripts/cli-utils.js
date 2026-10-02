const cp = require('child_process');

function needsWindowsShell(command) {
  return process.platform === 'win32' && /\.(cmd|bat)$/i.test(command);
}

function quoteForShell(value) {
  if (/^[A-Za-z0-9_./:-]+$/.test(value)) {
    return value;
  }
  return `"${String(value).replace(/"/g, '\\"')}"`;
}

function spawn(command, args, options = {}) {
  if (!needsWindowsShell(command)) {
    return cp.spawnSync(command, args, { ...options, shell: false });
  }

  const commandLine = [quoteForShell(command), ...args.map(quoteForShell)].join(' ');
  return cp.spawnSync(commandLine, { ...options, shell: true });
}

function fail(message) {
  console.error(message);
  process.exit(1);
}

function run(command, args, cwd, extraEnv = {}) {
  const result = spawn(command, args, {
    cwd,
    stdio: 'inherit',
    env: { ...process.env, ...extraEnv }
  });
  return result.status === 0;
}

function commandWorks(command, args = ['--version'], extraEnv = {}) {
  const result = spawn(command, args, {
    stdio: 'ignore',
    env: { ...process.env, ...extraEnv }
  });
  return result.status === 0;
}

module.exports = { commandWorks, fail, run, spawn };
