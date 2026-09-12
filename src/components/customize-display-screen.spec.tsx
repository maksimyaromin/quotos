import type { DndContextProps, DragEndEvent, DragOverEvent, DragStartEvent } from '@dnd-kit/core'
import { act, cleanup, fireEvent, render, screen } from '@testing-library/react'
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
    DragOverlay: ({ children }: { children?: ReactNode }) => children ?? null,
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

function rowFor(label: string): HTMLElement {
  const row = screen.getByText(label).parentElement
  if (row === null) throw new Error(`no row around ${label}`)
  return row
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
  test("joining a group lights that group's box and says so in its own name", () => {
    setup()

    hover(pin('a::session'), group('g1'))

    expect(screen.getByText('Money').closest('[data-joining="true"]')).toBeTruthy()
    expect(screen.getByText('Drop to add to Money')).toBeTruthy()
  })

  test('landing ahead of a member draws the line above that member, not the box', () => {
    setup()

    hover(pin('a::session'), pin('a::weekly', 'g1'))

    expect(rowFor('Work — Weekly').getAttribute('data-insert-before')).toBe('true')
    expect(screen.queryByText('Drop to add to Money')).toBeNull()
  })

  test('pairing two loose pins marks the one being dropped onto', () => {
    setup()

    hover(pin('b::session'), pin('a::session'))

    expect(rowFor('Work — Session').getAttribute('data-paired')).toBe('true')
    expect(screen.getByText('Drop to group these two')).toBeTruthy()
  })

  test('the way out of a group lights up only once it is what a release would do', () => {
    setup()

    hover(pin('a::weekly', 'g1'), null)
    expect(screen.getByText('Drop here to leave the group').getAttribute('data-drop')).toBeNull()

    act(() =>
      context().onDragOver?.({
        active: pin('a::weekly', 'g1'),
        over: LEAVE_ZONE,
      } as unknown as DragOverEvent),
    )
    expect(screen.getByText('Drop here to leave the group').getAttribute('data-drop')).toBe('true')
  })

  test('reordering groups draws the line above the group being landed on', () => {
    setup({
      groups: [
        { id: 'g1', name: 'Money', color: 'blue', members: [] },
        { id: 'g2', name: 'Current limit', color: 'red', members: [] },
      ],
    })

    hover(group('g2'), group('g1'))

    expect(screen.getByText('Money').closest('[data-insert-before="true"]')).toBeTruthy()
  })

  test('an abandoned drag leaves nothing lit up behind it', () => {
    setup()

    hover(pin('a::session'), group('g1'))
    act(() => context().onDragCancel?.({} as unknown as DragEndEvent))

    expect(screen.queryByText('Drop to add to Money')).toBeNull()
    expect(screen.queryByText('Drop here to leave the group')).toBeNull()
    expect(HANDLERS.onAddToGroup).not.toHaveBeenCalled()
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
