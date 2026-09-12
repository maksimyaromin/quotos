import { cleanup, fireEvent, render, screen } from '@testing-library/react'
import { afterEach, describe, expect, test, vi } from 'vitest'
import { PinDestinationItems } from './pin-destination-items'

afterEach(cleanup)

const GROUPS = [
  { id: 'g1', name: 'Fable' },
  { id: 'g2', name: 'Money' },
]

describe('where a pin can go', () => {
  test('leads with standalone, the behaviour a pin had before groups existed', () => {
    render(<PinDestinationItems groups={GROUPS} />)
    expect(screen.getAllByRole('menuitem')[0].textContent).toBe('Pin standalone')
  })

  test('offers every existing group, then the way to make a new one', () => {
    render(<PinDestinationItems groups={GROUPS} />)
    expect(screen.getAllByRole('menuitem').map((item) => item.textContent)).toEqual([
      'Pin standalone',
      'Add to Fable',
      'Add to Money',
      'New group…',
    ])
  })

  test('still offers standalone and a new group when no group exists yet', () => {
    render(<PinDestinationItems />)
    expect(screen.getAllByRole('menuitem').map((item) => item.textContent)).toEqual([
      'Pin standalone',
      'New group…',
    ])
  })

  test('reports the chosen destination, null for standalone', () => {
    const onPin = vi.fn()
    render(<PinDestinationItems groups={GROUPS} onPin={onPin} />)
    fireEvent.click(screen.getByText('Add to Money'))
    expect(onPin).toHaveBeenCalledWith('g2')

    fireEvent.click(screen.getByText('Pin standalone'))
    expect(onPin).toHaveBeenLastCalledWith(null)
  })
})

describe('naming a new group in place', () => {
  function openNameField() {
    render(<PinDestinationItems groups={GROUPS} onCreateGroup={onCreateGroup} />)
    fireEvent.click(screen.getByText('New group…'))
    return screen.getByPlaceholderText('Group name')
  }
  const onCreateGroup = vi.fn()

  test('swaps the entry for a field rather than opening a dialog', () => {
    openNameField()
    expect(screen.queryByText('New group…')).toBeNull()
  })

  test('commits the trimmed name on Enter', () => {
    onCreateGroup.mockReset()
    const field = openNameField()
    fireEvent.change(field, { target: { value: '  Current limit  ' } })
    fireEvent.keyDown(field, { key: 'Enter' })
    expect(onCreateGroup).toHaveBeenCalledWith('Current limit')
  })

  test('commits on blur as well, and only once when Create took the click', () => {
    onCreateGroup.mockReset()
    const field = openNameField()
    fireEvent.change(field, { target: { value: 'Money' } })
    fireEvent.click(screen.getByText('Create'))
    fireEvent.blur(field)
    expect(onCreateGroup).toHaveBeenCalledTimes(1)
  })

  test('an empty name makes no group at all', () => {
    onCreateGroup.mockReset()
    const field = openNameField()
    fireEvent.keyDown(field, { key: 'Enter' })
    expect(onCreateGroup).not.toHaveBeenCalled()
    expect(screen.getByText('New group…')).toBeTruthy()
  })

  test('Escape abandons the name without making a group', () => {
    onCreateGroup.mockReset()
    const field = openNameField()
    fireEvent.change(field, { target: { value: 'Money' } })
    fireEvent.keyDown(field, { key: 'Escape' })
    expect(onCreateGroup).not.toHaveBeenCalled()
    expect(screen.getByText('New group…')).toBeTruthy()
  })
})
