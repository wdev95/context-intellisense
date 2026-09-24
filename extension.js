const runtime = require('./src/extension-runtime');

module.exports = {
  activate: runtime.activate,
  deactivate: runtime.deactivate
};
