import { cleanup, fireEvent, render, screen } from '@testing-library/react'
import { afterEach, describe, expect, test, vi } from 'vitest'
import type { StatusItemSegment } from '@/types/entities'
import {
  type CustomizeDisplayScreenProps,
  CustomizeDisplayScreen,
} from './customize-display-screen'

afterEach(cleanup)

function segment(overrides: Partial<StatusItemSegment> & { text: string }): StatusItemSegment {
  return { color: 'neutral', groupStart: false, groupId: null, groupColor: null, ...overrides }
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

function drag(from: HTMLElement, onto: HTMLElement) {
  fireEvent.mouseDown(from, { button: 0 })
  fireEvent.mouseMove(onto)
  fireEvent.mouseUp(window)
}

describe('dragging a pin onto another', () => {
  test('groups two standalone pins together, the target first', () => {
    setup()

    drag(screen.getByText('Home — Session'), screen.getByText('Work — Session'))

    expect(HANDLERS.onGroupTogether).toHaveBeenCalledWith(['a::session', 'b::session'])
  })

  test('dropped on a group member, it joins that group ahead of the member', () => {
    setup()

    drag(screen.getByText('Work — Session'), screen.getByText('Work — Weekly'))

    expect(HANDLERS.onAddToGroup).toHaveBeenCalledWith('a::session', 'g1', 'a::weekly')
  })

  test("dropped on a group's own box, it joins at the end", () => {
    setup()

    drag(screen.getByText('Work — Session'), screen.getByText('Money'))

    expect(HANDLERS.onAddToGroup).toHaveBeenCalledWith('a::session', 'g1')
  })

  test('dropped back on itself, nothing is rearranged', () => {
    setup()

    const row = screen.getByText('Work — Session')
    drag(row, row)

    expect(HANDLERS.onGroupTogether).not.toHaveBeenCalled()
    expect(HANDLERS.onAddToGroup).not.toHaveBeenCalled()
  })

  test('released over nothing at all, nothing is rearranged', () => {
    setup()

    fireEvent.mouseDown(screen.getByText('Work — Session'), { button: 0 })
    fireEvent.mouseUp(window)

    expect(HANDLERS.onGroupTogether).not.toHaveBeenCalled()
  })
})

describe('dragging a member out of its group', () => {
  test('offers the way out only while a grouped member is being dragged', () => {
    setup()
    expect(screen.queryByText('Drop here to leave the group')).toBeNull()

    fireEvent.mouseDown(screen.getByText('Work — Session'), { button: 0 })
    expect(screen.queryByText('Drop here to leave the group')).toBeNull()
    fireEvent.mouseUp(window)

    fireEvent.mouseDown(screen.getByText('Work — Weekly'), { button: 0 })
    expect(screen.getByText('Drop here to leave the group')).toBeTruthy()
  })

  test('returns it to standalone without unpinning it', () => {
    setup()

    fireEvent.mouseDown(screen.getByText('Work — Weekly'), { button: 0 })
    fireEvent.mouseMove(screen.getByText('Drop here to leave the group'))
    fireEvent.mouseUp(window)

    expect(HANDLERS.onMakeStandalone).toHaveBeenCalledWith('a::weekly')
  })
})

describe('dragging a group', () => {
  test('onto another group reorders the two', () => {
    setup({
      groups: [
        { id: 'g1', name: 'Money', color: 'blue', members: [] },
        { id: 'g2', name: 'Current limit', color: 'red', members: [] },
      ],
    })

    drag(screen.getByText('Current limit'), screen.getByText('Money'))

    expect(HANDLERS.onMoveGroup).toHaveBeenCalledWith('g2', 'g1')
  })

  test('onto itself changes no order', () => {
    setup()

    const header = screen.getByText('Money')
    drag(header, header)

    expect(HANDLERS.onMoveGroup).not.toHaveBeenCalled()
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

  test('a header click does not count as a drag that reorders anything', () => {
    setup()

    fireEvent.mouseDown(screen.getByText('Money'), { button: 0 })
    fireEvent.mouseUp(window)
    fireEvent.click(screen.getByText('Money'))

    expect(HANDLERS.onMoveGroup).not.toHaveBeenCalled()
    expect(screen.getByLabelText('Group name')).toBeTruthy()
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
  test('draws the segments it was handed, with each group figure underlined in its colour', () => {
    setup({
      preview: [
        segment({ text: '99%', color: 'red', groupId: 'g1', groupColor: 'blue' }),
        segment({ text: '12%', groupStart: true }),
      ],
    })

    const strip = screen.getByLabelText('Menu bar preview')
    const figures = Array.from(strip.querySelectorAll('[data-color]'))
    expect(figures.map((f) => f.getAttribute('data-color'))).toEqual(['red', 'blue', 'neutral'])
    expect(strip.textContent).toContain('99%')
    expect(strip.textContent).toContain('12%')
  })

  test('a standalone figure carries no colour to underline it with', () => {
    setup({ preview: [segment({ text: '12%' })] })

    const strip = screen.getByLabelText('Menu bar preview')
    const underline = strip.querySelector('span > span')
    expect(underline?.getAttribute('data-color')).toBeNull()
  })
})
