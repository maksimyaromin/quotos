import { cleanup, render, screen, waitFor } from '@testing-library/react'
import { afterEach, describe, expect, test, vi } from 'vitest'
import type { StatusItemSegment } from '@/types/entities'
import { MenuBarPreview } from './menu-bar-preview'

const renderStatusItemPreview = vi.hoisted(() => vi.fn())

vi.mock('@/lib/tauri-client', () => ({ renderStatusItemPreview }))

const WIDTH = 3
const HEIGHT = 2
// Two by three pixels of opaque red, which is what has to survive the
// trip through base64 and back out into the canvas.
const PIXELS = new Uint8ClampedArray(WIDTH * HEIGHT * 4).fill(0)
for (let i = 0; i < PIXELS.length; i += 4) {
  PIXELS[i] = 0xff
  PIXELS[i + 3] = 0xff
}
const IMAGE = {
  width: WIDTH,
  height: HEIGHT,
  rgbaBase64: btoa(String.fromCharCode(...PIXELS)),
}

const figure = (text: string): StatusItemSegment => ({ kind: 'figure', text, color: 'neutral' })

interface FakeImageData {
  width: number
  height: number
  data: Uint8ClampedArray
}

function stubCanvas() {
  const painted: FakeImageData[] = []
  const context = {
    clearRect: vi.fn(),
    createImageData: (width: number, height: number): FakeImageData => ({
      width,
      height,
      data: new Uint8ClampedArray(width * height * 4),
    }),
    putImageData: (image: FakeImageData) => painted.push(image),
  }
  vi.spyOn(HTMLCanvasElement.prototype, 'getContext').mockReturnValue(
    context as unknown as CanvasRenderingContext2D,
  )
  return painted
}

afterEach(() => {
  cleanup()
  vi.restoreAllMocks()
  renderStatusItemPreview.mockReset()
})

describe('the menu bar preview', () => {
  test("paints the compositor's own pixels, byte for byte", async () => {
    const painted = stubCanvas()
    renderStatusItemPreview.mockResolvedValue(IMAGE)

    render(<MenuBarPreview segments={[figure('18%')]} iconFillPercent={18} />)

    await waitFor(() => expect(painted).toHaveLength(1))
    expect(painted[0].width).toBe(WIDTH)
    expect(painted[0].height).toBe(HEIGHT)
    expect([...painted[0].data]).toEqual([...PIXELS])
  })

  test('shows the bitmap at the size the menu bar presents it', async () => {
    stubCanvas()
    renderStatusItemPreview.mockResolvedValue(IMAGE)

    render(<MenuBarPreview segments={[figure('18%')]} iconFillPercent={18} />)

    const canvas = await waitFor(() => {
      const found = screen.getByLabelText('Menu bar preview').querySelector('canvas')
      expect(found).toBeTruthy()
      return found as HTMLCanvasElement
    })
    // Two bitmap pixels to the point: `status_item_render.rs`'s own
    // RENDER_SCALE, which is what the status item's image is shown at.
    expect(canvas.style.width).toBe(`${WIDTH / 2}px`)
    expect(canvas.style.height).toBe(`${HEIGHT / 2}px`)
  })

  test('asks again when the arrangement changes, and not when it has not', async () => {
    stubCanvas()
    renderStatusItemPreview.mockResolvedValue(IMAGE)

    const { rerender } = render(<MenuBarPreview segments={[figure('18%')]} iconFillPercent={18} />)
    await waitFor(() => expect(renderStatusItemPreview).toHaveBeenCalledTimes(1))

    // A new array holding the same arrangement is the same arrangement.
    rerender(<MenuBarPreview segments={[figure('18%')]} iconFillPercent={18} />)
    await waitFor(() => expect(renderStatusItemPreview).toHaveBeenCalledTimes(1))

    rerender(
      <MenuBarPreview
        segments={[figure('18%'), { kind: 'chip', slug: 'FAB', color: 'blue', groupId: 'g1' }]}
        iconFillPercent={18}
      />,
    )
    await waitFor(() => expect(renderStatusItemPreview).toHaveBeenCalledTimes(2))
  })

  // Outside Tauri there is no compositor to ask, and nothing else may
  // draw this: a second renderer is the drift the command removes.
  test('draws nothing at all when there is no compositor to ask', async () => {
    stubCanvas()
    renderStatusItemPreview.mockResolvedValue(null)

    render(<MenuBarPreview segments={[figure('18%')]} iconFillPercent={18} />)

    await waitFor(() => expect(renderStatusItemPreview).toHaveBeenCalled())
    const strip = screen.getByLabelText('Menu bar preview')
    expect(strip.querySelector('canvas')).toBeNull()
    expect(strip.textContent).toBe('')
  })
})
