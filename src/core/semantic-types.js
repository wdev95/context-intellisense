/** Maps XML command metadata to the semantic token type used by VS Code. */
function getCommandSemanticType(metadata) {
  if (!metadata) {
    return 'contextFallbackCommand';
  }
  if (metadata.kind === 'register') {
    return 'contextRegister';
  }
  if (metadata.file === 'file-job.mklx') {
    return 'namespace';
  }
  if (metadata.category === 'mathematics' && metadata.type === 'environment') {
    return 'function';
  }
  if (metadata.type === 'environment' || metadata.category === 'structure') {
    return 'contextStructure';
  }
  if (metadata.category === 'mathematics') {
    return 'contextMathCommand';
  }
  if (metadata.category === 'fonts') {
    return 'typeParameter';
  }
  if (metadata.level === 'primitive') {
    return 'keyword';
  }
  return 'function';
}

/** Maps ConTeXt internal command prefixes to semantic token types. */
function getInternalSemanticType(name, metadata) {
  if (/^\?{2,3}/.test(name) || /^[A-Za-z]!/.test(name)) {
    return '';
  }
  return metadata?.kind === 'register' ? 'contextRegister' : 'function';
}

module.exports = { getCommandSemanticType, getInternalSemanticType };
