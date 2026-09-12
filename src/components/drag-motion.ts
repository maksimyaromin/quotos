import type { Transition } from 'framer-motion'

// One motion language for the whole of the customize screen: picking a
// row up, the rows that move aside for it, and where it settles when it
// lands all read as the same physical thing. Nothing on that screen
// eases or jumps; everything springs, and these are the only two springs
// there are.

// The row under the pointer, and anything that has to answer the pointer
// at once: stiff and barely overshooting, so a pickup reads as taking
// hold of something rather than as a delay.
export const GRAB_SPRING: Transition = {
  type: 'spring',
  stiffness: 460,
  damping: 30,
  mass: 0.85,
}

// Rows moving aside, and a group box growing to take one in. Slower and
// heavier than the grab: these are being pushed, not carried, and a
// little overshoot is what reads as weight rather than as a redraw.
export const REFLOW_SPRING: Transition = {
  type: 'spring',
  stiffness: 300,
  damping: 26,
  mass: 1,
}

// How far a picked-up row lifts off the list. Small enough to stay the
// same row, large enough that the moment of pickup is unmistakable.
export const PICKED_UP = { scale: 1.035, rotate: -1.1 }
export const AT_REST = { scale: 1, rotate: 0 }

// macOS's Reduce Motion setting reaches the stylesheet through the
// `--dur-*` tokens, which a spring driven in JavaScript never reads; see
// "Design system styling" in docs/contributing.md. Honour it here
// instead, by arriving at the same end state with no motion at all.
export const INSTANT: Transition = { duration: 0 }

export function grabTransition(reducedMotion: boolean): Transition {
  return reducedMotion ? INSTANT : GRAB_SPRING
}

export function reflowTransition(reducedMotion: boolean): Transition {
  return reducedMotion ? INSTANT : REFLOW_SPRING
}

export function pickedUp(reducedMotion: boolean): typeof PICKED_UP {
  return reducedMotion ? AT_REST : PICKED_UP
}

// The drop is the one moment the drag library animates itself, from
// where the row was let go to where it has landed. Given the same
// overshoot as the springs above so the settle belongs to them, rather
// than the library's own linear default.
export const DROP_SETTLE = {
  duration: 320,
  easing: 'cubic-bezier(0.2, 1.24, 0.4, 1)',
}

export function dropSettle(reducedMotion: boolean): typeof DROP_SETTLE {
  return reducedMotion ? { duration: 0, easing: 'linear' } : DROP_SETTLE
}
