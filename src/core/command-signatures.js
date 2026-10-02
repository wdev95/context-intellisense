const vscode = require('vscode');
const { getDelimiterInfo } = require('./data-cache');

/** Returns the compact schema displayed for an XML argument. */
function describeArgumentSpec(spec) {
  if (spec.kind === 'assignments') {
    return spec.list ? '...=..., ...=...' : '...=...';
  }
  if (spec.kind === 'keywords') {
    return spec.list ? '..., ...' : '...';
  }
  if (spec.kind === 'content') {
    return 'CONTENT';
  }
  return '...';
}

/** Builds a snippet containing one placeholder for every non-content argument. */
function buildArgumentSnippet(argumentSpecs, startIndex = 1) {
  const parts = [];
  let tabIndex = startIndex;
  for (const spec of argumentSpecs || []) {
    if (spec.kind === 'content') {
      continue;
    }
    const delimiter = getDelimiterInfo(spec.delimiter);
    parts.push(`${delimiter.open}${'${' + tabIndex + '}'}${delimiter.close}`);
    tabIndex += 1;
  }
  return { text: parts.join(''), nextIndex: tabIndex };
}

/** Creates the insertion text for a command completion. */
function makeCommandInsertText(commandName, argumentSpecs) {
  const snippet = buildArgumentSnippet(argumentSpecs, 1);
  return snippet.text
    ? new vscode.SnippetString(`${commandName}${snippet.text}`)
    : commandName;
}

/** Builds the compact command label shown in the completion list. */
function makeArgumentPreview(commandName, argumentSpecs) {
  const argumentsText = (argumentSpecs || [])
    .filter(spec => spec.kind !== 'content')
    .map(spec => {
      const delimiter = getDelimiterInfo(spec.delimiter);
      return `${delimiter.open}${delimiter.close}`;
    })
    .join('');
  return `\\${commandName}${argumentsText}`;
}

module.exports = {
  describeArgumentSpec,
  buildArgumentSnippet,
  makeCommandInsertText,
  makeArgumentPreview
};
