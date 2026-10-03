// A deliberately bounded expression compiler. Never treat a whole shader as HLSL.
const functions = {
  sin: [1, 'sin'], cos: [1, 'cos'], abs: [1, 'abs'], floor: [1, 'floor'],
  ceil: [1, 'ceil'], fract: [1, 'frac'], sqrt: [1, 'sqrt'], exp: [1, 'exp'],
  log: [1, 'log'], normalize: [1, 'normalize'], length: [1, 'length'],
  min: [2, 'min'], max: [2, 'max'], pow: [2, 'pow'], dot: [2, 'dot'],
  cross: [2, 'cross'], step: [2, 'step'], mix: [3, 'lerp'],
  clamp: [3, 'clamp'], smoothstep: [3, 'smoothstep'],
};

export function compileExpression(source, bindings = {}) {
  return compileTypedExpression(source, bindings).code;
}

function compileTypedExpression(source, bindings = {}) {
  if (typeof source !== 'string' || source.length > 16384) throw new Error('Invalid shader expression');
  const tokens = [];
  const lexer = /\s+|(?:\d+\.?\d*|\.\d+)(?:[eE][+-]?\d+)?|[A-Za-z_]\w*|[()+\-*/,.]/gy;
  let offset = 0;
  while (offset < source.length) {
    lexer.lastIndex = offset;
    const token = lexer.exec(source);
    if (!token) throw new Error(`Unsupported GLSL token at ${offset}: ${source.slice(offset, offset + 24)}`);
    offset = lexer.lastIndex;
    if (token[0].trim()) tokens.push(token[0]);
  }
  let pos = 0;
  const take = () => tokens[pos++];
  const node = (code, size = 1) => ({ code, size });
  function compatible(a, b) {
    if (a.size !== b.size && a.size !== 1 && b.size !== 1) throw new Error('Incompatible vector dimensions');
    return Math.max(a.size, b.size);
  }
  function primary() {
    const token = take();
    let result;
    if (token === '-' || token === '+') { const rhs = primary(); result = node(`(${token}${rhs.code})`, rhs.size); }
    else if (token === '(') {
      const inner = expression(); result = node(`(${inner.code})`, inner.size);
      if (take() !== ')') throw new Error('Expected closing parenthesis');
    } else if (/^(?:\d|\.\d)/.test(token ?? '')) {
      if (!Number.isFinite(Number(token))) throw new Error('Non-finite shader constant');
      result = node(/[.eE]/.test(token) ? token : `${token}.0`);
    }
    else if (/^[A-Za-z_]\w*$/.test(token ?? '')) {
      if (tokens[pos] === '(') {
        take();
        const args = [];
        if (tokens[pos] !== ')') {
          do { args.push(expression()); } while (tokens[pos] === ',' && take());
        }
        if (take() !== ')') throw new Error('Expected closing function parenthesis');
        if (/^vec[234]$/.test(token)) {
          const size = Number(token.at(-1));
          const splat = args.length === 1 && args[0].size === 1;
          if (!splat && args.reduce((sum, a) => sum + a.size, 0) !== size) throw new Error(`Invalid ${token} component count`);
          const parts = splat ? Array(size).fill(args[0].code) : args.map(a => a.code);
          result = node(`float${size}(${parts.join(', ')})`, size);
        } else if (token === 'mod' && args.length === 2) {
          // GLSL mod differs from HLSL fmod for negative values.
          const size = compatible(...args); const [a, b] = args.map(a => a.code);
          result = node(`((${a}) - (${b}) * floor((${a}) / (${b})))`, size);
        } else if (token === 'atan' && (args.length === 1 || args.length === 2)) {
          const size = args.length === 2 ? compatible(...args) : args[0].size;
          result = node(`${args.length === 2 ? 'atan2' : 'atan'}(${args.map(a => a.code).join(', ')})`, size);
        } else {
          const fn = functions[token];
          if (!fn || args.length !== fn[0]) throw new Error(`Unsupported function/arity: ${token}`);
          let size = args.reduce((a, b) => node('', compatible(a, b))).size;
          if (token === 'dot') {
            if (args[0].size !== args[1].size) throw new Error('dot needs equal vector dimensions');
            size = 1;
          }
          if (token === 'cross') {
            if (args.some(a => a.size !== 3)) throw new Error('cross needs vec3');
            size = 3;
          }
          if (token === 'length') size = 1;
          result = node(`${fn[1]}(${args.map(a => a.code).join(', ')})`, size);
        }
      } else {
        if (!Object.hasOwn(bindings, token)) throw new Error(`Unbound shader identifier: ${token}`);
        const binding = bindings[token];
        result = typeof binding === 'string' ? node(binding) : node(binding.code, binding.size);
      }
    } else throw new Error(`Unexpected shader token: ${token}`);
    while (tokens[pos] === '.') {
      take();
      const swizzle = take();
      if (!/^(?:[xyzw]{1,4}|[rgba]{1,4})$/.test(swizzle ?? '')) throw new Error('Only vector swizzles are supported');
      if ([...swizzle].some(c => 'xyzw'.indexOf(c) >= result.size || 'rgba'.indexOf(c) >= result.size)) throw new Error('Swizzle exceeds vector dimensions');
      result = node(`(${result.code}).${swizzle}`, swizzle.length);
    }
    return result;
  }
  function binary(next, ops) {
    let lhs = next();
    while (ops.includes(tokens[pos])) { const op = take(); const rhs = next(); lhs = node(`(${lhs.code} ${op} ${rhs.code})`, compatible(lhs, rhs)); }
    return lhs;
  }
  const product = () => binary(primary, ['*', '/']);
  const expression = () => binary(product, ['+', '-']);
  const result = expression();
  if (pos !== tokens.length) throw new Error(`Unexpected trailing token: ${tokens[pos]}`);
  return result;
}

export function shaderRecipe(material, name) {
  const bridge = material.userData?.elfentierUE ?? {};
  // Only auto-extract a single assignment. Helpers, control flow and chunks need an adapter.
  const clean = (material.fragmentShader ?? '').replace(/\/\*[\s\S]*?\*\//g, '').replace(/\/\/[^\n]*/g, '');
  const match = clean.match(/void\s+main\s*\(\s*\)\s*\{\s*gl_FragColor\s*=\s*([^;]+);\s*\}\s*$/);
  if (!bridge.fragmentExpression && !match) throw new Error('Provide userData.elfentierUE.fragmentExpression; only single-assignment fragment shaders auto-convert');
  if (!bridge.fragmentExpression && /#|\b(?:attribute|in|out)\b/.test(clean)) throw new Error('Preprocessor/GLSL3 shader needs an explicit expression adapter');
  const bindings = { UV: { code: 'UV', size: 2 }, Time: { code: 'Time', size: 1 } };
  const inputs = [{ name: 'UV', kind: 'uv' }, { name: 'Time', kind: 'time' }];
  for (const [symbol, kind] of Object.entries(bridge.bindings ?? {})) {
    if (!/^[A-Za-z_]\w*$/.test(symbol) || !['uv', 'time'].includes(kind)) throw new Error(`Unsupported binding ${symbol}: ${kind}`);
    bindings[symbol] = bindings[kind === 'uv' ? 'UV' : 'Time'];
  }
  for (const [symbol, uniform] of Object.entries(material.uniforms ?? {})) {
    if (Object.hasOwn(bindings, symbol)) continue;
    if (!/^[A-Za-z_]\w*$/.test(symbol) || symbol.startsWith('gl_')) throw new Error(`Invalid uniform: ${symbol}`);
    const value = uniform.value;
    if (typeof value === 'number' && Number.isFinite(value)) {
      inputs.push({ name: symbol, kind: 'scalar', value }); bindings[symbol] = { code: symbol, size: 1 };
    } else if (value?.isColor || value?.isVector2 || value?.isVector3 || value?.isVector4) {
      const values = value.toArray();
      if (!values.every(Number.isFinite)) throw new Error(`Non-finite uniform: ${symbol}`);
      inputs.push({ name: symbol, kind: 'vector', value: [...values, ...Array(4 - values.length).fill(0)] });
      bindings[symbol] = { code: `${symbol}.${'xyzw'.slice(0, values.length)}`, size: values.length };
    } else if (new RegExp(`\\b${symbol}\\b`).test(bridge.fragmentExpression ?? match?.[1] ?? '')) {
      throw new Error(`Unsupported uniform ${symbol}; samplers/matrices need baking or a native adapter`);
    }
  }
  const compiled = compileTypedExpression(bridge.fragmentExpression ?? match[1], bindings);
  if (compiled.size !== 4) throw new Error('Fragment expression must return vec4');
  const hlsl = `return ${compiled.code};`;
  // No automatic claim that the vertex program was reproduced.
  if (!bridge.fragmentExpression && !/^\s*void\s+main\s*\(\s*\)\s*\{\s*gl_Position\s*=\s*projectionMatrix\s*\*\s*modelViewMatrix\s*\*\s*vec4\s*\(\s*position\s*,\s*1\.0\s*\)\s*;\s*\}\s*$/.test(material.vertexShader ?? '')) {
    throw new Error('Custom vertex shader requires an explicit expression adapter or surface cache');
  }
  return { name, kind: 'custom_expression', hlsl, inputs, output: 'float4',
    transparent: material.transparent, alphaTest: material.alphaTest, twoSided: material.side === 2,
    sourceVertexPreserved: false, adapterExplicit: Boolean(bridge.fragmentExpression) };
}
