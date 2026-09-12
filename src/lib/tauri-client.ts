import type { ScheduledRefreshEvent, StatusItemImage, StatusItemSegment } from '@/types/entities'
import * as live from './live-client'
import * as mock from './mock-client'

const isTauri = typeof window !== 'undefined' && '__TAURI_INTERNALS__' in window

const client = isTauri ? live : mock

export const listAccounts = client.listAccounts
export const fetchSnapshot = client.fetchSnapshot
export const hidePanel = client.hidePanel
export const onPanelVisibility = client.onPanelVisibility
export const setDetached = client.setDetached

export const dragWindowStep: () => Promise<void> = isTauri ? live.dragWindowStep : async () => {}
export const endWindowDrag: () => Promise<void> = isTauri ? live.endWindowDrag : async () => {}
export const renderStatusItem: (
  segments: StatusItemSegment[],
  worstUsedPercent: number,
  tooltip: string,
) => Promise<void> = isTauri ? live.renderStatusItem : async () => {}
// Outside Tauri there is no compositor to ask, and nothing else may
// draw this preview: a second renderer is exactly the drift this
// command exists to remove. See "One renderer, two surfaces" in
// docs/status-item-rendering.md.
export const renderStatusItemPreview: (
  segments: StatusItemSegment[],
  worstUsedPercent: number,
) => Promise<StatusItemImage | null> = isTauri ? live.renderStatusItemPreview : async () => null
export const onQuotaRefresh: (
  callback: (event: ScheduledRefreshEvent) => void,
) => Promise<() => void> = isTauri ? live.onQuotaRefresh : async () => () => {}
export const kickScheduler: () => Promise<void> = isTauri ? live.kickScheduler : async () => {}
export const onPanelBeakOffset: (callback: (offsetPx: number) => void) => Promise<() => void> =
  isTauri ? live.onPanelBeakOffset : async () => () => {}
export const onStatusItemGroupClicked: (
  callback: (groupId: string) => void,
) => Promise<() => void> = isTauri ? live.onStatusItemGroupClicked : async () => () => {}

export const startSignIn = client.startSignIn
export const submitSignInCode = client.submitSignInCode
export const cancelSignIn = client.cancelSignIn
export const forgetSignIn = client.forgetSignIn
export const onSignInFinished = client.onSignInFinished
export const statuslineStatus = client.statuslineStatus
export const statuslineEnable = client.statuslineEnable
export const statuslineDisable = client.statuslineDisable
