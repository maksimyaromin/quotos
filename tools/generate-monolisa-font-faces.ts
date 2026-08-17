#!/usr/bin/env node
import { existsSync, writeFileSync } from 'node:fs'
import { join } from 'node:path'

const FONTS_DIR = 'src/design-system/assets/fonts'
const OUTPUT_PATH = 'src/design-system/tokens/monolisa.generated.css'
const WEIGHTS = [400, 500, 600, 700]

function fontFace(weight: number): string {
  return `@font-face {
  font-family: "MonoLisa";
  font-style: normal;
  font-weight: ${weight};
  font-display: swap;
  src: url("../assets/fonts/MonoLisa-${weight}.woff2") format("woff2");
}
`
}

const blocks = WEIGHTS.filter((weight) =>
  existsSync(join(FONTS_DIR, `MonoLisa-${weight}.woff2`)),
).map(fontFace)

writeFileSync(OUTPUT_PATH, blocks.join('\n'))
