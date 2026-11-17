// App-wide state: the open tab and month, and what the backend returned.

import { api, type Plans, type Settings, type Txn } from './api'
import { ONE_OFF } from './money'
import { cats, newId, setCategories } from './categories.svelte'

export type Tab = 'home' | 'review' | 'activity' | 'plans' | 'settings'

export const ui = $state({
  tab: 'home' as Tab,
  /** Which budget period is shown: 0 = the current one, 1 = the one before, … */
  period: 0,
  /** Transaction open in the detail sheet. */
  open: null as string | null,
  /** Review shows payments marked Unknown instead of untagged ones. */
  reviewUnknown: false,
  /** Short message at the bottom of the screen. */
  toast: '',
})

export const data = $state({
  loaded: false,
  txns: [] as Txn[],
  plans: { salary: 0, save_percent: 20, bills: [], bills_paid: {}, weekly: {}, goal: null } as Plans,
  settings: { own_numbers: [], amount_rules: {} } as Settings,
})

export async function reload() {
  const [txns, plans, settings] = await Promise.all([api.transactions(), api.plans(), api.settings()])
  data.txns = txns
  data.plans = plans
  data.settings = settings
  setCategories(settings.categories)
  data.loaded = true
}

/** Add a category (from a picker's "New category…") and return its id. */
export async function addCategory(kind: 'spend' | 'income', label: string, emoji: string): Promise<string> {
  const id = newId(label)
  const lists = $state.snapshot(cats)
  lists[kind].push({ id, label: label.trim(), emoji: emoji.trim() || '•' })
  await api.saveSettings({ ...$state.snapshot(data.settings), categories: lists })
  await reload()
  return id
}

let toastTimer: ReturnType<typeof setTimeout> | undefined
export function toast(message: string) {
  ui.toast = message
  clearTimeout(toastTimer)
  toastTimer = setTimeout(() => (ui.toast = ''), 3500)
}

/** Tag a transaction and refresh; says so when others were tagged along. */
export async function categorize(t: Txn, category: string | null, remember: boolean) {
  // "Lent" is about this payment, not everything paid to this person.
  const more = await api.categorize(t.id, category, remember && !ONE_OFF.has(category ?? ''))
  await reload()
  if (more > 0) toast(`Also tagged ${more} more like it`)
}

/** Mark a fixed bill paid (or not) by hand for one budget period. */
export async function toggleBillPaid(periodKey: string, billId: string) {
  const plans = $state.snapshot(data.plans)
  const paid = new Set(plans.bills_paid[periodKey] ?? [])
  if (paid.has(billId)) paid.delete(billId)
  else paid.add(billId)
  plans.bills_paid[periodKey] = [...paid]
  await api.savePlans(plans)
  await reload()
}

/**
 * Tag several payments at once. `category` null leaves categories as they
 * are; `note` null leaves notes as they are ("" clears them). With
 * `remember`, the category is also used for future payments to the same
 * recipient (only offered when they all go to one recipient).
 */
export async function tagMany(ids: string[], category: string | null, note: string | null, remember: boolean) {
  const learn = remember && category !== null && !ONE_OFF.has(category)
  for (const [i, id] of ids.entries()) {
    if (category !== null) await api.categorize(id, category, learn && i === 0)
    if (note !== null) await api.setNote(id, note)
  }
  await reload()
  toast(ids.length === 1 ? 'Saved' : `Saved ${ids.length} payments`)
}
