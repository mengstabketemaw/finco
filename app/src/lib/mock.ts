// Browser stand-in for the backend: transactions come from
// public/dev-data.json (made by `cargo run -p finco-core --example export`),
// and edits are kept in localStorage.

import type { Api, Plans, Settings, Split, Suggestion, Txn } from './api'
import { recipientKey } from './money'

const KEY = 'finco.mock'

interface Saved {
  categories: Record<string, string | null>
  notes: Record<string, string>
  /** Repayment id → ids it pays back. */
  settles: Record<string, string[]>
  splits: Record<string, Split[]>
  /** Recipient key → category, learned from "remember". */
  learned: Record<string, string>
  plans: Plans
  settings: Settings
}

function load(): Saved {
  const empty: Saved = {
    categories: {},
    notes: {},
    settles: {},
    splits: {},
    learned: {},
    plans: {
      salary: 6000000,
      save_percent: 20,
      bills: [
        { id: 'rent', name: 'Rent', category: 'rent', amount: 0 },
        { id: 'electricity', name: 'Electricity', category: 'electricity', amount: 0 },
      ],
      bills_paid: {},
      weekly: {},
      goal: null,
    },
    settings: {
      own_numbers: [],
      amount_rules: { '1500': 'transport', '2000': 'transport', '2500': 'transport', '5000': 'transport' },
    },
  }
  try {
    const stored = JSON.parse(localStorage.getItem(KEY) ?? '{}')
    return {
      ...empty,
      ...stored,
      plans: { ...empty.plans, ...stored.plans },
      settings: { ...empty.settings, ...stored.settings },
    }
  } catch {
    return empty
  }
}

function save() {
  try {
    localStorage.setItem(KEY, JSON.stringify(saved))
  } catch {}
}

let saved = load()
let base: Txn[] | null = null

async function raw(): Promise<Txn[]> {
  if (!base) {
    const res = await fetch('/dev-data.json')
    base = res.ok ? await res.json() : []
  }
  return base!
}

/**
 * Category for a transaction: set by hand, else learned for its recipient,
 * else from finco-core (packages, fees), else by its exact amount.
 */
function categoryOf(t: Txn): string | null {
  if (t.id in saved.categories) return saved.categories[t.id]
  const key = recipientKey(t)
  if (t.flow === 'spend' && key && saved.learned[key]) return saved.learned[key]
  if (t.category) return t.category
  if (t.flow === 'spend' && t.kind === 'transfer_out') return saved.settings.amount_rules[String(t.amount)] ?? null
  return null
}

const cashBack = (t: Txn) => t.splits.filter((p) => p.category === 'cash_back').reduce((s, p) => s + p.amount, 0)

/** The same rules as finco_core::settle, simplified. */
async function views(): Promise<Txn[]> {
  const all = (await raw()).map((t) => ({
    ...t,
    category: categoryOf(t),
    note: saved.notes[t.id] ?? null,
    settles: saved.settles[t.id] ?? [],
    splits: saved.splits[t.id] ?? [],
    repaid: 0,
    counted: 0,
  }))
  // Each repayment is shared out over what it settles, earlier ones first.
  const open = new Map(all.map((t) => [t.id, t.amount - cashBack(t)]))
  const repaid = new Map<string, number>()
  for (const r of all.filter((t) => t.settles.length).sort((a, b) => a.occurred_at.localeCompare(b.occurred_at))) {
    let left = r.amount
    for (const id of r.settles) {
      const cap = open.get(id)
      if (cap === undefined) continue
      const got = repaid.get(id) ?? 0
      const give = Math.min(left, Math.max(0, cap - got))
      repaid.set(id, got + give)
      left -= give
    }
  }
  for (const t of all) {
    t.repaid = repaid.get(t.id) ?? 0
    const parked = t.settles.length > 0 || t.category === 'lent' || t.category === 'borrowed'
    t.counted = parked || (t.flow !== 'spend' && t.flow !== 'income') ? 0 : Math.max(0, t.amount - t.repaid - cashBack(t))
  }
  return all
}

export const mock: Api = {
  transactions: views,

  async link(repayId, settles) {
    saved.settles[repayId] = settles
    save()
  },

  async suggest(repayId) {
    const all = await views()
    const r = all.find((t) => t.id === repayId)
    if (!r) return []
    const same = (t: Txn) => !!t.counterparty && t.counterparty.toLowerCase() === r.counterparty?.toLowerCase()
    const at = (t: Txn) => Date.parse(t.occurred_at)
    return all
      .filter((t) => {
        const open = t.amount - t.repaid - cashBack(t)
        const wanted = r.flow === 'income' ? t.flow === 'spend' && !['fee', 'package', 'airtime'].includes(t.kind) : t.category === 'borrowed'
        return wanted && t.id !== r.id && t.settles.length === 0 && open > 0 && at(r) - at(t) < 60 * 864e5 && at(t) - at(r) < 7 * 864e5
      })
      .map((t): Suggestion => {
        const open = t.amount - t.repaid - cashBack(t)
        const marked = t.category === 'lent' || t.category === 'borrowed'
        const score = (same(t) ? 4 : 0) + (marked ? 2 : 0) + (open === r.amount ? 3 : open > r.amount ? 1 : 0) + (Math.abs(at(r) - at(t)) < 3 * 864e5 ? 1 : 0)
        return { txn: t, remaining: open, score, likely: (same(t) && open >= r.amount) || (marked && open === r.amount) }
      })
      .sort((a, b) => b.score - a.score || b.txn.occurred_at.localeCompare(a.txn.occurred_at))
  },

  async setRepaidBy(paymentId, repayIds) {
    for (const t of await raw()) {
      const list = saved.settles[t.id] ?? []
      const has = list.includes(paymentId)
      const want = repayIds.includes(t.id)
      if (has === want || t.id === paymentId) continue
      saved.settles[t.id] = want ? [...list, paymentId] : list.filter((x) => x !== paymentId)
    }
    save()
  },

  async setSplits(id, splits) {
    const parts = splits.filter((p) => p.amount > 0)
    if (parts.length) saved.splits[id] = parts
    else delete saved.splits[id]
    save()
  },

  async categorize(id, category, remember) {
    const all = await raw()
    const t = all.find((t) => t.id === id)
    if (!t) return 0
    const before = all.filter((o) => o.id !== id && categoryOf(o) === null)
    saved.categories[id] = category
    const key = recipientKey(t)
    if (remember && key && category) saved.learned[key] = category
    save()
    return before.filter((o) => categoryOf(o) !== null).length
  },

  async setNote(id, note) {
    saved.notes[id] = note
    save()
  },

  async plans() {
    return structuredClone(saved.plans)
  },

  async savePlans(plans) {
    saved.plans = structuredClone(plans)
    save()
  },

  async settings() {
    return structuredClone(saved.settings)
  },

  async saveSettings(settings) {
    saved.settings = structuredClone(settings)
    save()
  },
}
