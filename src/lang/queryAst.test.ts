import {
  assertParse,
  recast,
  initPromise,
  PathToNode,
  Identifier,
  topLevelRange,
} from './wasm'
import {
  findAllPreviousVariables,
  isNodeSafeToReplace,
  isTypeInValue,
  hasExtrudeSketch,
  findUsesOfTagInPipe,
  hasSketchPipeBeenExtruded,
  doesSceneHaveSweepableSketch,
  traverse,
  getNodeFromPath,
  doesSceneHaveExtrudedSketch,
} from './queryAst'
import { getNodePathFromSourceRange } from 'lang/queryAstNodePathUtils'
import { enginelessExecutor } from '../lib/testHelpers'
import {
  createArrayExpression,
  createCallExpression,
  createLiteral,
  createPipeSubstitution,
} from './modifyAst'
import { err } from 'lib/trap'
import { codeRefFromRange } from './std/artifactGraph'

beforeAll(async () => {
  await initPromise
})

describe('findAllPreviousVariables', () => {
  it('should find all previous variables', async () => {
    const code = `baseThick = 1
armAngle = 60

baseThickHalf = baseThick / 2
halfArmAngle = armAngle / 2

arrExpShouldNotBeIncluded = [1, 2, 3]
objExpShouldNotBeIncluded = { a: 1, b: 2, c: 3 }

part001 = startSketchOn('XY')
  |> startProfileAt([0, 0], %)
  |> yLineTo(1, %)
  |> xLine(3.84, %) // selection-range-7ish-before-this

variableBelowShouldNotBeIncluded = 3
`
    const rangeStart = code.indexOf('// selection-range-7ish-before-this') - 7
    const ast = assertParse(code)
    const execState = await enginelessExecutor(ast)

    const { variables, bodyPath, insertIndex } = findAllPreviousVariables(
      ast,
      execState.memory,
      topLevelRange(rangeStart, rangeStart)
    )
    expect(variables).toEqual([
      { key: 'baseThick', value: 1 },
      { key: 'armAngle', value: 60 },
      { key: 'baseThickHalf', value: 0.5 },
      { key: 'halfArmAngle', value: 30 },
      // no arrExpShouldNotBeIncluded, variableBelowShouldNotBeIncluded etc
    ])
    // there are 4 number variables and 2 non-number variables before the sketch var
    // ∴ the insert index should be 6
    expect(insertIndex).toEqual(6)
    expect(bodyPath).toEqual([['body', '']])
  })
})

describe('testing argIsNotIdentifier', () => {
  const code = `part001 = startSketchOn('XY')
|> startProfileAt([-1.2, 4.83], %)
|> line([2.8, 0], %)
|> angledLine([100 + 100, 3.09], %)
|> angledLine([abc, 3.09], %)
|> angledLine([def('yo'), 3.09], %)
|> angledLine([ghi(%), 3.09], %)
|> angledLine([jkl('yo') + 2, 3.09], %)
yo = 5 + 6
yo2 = hmm([identifierGuy + 5])`
  it('find a safe binaryExpression', () => {
    const ast = assertParse(code)
    const rangeStart = code.indexOf('100 + 100') + 2
    const result = isNodeSafeToReplace(
      ast,
      topLevelRange(rangeStart, rangeStart)
    )
    if (err(result)) throw result
    expect(result.isSafe).toBe(true)
    expect(result.value?.type).toBe('BinaryExpression')
    expect(code.slice(result.value.start, result.value.end)).toBe('100 + 100')
    const replaced = result.replacer(structuredClone(ast), 'replaceName')
    if (err(replaced)) throw replaced
    const outCode = recast(replaced.modifiedAst)
    expect(outCode).toContain(`angledLine([replaceName, 3.09], %)`)
  })
  it('find a safe Identifier', () => {
    const ast = assertParse(code)
    const rangeStart = code.indexOf('abc')
    const result = isNodeSafeToReplace(
      ast,
      topLevelRange(rangeStart, rangeStart)
    )
    if (err(result)) throw result
    expect(result.isSafe).toBe(true)
    expect(result.value?.type).toBe('Identifier')
    expect(code.slice(result.value.start, result.value.end)).toBe('abc')
  })
  it('find a safe CallExpression', () => {
    const ast = assertParse(code)
    const rangeStart = code.indexOf('def')
    const result = isNodeSafeToReplace(
      ast,
      topLevelRange(rangeStart, rangeStart)
    )
    if (err(result)) throw result
    expect(result.isSafe).toBe(true)
    expect(result.value?.type).toBe('CallExpression')
    expect(code.slice(result.value.start, result.value.end)).toBe("def('yo')")
    const replaced = result.replacer(structuredClone(ast), 'replaceName')
    if (err(replaced)) throw replaced
    const outCode = recast(replaced.modifiedAst)
    expect(outCode).toContain(`angledLine([replaceName, 3.09], %)`)
  })
  it('find an UNsafe CallExpression, as it has a PipeSubstitution', () => {
    const ast = assertParse(code)
    const rangeStart = code.indexOf('ghi')
    const range = topLevelRange(rangeStart, rangeStart)
    const result = isNodeSafeToReplace(ast, range)
    if (err(result)) throw result
    expect(result.isSafe).toBe(false)
    expect(result.value?.type).toBe('CallExpression')
    expect(code.slice(result.value.start, result.value.end)).toBe('ghi(%)')
  })
  it('find an UNsafe Identifier, as it is a callee', () => {
    const ast = assertParse(code)
    const rangeStart = code.indexOf('ine([2.8,')
    const result = isNodeSafeToReplace(
      ast,
      topLevelRange(rangeStart, rangeStart)
    )
    if (err(result)) throw result
    expect(result.isSafe).toBe(false)
    expect(result.value?.type).toBe('CallExpression')
    expect(code.slice(result.value.start, result.value.end)).toBe(
      'line([2.8, 0], %)'
    )
  })
  it("find a safe BinaryExpression that's assigned to a variable", () => {
    const ast = assertParse(code)
    const rangeStart = code.indexOf('5 + 6') + 1
    const result = isNodeSafeToReplace(
      ast,
      topLevelRange(rangeStart, rangeStart)
    )
    if (err(result)) throw result
    expect(result.isSafe).toBe(true)
    expect(result.value?.type).toBe('BinaryExpression')
    expect(code.slice(result.value.start, result.value.end)).toBe('5 + 6')
    const replaced = result.replacer(structuredClone(ast), 'replaceName')
    if (err(replaced)) throw replaced
    const outCode = recast(replaced.modifiedAst)
    expect(outCode).toContain(`yo = replaceName`)
  })
  it('find a safe BinaryExpression that has a CallExpression within', () => {
    const ast = assertParse(code)
    const rangeStart = code.indexOf('jkl') + 1
    const result = isNodeSafeToReplace(
      ast,
      topLevelRange(rangeStart, rangeStart)
    )
    if (err(result)) throw result
    expect(result.isSafe).toBe(true)
    expect(result.value?.type).toBe('BinaryExpression')
    expect(code.slice(result.value.start, result.value.end)).toBe(
      "jkl('yo') + 2"
    )
    const replaced = result.replacer(structuredClone(ast), 'replaceName')
    if (err(replaced)) throw replaced
    const { modifiedAst } = replaced
    const outCode = recast(modifiedAst)
    expect(outCode).toContain(`angledLine([replaceName, 3.09], %)`)
  })
  it('find a safe BinaryExpression within a CallExpression', () => {
    const ast = assertParse(code)

    const rangeStart = code.indexOf('identifierGuy') + 1
    const result = isNodeSafeToReplace(
      ast,
      topLevelRange(rangeStart, rangeStart)
    )
    if (err(result)) throw result

    expect(result.isSafe).toBe(true)
    expect(result.value?.type).toBe('BinaryExpression')
    expect(code.slice(result.value.start, result.value.end)).toBe(
      'identifierGuy + 5'
    )
    const replaced = result.replacer(structuredClone(ast), 'replaceName')
    if (err(replaced)) throw replaced
    const { modifiedAst } = replaced
    const outCode = recast(modifiedAst)
    expect(outCode).toContain(`yo2 = hmm([replaceName])`)
  })

  describe('testing isTypeInValue', () => {
    it('finds the pipeSubstituion', () => {
      const val = createCallExpression('yoyo', [
        createArrayExpression([
          createLiteral(1),
          createCallExpression('yoyo2', [createPipeSubstitution()]),
          createLiteral('hey'),
        ]),
      ])
      expect(isTypeInValue(val, 'PipeSubstitution')).toBe(true)
    })
    it('There is no pipeSubstituion', () => {
      const val = createCallExpression('yoyo', [
        createArrayExpression([
          createLiteral(1),
          createCallExpression('yoyo2', [createLiteral(5)]),
          createLiteral('hey'),
        ]),
      ])
      expect(isTypeInValue(val, 'PipeSubstitution')).toBe(false)
    })
  })
})

describe('testing getNodePathFromSourceRange', () => {
  const code = `part001 = startSketchOn('XY')
  |> startProfileAt([0.39, -0.05], %)
  |> line([0.94, 2.61], %)
  |> line([-0.21, -1.4], %)`
  it('finds the second line when cursor is put at the end', () => {
    const searchLn = `line([0.94, 2.61], %)`
    const sourceIndex = code.indexOf(searchLn) + searchLn.length
    const ast = assertParse(code)

    const result = getNodePathFromSourceRange(
      ast,
      topLevelRange(sourceIndex, sourceIndex)
    )
    expect(result).toEqual([
      ['body', ''],
      [0, 'index'],
      ['declaration', 'VariableDeclaration'],
      ['init', ''],
      ['body', 'PipeExpression'],
      [2, 'index'],
    ])
  })
  it('finds the last line when cursor is put at the end', () => {
    const searchLn = `line([-0.21, -1.4], %)`
    const sourceIndex = code.indexOf(searchLn) + searchLn.length
    const ast = assertParse(code)

    const result = getNodePathFromSourceRange(
      ast,
      topLevelRange(sourceIndex, sourceIndex)
    )
    const expected = [
      ['body', ''],
      [0, 'index'],
      ['declaration', 'VariableDeclaration'],
      ['init', ''],
      ['body', 'PipeExpression'],
      [3, 'index'],
    ]
    expect(result).toEqual(expected)
    // expect similar result for start of line
    const startSourceIndex = code.indexOf(searchLn)
    const startResult = getNodePathFromSourceRange(
      ast,
      topLevelRange(startSourceIndex, startSourceIndex)
    )
    expect(startResult).toEqual([...expected, ['callee', 'CallExpression']])
    // expect similar result when whole line is selected
    const selectWholeThing = getNodePathFromSourceRange(
      ast,
      topLevelRange(startSourceIndex, sourceIndex)
    )
    expect(selectWholeThing).toEqual(expected)
  })

  it('finds the node in if-else condition', () => {
    const code = `y = 0
    x = if x > y {
      x + 1
    } else {
      y
    }`
    const searchLn = `x > y`
    const sourceIndex = code.indexOf(searchLn)
    const ast = assertParse(code)

    const result = getNodePathFromSourceRange(
      ast,
      topLevelRange(sourceIndex, sourceIndex)
    )
    expect(result).toEqual([
      ['body', ''],
      [1, 'index'],
      ['declaration', 'VariableDeclaration'],
      ['init', ''],
      ['cond', 'IfExpression'],
      ['left', 'BinaryExpression'],
    ])
    const _node = getNodeFromPath<Identifier>(ast, result)
    if (err(_node)) throw _node
    expect(_node.node.type).toEqual('Identifier')
    expect(_node.node.name).toEqual('x')
  })

  it('finds the node in if-else then', () => {
    const code = `y = 0
    x = if x > y {
      x + 1
    } else {
      y
    }`
    const searchLn = `x + 1`
    const sourceIndex = code.indexOf(searchLn)
    const ast = assertParse(code)

    const result = getNodePathFromSourceRange(
      ast,
      topLevelRange(sourceIndex, sourceIndex)
    )
    expect(result).toEqual([
      ['body', ''],
      [1, 'index'],
      ['declaration', 'VariableDeclaration'],
      ['init', ''],
      ['then_val', 'IfExpression'],
      ['body', 'IfExpression'],
      [0, 'index'],
      ['expression', 'ExpressionStatement'],
      ['left', 'BinaryExpression'],
    ])
    const _node = getNodeFromPath<Identifier>(ast, result)
    if (err(_node)) throw _node
    expect(_node.node.type).toEqual('Identifier')
    expect(_node.node.name).toEqual('x')
  })

  it('finds the node in import statement item', () => {
    const code = `import foo, bar as baz from 'thing.kcl'`
    const searchLn = `bar`
    const sourceIndex = code.indexOf(searchLn)
    const ast = assertParse(code)

    const result = getNodePathFromSourceRange(
      ast,
      topLevelRange(sourceIndex, sourceIndex)
    )
    expect(result).toEqual([
      ['body', ''],
      [0, 'index'],
      ['selector', 'ImportStatement'],
      ['items', 'ImportSelector'],
      [1, 'index'],
      ['name', 'ImportItem'],
    ])
    const _node = getNodeFromPath<Identifier>(ast, result)
    if (err(_node)) throw _node
    expect(_node.node.type).toEqual('Identifier')
    expect(_node.node.name).toEqual('bar')
  })
})

describe('testing hasExtrudeSketch', () => {
  it('find sketch', async () => {
    const exampleCode = `length001 = 2
part001 = startSketchAt([-1.41, 3.46])
  |> line([19.49, 1.16], %, $seg01)
  |> angledLine([-35, length001], %)
  |> line([-3.22, -7.36], %)
  |> angledLine([-175, segLen(seg01)], %)`
    const ast = assertParse(exampleCode)

    const execState = await enginelessExecutor(ast)
    const result = hasExtrudeSketch({
      ast,
      selection: {
        codeRef: codeRefFromRange(topLevelRange(100, 101), ast),
      },
      programMemory: execState.memory,
    })
    expect(result).toEqual(true)
  })
  it('find solid', async () => {
    const exampleCode = `length001 = 2
part001 = startSketchAt([-1.41, 3.46])
  |> line([19.49, 1.16], %, $seg01)
  |> angledLine([-35, length001], %)
  |> line([-3.22, -7.36], %)
  |> angledLine([-175, segLen(seg01)], %)
  |> extrude(1, %)`
    const ast = assertParse(exampleCode)

    const execState = await enginelessExecutor(ast)
    const result = hasExtrudeSketch({
      ast,
      selection: {
        codeRef: codeRefFromRange(topLevelRange(100, 101), ast),
      },
      programMemory: execState.memory,
    })
    expect(result).toEqual(true)
  })
  it('finds nothing', async () => {
    const exampleCode = `length001 = 2`
    const ast = assertParse(exampleCode)

    const execState = await enginelessExecutor(ast)
    const result = hasExtrudeSketch({
      ast,
      selection: {
        codeRef: codeRefFromRange(topLevelRange(10, 11), ast),
      },
      programMemory: execState.memory,
    })
    expect(result).toEqual(false)
  })
})

describe('Testing findUsesOfTagInPipe', () => {
  const exampleCode = `part001 = startSketchOn('-XZ')
|> startProfileAt([68.12, 156.65], %)
|> line([306.21, 198.82], %)
|> line([306.21, 198.85], %, $seg01)
|> angledLine([-65, segLen(seg01)], %)
|> line([306.21, 198.87], %)
|> angledLine([65, segLen(seg01)], %)`
  it('finds the current segment', async () => {
    const ast = assertParse(exampleCode)

    const lineOfInterest = `198.85], %, $seg01`
    const characterIndex =
      exampleCode.indexOf(lineOfInterest) + lineOfInterest.length
    const pathToNode = getNodePathFromSourceRange(
      ast,
      topLevelRange(characterIndex, characterIndex)
    )
    const result = findUsesOfTagInPipe(ast, pathToNode)
    expect(result).toHaveLength(2)
    result.forEach((range) => {
      expect(exampleCode.slice(range[0], range[1])).toContain('segLen')
    })
  })
  it('find no tag if line has no tag', () => {
    const ast = assertParse(exampleCode)

    const lineOfInterest = `line([306.21, 198.82], %)`
    const characterIndex =
      exampleCode.indexOf(lineOfInterest) + lineOfInterest.length
    const pathToNode = getNodePathFromSourceRange(
      ast,
      topLevelRange(characterIndex, characterIndex)
    )
    const result = findUsesOfTagInPipe(ast, pathToNode)
    expect(result).toHaveLength(0)
  })
})

describe('Testing hasSketchPipeBeenExtruded', () => {
  const exampleCode = `sketch001 = startSketchOn('XZ')
  |> startProfileAt([3.29, 7.86], %)
  |> line([2.48, 2.44], %)
  |> line([2.66, 1.17], %)
  |> line([3.75, 0.46], %)
  |> line([4.99, -0.46], %, $seg01)
  |> line([3.3, -2.12], %)
  |> line([2.16, -3.33], %)
  |> line([0.85, -3.08], %)
  |> line([-0.18, -3.36], %)
  |> line([-3.86, -2.73], %)
  |> line([-17.67, 0.85], %)
  |> close(%)
extrude001 = extrude(10, sketch001)
sketch002 = startSketchOn(extrude001, seg01)
  |> startProfileAt([-12.94, 6.6], %)
  |> line([2.45, -0.2], %)
  |> line([-2, -1.25], %)
  |> lineTo([profileStartX(%), profileStartY(%)], %)
  |> close(%)
sketch003 = startSketchOn(extrude001, 'END')
  |> startProfileAt([8.14, 2.8], %)
  |> line([-1.24, 4.39], %)
  |> line([3.79, 1.91], %)
  |> line([1.77, -2.95], %)
  |> line([3.12, 1.74], %)
  |> line([1.91, -4.09], %)
  |> line([-5.6, -2.75], %)
  |> lineTo([profileStartX(%), profileStartY(%)], %)
  |> close(%)
  |> extrude(3.14, %)
`
  it('identifies sketch001 pipe as extruded (extrusion after pipe)', async () => {
    const ast = assertParse(exampleCode)
    const lineOfInterest = `line([4.99, -0.46], %, $seg01)`
    const characterIndex =
      exampleCode.indexOf(lineOfInterest) + lineOfInterest.length
    const extruded = hasSketchPipeBeenExtruded(
      {
        codeRef: codeRefFromRange(
          topLevelRange(characterIndex, characterIndex),
          ast
        ),
      },
      ast
    )
    expect(extruded).toBeTruthy()
  })
  it('identifies sketch002 pipe as not extruded', async () => {
    const ast = assertParse(exampleCode)
    const lineOfInterest = `line([2.45, -0.2], %)`
    const characterIndex =
      exampleCode.indexOf(lineOfInterest) + lineOfInterest.length
    const extruded = hasSketchPipeBeenExtruded(
      {
        codeRef: codeRefFromRange(
          topLevelRange(characterIndex, characterIndex),
          ast
        ),
      },
      ast
    )
    expect(extruded).toBeFalsy()
  })
  it('identifies sketch003 pipe as extruded (extrusion within pipe)', async () => {
    const ast = assertParse(exampleCode)
    const lineOfInterest = `|> line([3.12, 1.74], %)`
    const characterIndex =
      exampleCode.indexOf(lineOfInterest) + lineOfInterest.length
    const extruded = hasSketchPipeBeenExtruded(
      {
        codeRef: codeRefFromRange(
          topLevelRange(characterIndex, characterIndex),
          ast
        ),
      },
      ast
    )
    expect(extruded).toBeTruthy()
  })
})

describe('Testing doesSceneHaveSweepableSketch', () => {
  it('finds sketch001 pipe to be extruded', async () => {
    const exampleCode = `sketch001 = startSketchOn('XZ')
  |> startProfileAt([3.29, 7.86], %)
  |> line([2.48, 2.44], %)
  |> line([-3.86, -2.73], %)
  |> line([-17.67, 0.85], %)
  |> close(%)
extrude001 = extrude(10, sketch001)
sketch002 = startSketchOn(extrude001, $seg01)
  |> startProfileAt([-12.94, 6.6], %)
  |> line([2.45, -0.2], %)
  |> line([-2, -1.25], %)
  |> lineTo([profileStartX(%), profileStartY(%)], %)
  |> close(%)
`
    const ast = assertParse(exampleCode)
    const extrudable = doesSceneHaveSweepableSketch(ast)
    expect(extrudable).toBeTruthy()
  })
  it('finds sketch001 and sketch002 pipes to be lofted', async () => {
    const exampleCode = `sketch001 = startSketchOn('XZ')
  |> circle({ center = [0, 0], radius = 1 }, %)
plane001 = offsetPlane('XZ', 2)
sketch002 = startSketchOn(plane001)
  |> circle({ center = [0, 0], radius = 3 }, %)
`
    const ast = assertParse(exampleCode)
    const extrudable = doesSceneHaveSweepableSketch(ast, 2)
    expect(extrudable).toBeTruthy()
  })
  it('find sketch002 NOT pipe to be extruded', async () => {
    const exampleCode = `sketch001 = startSketchOn('XZ')
  |> startProfileAt([3.29, 7.86], %)
  |> line([2.48, 2.44], %)
  |> line([-3.86, -2.73], %)
  |> line([-17.67, 0.85], %)
  |> close(%)
extrude001 = extrude(10, sketch001)
`
    const ast = assertParse(exampleCode)
    const extrudable = doesSceneHaveSweepableSketch(ast)
    expect(extrudable).toBeFalsy()
  })
})

describe('Testing doesSceneHaveExtrudedSketch', () => {
  it('finds extruded sketch as variable', async () => {
    const exampleCode = `sketch001 = startSketchOn('XZ')
  |> circle({ center = [0, 0], radius = 1 }, %)
extrude001 = extrude(1, sketch001)
`
    const ast = assertParse(exampleCode)
    if (err(ast)) throw ast
    const extrudable = doesSceneHaveExtrudedSketch(ast)
    expect(extrudable).toBeTruthy()
  })
  it('finds extruded sketch in pipe', async () => {
    const exampleCode = `extrude001 = startSketchOn('XZ')
  |> circle({ center = [0, 0], radius = 1 }, %)
  |> extrude(1, %)
`
    const ast = assertParse(exampleCode)
    if (err(ast)) throw ast
    const extrudable = doesSceneHaveExtrudedSketch(ast)
    expect(extrudable).toBeTruthy()
  })
  it('finds no extrusion with sketch only', async () => {
    const exampleCode = `extrude001 = startSketchOn('XZ')
  |> circle({ center = [0, 0], radius = 1 }, %)
`
    const ast = assertParse(exampleCode)
    if (err(ast)) throw ast
    const extrudable = doesSceneHaveExtrudedSketch(ast)
    expect(extrudable).toBeFalsy()
  })
})

describe('Testing traverse and pathToNode', () => {
  it.each([
    ['basic', '2.73'],
    [
      'very nested, array, object, callExpression, array, memberExpression',
      '.yo',
    ],
  ])('testing %s', async (testName, literalOfInterest) => {
    const code = `myVar = 5
sketch001 = startSketchOn('XZ')
  |> startProfileAt([3.29, 7.86], %)
  |> line([2.48, 2.44], %)
  |> line([-3.86, -2.73], %)
  |> line([-17.67, 0.85], %)
  |> close(%)
bing = { yo: 55 }
myNestedVar = [
  {
  prop:   line([bing.yo, 21], sketch001)
}
]
  `
    const ast = assertParse(code)
    let pathToNode: PathToNode = []
    traverse(ast, {
      enter: (node, path) => {
        if (
          node.type === 'Literal' &&
          String((node as any).value.value) === literalOfInterest
        ) {
          pathToNode = path
        } else if (
          node.type === 'Identifier' &&
          literalOfInterest.includes(node.name)
        ) {
          pathToNode = path
        }
      },
    })

    const literalIndex = code.indexOf(literalOfInterest)
    const pathToNode2 = getNodePathFromSourceRange(
      ast,
      topLevelRange(literalIndex + 2, literalIndex + 2)
    )
    expect(pathToNode).toEqual(pathToNode2)
  })
})
