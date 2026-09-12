import type { DndContextProps, DragEndEvent, DragOverEvent, DragStartEvent } from '@dnd-kit/core'
import { act, cleanup, fireEvent, render, screen, waitFor } from '@testing-library/react'
import type { ReactNode } from 'react'
import { afterEach, describe, expect, test, vi } from 'vitest'
import { type DragItem, groupDragId, pinDragId, UNGROUPED_DROP_ID } from '@/lib/customize-drag'
import type { StatusItemSegment } from '@/types/entities'
import {
  type CustomizeDisplayScreenProps,
  CustomizeDisplayScreen,
} from './customize-display-screen'

// The drag library reports which element the pointer is over from real
// measured geometry, which jsdom has none of. Its context is stood in
// for here so a spec can hand the screen the same answers a real drag
// would; everything below the context, the sortable rows included, is
// the library's own.
const dnd = vi.hoisted(() => ({ props: null as unknown }))

vi.mock('@dnd-kit/core', async (importOriginal) => {
  const actual = await importOriginal<typeof import('@dnd-kit/core')>()
  return {
    ...actual,
    DndContext: (props: DndContextProps) => {
      dnd.props = props
      return props.children
    },
    DragOverlay: ({ children }: { children?: ReactNode }) => (
      <div data-testid="drag-overlay">{children}</div>
    ),
  }
})

afterEach(cleanup)

function segment(overrides: Partial<StatusItemSegment> & { text: string }): StatusItemSegment {
  return { color: 'neutral', groupStart: false, groupId: null, slug: null, ...overrides }
}

const HANDLERS = {
  onAddToGroup: vi.fn(),
  onMakeStandalone: vi.fn(),
  onGroupTogether: vi.fn(),
  onMoveGroup: vi.fn(),
  onRenameGroup: vi.fn(),
  onUngroup: vi.fn(),
  onDeleteGroup: vi.fn(),
  onNewGroup: vi.fn(() => 'g-new'),
}

function setup(overrides: Partial<CustomizeDisplayScreenProps> = {}) {
  for (const handler of Object.values(HANDLERS)) handler.mockClear()
  const props: CustomizeDisplayScreenProps = {
    preview: [],
    groups: [
      {
        id: 'g1',
        name: 'Money',
        color: 'blue',
        members: [{ key: 'a::weekly', label: 'Work — Weekly', used: 18 }],
      },
    ],
    standalone: [
      { key: 'a::session', label: 'Work — Session', used: 99 },
      { key: 'b::session', label: 'Home — Session', used: 12 },
    ],
    ...HANDLERS,
    ...overrides,
  }
  render(<CustomizeDisplayScreen {...props} />)
  return props
}

// The screen reads an id and whatever was attached to the element; the
// rest of the library's event is geometry it never looks at.
interface Target {
  id: string
  data: { current?: DragItem }
}

function pin(key: string, groupId: string | null = null): Target {
  return { id: pinDragId(key), data: { current: { kind: 'pin', key, groupId } } }
}

function group(groupId: string): Target {
  return { id: groupDragId(groupId), data: { current: { kind: 'group', groupId } } }
}

const LEAVE_ZONE: Target = { id: UNGROUPED_DROP_ID, data: {} }

function context(): DndContextProps {
  return dnd.props as DndContextProps
}

function hover(active: Target, over: Target | null) {
  act(() => context().onDragStart?.({ active } as unknown as DragStartEvent))
  act(() => context().onDragOver?.({ active, over } as unknown as DragOverEvent))
}

function drag(active: Target, over: Target | null) {
  hover(active, over)
  act(() => context().onDragEnd?.({ active, over } as unknown as DragEndEvent))
}

// The row in the list, never the copy of it riding under the pointer,
// and never the space being held open for it.
function rowFor(label: string): HTMLElement {
  const row = screen
    .getAllByText(label)
    .find(
      (element) =>
        element.closest('[data-testid="drag-overlay"]') === null &&
        element.closest('[data-slot="true"]') === null,
    )?.parentElement
  if (row === null || row === undefined) throw new Error(`no row around ${label}`)
  return row
}

// True once the row has been taken out of the list and is riding under
// the pointer instead.
function carried(label: string): boolean {
  return rowFor(label).parentElement?.getAttribute('data-carried') === 'true'
}

// Everything the list holds inside one group's box, in order: its
// members by name, and the space being held open written as a slot.
function rowsIn(groupName: string): string[] {
  const box = screen.getByRole('group', { name: groupName })
  return Array.from(box.querySelectorAll('[data-slot="true"], [aria-label^="Reorder "]'))
    .map((element) =>
      element.getAttribute('data-slot') === 'true'
        ? 'a space for it'
        : (element.getAttribute('aria-label') ?? '').replace('Reorder ', ''),
    )
    .filter((label) => label !== groupName)
}

describe('dragging a pin onto another', () => {
  test('groups two standalone pins together, the target first', () => {
    setup()

    drag(pin('b::session'), pin('a::session'))

    expect(HANDLERS.onGroupTogether).toHaveBeenCalledWith(['a::session', 'b::session'])
  })

  test('dropped on a group member, it joins that group ahead of the member', () => {
    setup()

    drag(pin('a::session'), pin('a::weekly', 'g1'))

    expect(HANDLERS.onAddToGroup).toHaveBeenCalledWith('a::session', 'g1', 'a::weekly')
  })

  test("dropped on a group's own box, it joins at the end", () => {
    setup()

    drag(pin('a::session'), group('g1'))

    expect(HANDLERS.onAddToGroup).toHaveBeenCalledWith('a::session', 'g1', undefined)
  })

  test('dropped back on itself, nothing is rearranged', () => {
    setup()

    drag(pin('a::session'), pin('a::session'))

    expect(HANDLERS.onGroupTogether).not.toHaveBeenCalled()
    expect(HANDLERS.onAddToGroup).not.toHaveBeenCalled()
  })

  test('released over nothing at all, nothing is rearranged', () => {
    setup()

    drag(pin('a::session'), null)

    expect(HANDLERS.onGroupTogether).not.toHaveBeenCalled()
    expect(HANDLERS.onAddToGroup).not.toHaveBeenCalled()
  })

  test('dropped on a member of the group it is already in, it moves ahead of that member', () => {
    setup({
      groups: [
        {
          id: 'g1',
          name: 'Money',
          color: 'blue',
          members: [
            { key: 'a::weekly', label: 'Work — Weekly', used: 18 },
            { key: 'b::weekly', label: 'Home — Weekly', used: 44 },
          ],
        },
      ],
    })

    drag(pin('b::weekly', 'g1'), pin('a::weekly', 'g1'))

    expect(HANDLERS.onAddToGroup).toHaveBeenCalledWith('b::weekly', 'g1', 'a::weekly')
  })
})

describe('dragging a member out of its group', () => {
  test('offers the way out only while a grouped member is being dragged', () => {
    setup()
    expect(screen.queryByText('Drop here to leave the group')).toBeNull()

    hover(pin('a::session'), null)
    expect(screen.queryByText('Drop here to leave the group')).toBeNull()

    hover(pin('a::weekly', 'g1'), null)
    expect(screen.getByText('Drop here to leave the group')).toBeTruthy()
  })

  test('returns it to standalone without unpinning it', () => {
    setup()

    drag(pin('a::weekly', 'g1'), LEAVE_ZONE)

    expect(HANDLERS.onMakeStandalone).toHaveBeenCalledWith('a::weekly')
  })

  test('a pin that was standing alone anyway is not put through leaving a group', () => {
    setup()

    drag(pin('a::session'), LEAVE_ZONE)

    expect(HANDLERS.onMakeStandalone).not.toHaveBeenCalled()
  })
})

describe('dragging a group', () => {
  const TWO_GROUPS = [
    { id: 'g1', name: 'Money', color: 'blue' as const, members: [] },
    { id: 'g2', name: 'Current limit', color: 'red' as const, members: [] },
  ]

  test('onto another group reorders the two', () => {
    setup({ groups: TWO_GROUPS })

    drag(group('g2'), group('g1'))

    expect(HANDLERS.onMoveGroup).toHaveBeenCalledWith('g2', 'g1')
  })

  test("onto another group's member, it still lands ahead of that group", () => {
    setup({
      groups: [
        { ...TWO_GROUPS[0], members: [{ key: 'a::weekly', label: 'Work — Weekly', used: 18 }] },
        TWO_GROUPS[1],
      ],
    })

    drag(group('g2'), pin('a::weekly', 'g1'))

    expect(HANDLERS.onMoveGroup).toHaveBeenCalledWith('g2', 'g1')
  })

  test('onto itself changes no order', () => {
    setup({ groups: TWO_GROUPS })

    drag(group('g1'), group('g1'))

    expect(HANDLERS.onMoveGroup).not.toHaveBeenCalled()
  })

  test('onto a standalone pin, which no group sits at, changes no order', () => {
    setup({ groups: TWO_GROUPS })

    drag(group('g1'), pin('a::session'))

    expect(HANDLERS.onMoveGroup).not.toHaveBeenCalled()
  })
})

describe('where a drop will land is shown while the drag is still in the air', () => {
  test('joining a group opens a space for the row inside that box', () => {
    setup()
    expect(rowsIn('Money')).toEqual(['Work — Weekly'])

    hover(pin('a::session'), group('g1'))

    expect(rowsIn('Money')).toEqual(['Work — Weekly', 'a space for it'])
    expect(screen.getByRole('group', { name: 'Money' }).getAttribute('data-joining')).toBe('true')
  })

  test('landing ahead of a member opens it ahead of that member, not at the end', () => {
    setup()

    hover(pin('a::session'), pin('a::weekly', 'g1'))

    expect(rowsIn('Money')).toEqual(['a space for it', 'Work — Weekly'])
  })

  test('the space holds the very row being carried, named', () => {
    setup()

    hover(pin('a::session'), group('g1'))

    const slot = screen.getByRole('group', { name: 'Money' }).querySelector('[data-slot="true"]')
    expect(slot?.textContent).toContain('Work — Session')
    expect(slot?.textContent).toContain('99%')
  })

  test('the list closes up behind the row that was taken out of it', () => {
    setup()
    expect(carried('Work — Session')).toBe(false)

    hover(pin('a::session'), group('g1'))

    expect(carried('Work — Session')).toBe(true)
  })

  test('leaving a group opens the space among the loose rows instead', () => {
    setup()

    hover(pin('a::weekly', 'g1'), LEAVE_ZONE)

    expect(rowsIn('Money')).toEqual(['Work — Weekly'])
    expect(carried('Work — Weekly')).toBe(true)
    const slot = document.querySelector('[data-slot="true"]')
    expect(slot?.closest('[role="group"]')).toBeNull()
    expect(slot?.textContent).toContain('Work — Weekly')
  })

  test('reordering groups opens the space above the box it would land in front of', () => {
    setup({
      groups: [
        { id: 'g1', name: 'Money', color: 'blue', members: [] },
        { id: 'g2', name: 'Current limit', color: 'red', members: [] },
      ],
    })

    hover(group('g2'), group('g1'))

    const slot = document.querySelector('[data-slot="true"]')
    expect(slot?.textContent).toContain('Current limit')
    const money = screen.getByRole('group', { name: 'Money' })
    expect(
      (slot?.compareDocumentPosition(money) ?? 0) & Node.DOCUMENT_POSITION_FOLLOWING,
    ).toBeTruthy()
  })

  test('pairing two loose pins marks the one being dropped onto, since no box exists yet', () => {
    setup()

    hover(pin('b::session'), pin('a::session'))

    expect(rowFor('Work — Session').getAttribute('data-paired')).toBe('true')
    expect(screen.getByText('Drop to group these two')).toBeTruthy()
    expect(document.querySelector('[data-slot="true"]')).toBeNull()
  })

  test('the way out of a group lights up only once it is what a release would do', () => {
    setup()

    hover(pin('a::weekly', 'g1'), null)
    expect(screen.getByText('Drop here to leave the group').closest('[data-drop]')).toBeNull()

    act(() =>
      context().onDragOver?.({
        active: pin('a::weekly', 'g1'),
        over: LEAVE_ZONE,
      } as unknown as DragOverEvent),
    )
    expect(
      screen.getByText('Drop here to leave the group').closest('[data-drop="true"]'),
    ).toBeTruthy()
  })

  // The space closes on its way out rather than vanishing, so it is
  // still on screen for as long as that takes.
  test('an abandoned drag closes every space it had opened', async () => {
    setup()

    hover(pin('a::session'), group('g1'))
    act(() => context().onDragCancel?.({} as unknown as DragEndEvent))

    expect(carried('Work — Session')).toBe(false)
    expect(screen.getByRole('group', { name: 'Money' }).getAttribute('data-joining')).toBeNull()
    expect(screen.queryByText('Drop to group these two')).toBeNull()
    expect(HANDLERS.onAddToGroup).not.toHaveBeenCalled()
    await waitFor(() => expect(document.querySelector('[data-slot="true"]')).toBeNull())
    expect(rowsIn('Money')).toEqual(['Work — Weekly'])
  })
})

describe('every row is picked up by its own handle', () => {
  test('each pin and each group offers one, named after what it moves', () => {
    setup()

    expect(screen.getByLabelText('Reorder Money')).toBeTruthy()
    expect(screen.getByLabelText('Reorder Work — Weekly')).toBeTruthy()
    expect(screen.getByLabelText('Reorder Work — Session')).toBeTruthy()
  })
})

describe('a group header owns its own name and its own dissolution', () => {
  test('renaming commits on Enter, following the row rename pattern', () => {
    setup()

    fireEvent.click(screen.getByText('Money'))
    const field = screen.getByLabelText('Group name')
    fireEvent.change(field, { target: { value: ' Current limit ' } })
    fireEvent.keyDown(field, { key: 'Enter' })

    expect(HANDLERS.onRenameGroup).toHaveBeenCalledWith('g1', 'Current limit')
  })

  test('renaming commits on blur too, and an emptied field keeps the old name', () => {
    setup()

    fireEvent.click(screen.getByText('Money'))
    fireEvent.change(screen.getByLabelText('Group name'), { target: { value: '  ' } })
    fireEvent.blur(screen.getByLabelText('Group name'))

    expect(HANDLERS.onRenameGroup).not.toHaveBeenCalled()
  })

  test('Escape abandons the rename', () => {
    setup()

    fireEvent.click(screen.getByText('Money'))
    fireEvent.change(screen.getByLabelText('Group name'), { target: { value: 'Nope' } })
    fireEvent.keyDown(screen.getByLabelText('Group name'), { key: 'Escape' })

    expect(HANDLERS.onRenameGroup).not.toHaveBeenCalled()
    expect(screen.getByText('Money')).toBeTruthy()
  })

  test('a drag begun over an open rename field commits it rather than dropping it', () => {
    setup()

    fireEvent.click(screen.getByText('Money'))
    fireEvent.change(screen.getByLabelText('Group name'), { target: { value: 'Fable' } })
    drag(pin('a::session'), group('g1'))

    expect(HANDLERS.onRenameGroup).toHaveBeenCalledWith('g1', 'Fable')
  })

  test('ungrouping and deleting are both on the header, and neither unpins', () => {
    setup()

    fireEvent.click(screen.getByLabelText('Ungroup Money'))
    fireEvent.click(screen.getByLabelText('Delete Money'))

    expect(HANDLERS.onUngroup).toHaveBeenCalledWith('g1')
    expect(HANDLERS.onDeleteGroup).toHaveBeenCalledWith('g1')
    expect(HANDLERS.onMakeStandalone).not.toHaveBeenCalled()
  })

  test('ungrouping an already empty group is offered but disabled', () => {
    setup({ groups: [{ id: 'g1', name: 'Money', color: 'blue', members: [] }] })

    expect(screen.getByLabelText<HTMLButtonElement>('Ungroup Money').disabled).toBe(true)
  })
})

describe('an empty group can be made and named before anything goes in it', () => {
  test('creating one opens its name for editing straight away', () => {
    const props = setup()
    fireEvent.click(screen.getByText('+ New empty group'))
    expect(props.onNewGroup).toHaveBeenCalled()

    cleanup()
    setup({
      groups: [...props.groups, { id: 'g-new', name: 'Group 2', color: 'violet', members: [] }],
    })
    fireEvent.click(screen.getByText('+ New empty group'))

    expect(screen.getByLabelText<HTMLInputElement>('Group name').value).toBe('Group 2')
  })

  test('it says what to do with it rather than looking broken', () => {
    setup({ groups: [{ id: 'g1', name: 'Money', color: 'blue', members: [] }] })
    expect(screen.getByText('Drag a pin in here.')).toBeTruthy()
  })
})

describe('the preview strip', () => {
  test("draws the segments it was handed, each group's figures led by its slug", () => {
    setup({
      preview: [
        segment({ text: '99%', color: 'red', groupId: 'g1', slug: 'MON' }),
        segment({ text: '12%', groupStart: true }),
      ],
    })

    const strip = screen.getByLabelText('Menu bar preview')
    expect(strip.textContent).toContain('MON')
    expect(strip.textContent).toContain('99%')
    expect(strip.textContent).toContain('12%')
    expect(
      Array.from(strip.querySelectorAll('[data-color]')).map((f) => f.getAttribute('data-color')),
    ).toEqual(['red', 'neutral'])
  })

  test('a standalone figure is drawn on its own, with no slug in front of it', () => {
    setup({ preview: [segment({ text: '12%' })] })

    expect(screen.getByLabelText('Menu bar preview').textContent).toBe('12%')
  })
})
