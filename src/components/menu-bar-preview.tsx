import { useEffect, useRef, useState } from 'react'
import { renderStatusItemPreview } from '@/lib/tauri-client'
import type { StatusItemImage, StatusItemSegment } from '@/types/entities'
import styles from './menu-bar-preview.module.css'

// Two pixels per point, `status_item_render.rs`'s `RENDER_SCALE`: what
// the compositor draws at, and so what this has to divide by to show
// the image at the size the menu bar shows it.
const RENDER_SCALE = 2

function decodeRgba(base64: string): Uint8ClampedArray {
  const binary = atob(base64)
  const bytes = new Uint8ClampedArray(binary.length)
  for (let i = 0; i < binary.length; i += 1) bytes[i] = binary.charCodeAt(i)
  return bytes
}

// The menu bar as it will actually look, because it is the menu bar's
// own compositor drawing it: the screen sends the arrangement it is
// showing and paints back the bitmap that comes out. Nothing here
// re-describes a chip, a figure or a gap, which is what kept the old
// CSS strip drifting from the tray it stood for. See "One renderer,
// two surfaces" in docs/status-item-rendering.md.
export function MenuBarPreview({
  segments,
  iconFillPercent,
}: {
  segments: StatusItemSegment[]
  iconFillPercent: number
}) {
  const canvasRef = useRef<HTMLCanvasElement>(null)
  const [image, setImage] = useState<StatusItemImage | null>(null)
  const serialized = JSON.stringify(segments)

  useEffect(() => {
    let cancelled = false
    void (async () => {
      try {
        const rendered = await renderStatusItemPreview(
          JSON.parse(serialized) as StatusItemSegment[],
          iconFillPercent,
        )
        if (!cancelled) setImage(rendered)
      } catch (error) {
        console.error('Quotos: drawing the menu bar preview failed', error)
      }
    })()
    return () => {
      cancelled = true
    }
  }, [serialized, iconFillPercent])

  useEffect(() => {
    const canvas = canvasRef.current
    if (canvas === null || image === null) return
    const context = canvas.getContext('2d')
    if (context === null) return
    context.clearRect(0, 0, canvas.width, canvas.height)
    // Through the context's own `createImageData` rather than the
    // `ImageData` constructor: same bytes, and the one path that does
    // not reach for a global.
    const painted = context.createImageData(image.width, image.height)
    painted.data.set(decodeRgba(image.rgbaBase64))
    context.putImageData(painted, 0, 0)
  }, [image])

  return (
    <div className={styles.strip} role="img" aria-label="Menu bar preview">
      {image === null ? null : (
        <canvas
          ref={canvasRef}
          width={image.width}
          height={image.height}
          style={{
            width: `${image.width / RENDER_SCALE}px`,
            height: `${image.height / RENDER_SCALE}px`,
          }}
          className={styles.image}
        />
      )}
    </div>
  )
}
