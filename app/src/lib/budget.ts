// Budget periods and the numbers on the dashboard.
//
// A period runs from one salary to the next: the salary is a Zemen deposit
// close to the salary set in Plans, and it doesn't come on a fixed date.
// Before any salary is known (or with no salary set), periods are calendar
// months. Weekly limits run Monday to Sunday regardless of the salary.

import type { Bill, Plans, Txn } from './api'
import { birr, category, dayLabel } from './money'

const DAY = 86_400_000
/** Messages are in Addis Ababa time, UTC+3 all year. */
const TZ = 3 * 3_600_000
/** Days a salary is expected to last, for the per-day pace. */
const CYCLE_DAYS = 30

export const at = (t: Txn) => Date.parse(t.occurred_at)
/** Local midnight of the day containing `ms`. */
const dayStart = (ms: number) => Math.floor((ms + TZ) / DAY) * DAY - TZ
const isoDay = (ms: number) => new Date(ms + TZ).toISOString().slice(0, 10)

// ── Periods ─────────────────────────────────────────────────────────────────

export interface Period {
  /** Local date it starts; also the key for bills paid by hand. */
  key: string
  start: number
  /** Exclusive end; null while it is still running. */
  end: number | null
  salary: Txn | null
  kind: 'cycle' | 'month' | 'before'
}

export function isSalary(t: Txn, salary: number): boolean {
  return (
    salary > 0 &&
    t.flow === 'income' &&
    t.source === 'zemen' &&
    t.kind === 'deposit' &&
    Math.abs(t.amount - salary) <= salary * 0.05
  )
}

/** Newest first. */
export function periods(txns: Txn[], plans: Plans, now = Date.now()): Period[] {
  if (txns.length === 0) return [monthPeriod(now)]
  const earliest = Math.min(...txns.map(at))
  const salaries = txns.filter((t) => isSalary(t, plans.salary)).sort((a, b) => at(a) - at(b))
  const list: Period[] = []
  if (salaries.length === 0) {
    for (let p = monthPeriod(earliest); ; p = monthPeriod(p.end!)) {
      list.push(p)
      if (p.end! > now) break
    }
    list[list.length - 1].end = null
  } else {
    if (earliest < at(salaries[0])) {
      list.push({ key: isoDay(earliest), start: dayStart(earliest), end: at(salaries[0]), salary: null, kind: 'before' })
    }
    salaries.forEach((s, i) =>
      list.push({ key: isoDay(at(s)), start: at(s), end: i + 1 < salaries.length ? at(salaries[i + 1]) : null, salary: s, kind: 'cycle' }),
    )
  }
  return list.reverse()
}

function monthPeriod(ms: number): Period {
  const d = new Date(ms + TZ)
  const y = d.getUTCFullYear()
  const m = d.getUTCMonth()
  const start = Date.UTC(y, m, 1) - TZ
  return { key: isoDay(start), start, end: Date.UTC(y, m + 1, 1) - TZ, salary: null, kind: 'month' }
}

export function inPeriod(t: Txn, p: Period): boolean {
  const ms = at(t)
  return ms >= p.start && (p.end === null || ms < p.end)
}

export function periodLabel(p: Period): string {
  if (p.kind === 'month') {
    return new Date(p.start + TZ).toLocaleDateString('en-GB', { month: 'long', year: 'numeric', timeZone: 'UTC' })
  }
  const from = dayLabel(p.key).replace(/^\w+ /, '')
  if (p.kind === 'before') return `Before salary · to ${dayLabel(isoDay(p.end! - 1)).replace(/^\w+ /, '')}`
  return p.end === null ? `Salary ${from} → now` : `${from} – ${dayLabel(isoDay(p.end - 1)).replace(/^\w+ /, '')}`
}

/** Days in the period and how many have passed (today counts as passed). */
export function span(p: Period, now = Date.now()): { total: number; elapsed: number; left: number } {
  const expectedEnd = p.end ?? (p.kind === 'cycle' ? Math.max(p.start + CYCLE_DAYS * DAY, dayStart(now) + DAY) : p.start)
  const total = Math.max(1, Math.round((dayStart(expectedEnd - 1) + DAY - dayStart(p.start)) / DAY))
  const elapsed = Math.min(total, Math.max(1, Math.round((dayStart(Math.min(now, expectedEnd - 1)) - dayStart(p.start)) / DAY) + 1))
  return { total, elapsed, left: total - elapsed }
}

// ── Totals ──────────────────────────────────────────────────────────────────

export interface Totals {
  /** Money in, other than transfers between your own accounts. */
  income: number
  /** Spending including every fee. */
  spent: number
  /** Service fees, VAT, levies and credit charges. */
  fees: number
  byCategory: { id: string; amount: number; count: number }[]
  untagged: { count: number; amount: number }
}

export function totals(list: Txn[]): Totals {
  const cats = new Map<string, { amount: number; count: number }>()
  const add = (id: string, amount: number) => {
    const c = cats.get(id) ?? { amount: 0, count: 0 }
    c.amount += amount
    c.count++
    cats.set(id, c)
  }
  let income = 0
  let spent = 0
  let fees = 0
  const untagged = { count: 0, amount: 0 }
  for (const t of list) {
    fees += t.fee
    spent += t.fee
    // `counted`: what's left after friends paid their share back; 0 for
    // lent and borrowed money and for repayments.
    if (t.flow === 'income') income += t.counted
    if (t.flow !== 'spend' || t.counted === 0) continue
    spent += t.counted
    if (t.kind === 'fee') fees += t.counted
    else if (t.category) add(t.category, t.counted)
    else {
      untagged.count++
      untagged.amount += t.counted
    }
  }
  if (fees) add('fees', fees)
  const byCategory = [...cats].map(([id, c]) => ({ id, ...c })).sort((a, b) => b.amount - a.amount)
  return { income, spent, fees, byCategory, untagged }
}

// ── Bills ───────────────────────────────────────────────────────────────────

export interface BillState {
  bill: Bill
  /** Paid by a matching payment or marked paid by hand. */
  paid: boolean
  /** Paid this period through payments tagged with the bill's category. */
  paidAmount: number
  manual: boolean
}

export function billStates(list: Txn[], p: Period, plans: Plans): BillState[] {
  const manual = plans.bills_paid[p.key] ?? []
  return plans.bills.map((bill) => {
    const paidAmount = list.filter((t) => t.flow === 'spend' && t.category === bill.category).reduce((s, t) => s + t.counted, 0)
    const byHand = manual.includes(bill.id)
    return { bill, paidAmount, manual: byHand, paid: byHand || paidAmount > 0 }
  })
}

// ── The budget for a period ─────────────────────────────────────────────────

export interface Budget {
  /** Salary plus any other money in. */
  income: number
  saveTarget: number
  spent: number
  /** Bills not paid yet, kept aside. */
  reserved: number
  /** What can still be spent while keeping the savings target and the bills. */
  left: number
  perDay: number | null
  /** income − spent; for a running period, what's saved if nothing else is spent. */
  saved: number
  /** Saved at the end of the period if day-to-day spending keeps its pace so far. */
  projectedSaved: number | null
}

export function budget(list: Txn[], p: Period, plans: Plans, now = Date.now()): Budget {
  const tot = totals(list)
  const income = tot.income
  const saveTarget = p.kind === 'cycle' ? Math.round((income * plans.save_percent) / 100) : 0
  const bills = billStates(list, p, plans)
  const reserved = bills.filter((b) => !b.paid).reduce((s, b) => s + b.bill.amount, 0)
  const running = p.end === null
  const left = income - saveTarget - tot.spent - reserved
  const s = span(p, now)
  const perDay = running && s.left + 1 > 0 ? Math.floor(left / (s.left + 1)) : null

  let projectedSaved: number | null = null
  if (running && p.kind === 'cycle') {
    // Day-to-day pace: spending other than bills and one-offs.
    const fixed = new Set(['big', ...plans.bills.map((b) => b.category)])
    const dayToDay = list
      .filter((t) => t.flow === 'spend' && !fixed.has(t.category ?? ''))
      .reduce((sum, t) => sum + t.counted + t.fee, 0)
    projectedSaved = income - tot.spent - reserved - Math.round((dayToDay / s.elapsed) * s.left)
  }
  return { income, saveTarget, spent: tot.spent, reserved, left, perDay, saved: income - tot.spent, projectedSaved }
}

// ── Weekly limits ───────────────────────────────────────────────────────────

/** Monday 00:00 of the week containing `ms`. */
export function weekStart(ms: number): number {
  const d = dayStart(ms)
  const weekday = new Date(d + TZ).getUTCDay() // 0 = Sunday
  return d - ((weekday + 6) % 7) * DAY
}

export function weekLabel(start: number): string {
  return `${dayLabel(isoDay(start)).replace(/^\w+ /, '')} – ${dayLabel(isoDay(start + 6 * DAY)).replace(/^\w+ /, '')}`
}

export interface WeekRow {
  id: string
  spent: number
  limit: number
  /** Share of the week gone, 0–1. */
  pace: number
  state: 'ok' | 'ahead' | 'near' | 'over'
}

export function weekRows(txns: Txn[], plans: Plans, start: number, now = Date.now()): WeekRow[] {
  const end = start + 7 * DAY
  const pace = Math.min(1, Math.max(1 / 7, (dayStart(Math.min(now, end - 1)) - start) / DAY / 7 + 1 / 7))
  const spentBy = new Map<string, number>()
  for (const t of txns) {
    const ms = at(t)
    if (t.flow !== 'spend' || !t.category || ms < start || ms >= end) continue
    spentBy.set(t.category, (spentBy.get(t.category) ?? 0) + t.counted)
  }
  return Object.entries(plans.weekly)
    .filter(([, limit]) => limit > 0)
    .map(([id, limit]) => {
      const spent = spentBy.get(id) ?? 0
      const state: WeekRow['state'] =
        spent > limit ? 'over' : spent >= limit * 0.9 ? 'near' : spent > limit * pace * 1.15 ? 'ahead' : 'ok'
      return { id, spent, limit, pace, state }
    })
    .sort((a, b) => b.spent / b.limit - a.spent / a.limit)
}

export const STATE_TEXT: Record<WeekRow['state'], string> = {
  ok: 'on track',
  ahead: 'ahead of pace',
  near: 'almost used up',
  over: 'over',
}

// ── Analysis ────────────────────────────────────────────────────────────────

export interface Insight {
  tone: 'good' | 'bad' | 'info'
  text: string
}

export function insights(txns: Txn[], all: Period[], index: number, plans: Plans, now = Date.now()): Insight[] {
  const p = all[index]
  const list = txns.filter((t) => inPeriod(t, p))
  const tot = totals(list)
  const b = budget(list, p, plans, now)
  const out: Insight[] = []
  const running = p.end === null

  if (p.kind === 'cycle') {
    if (running && b.projectedSaved !== null) {
      const good = b.projectedSaved >= b.saveTarget
      out.push({
        tone: good ? 'good' : 'bad',
        text: `At this pace you'll keep about ${birr(Math.max(0, b.projectedSaved), true)} of your salary (target ${birr(b.saveTarget, true)}).`,
      })
    } else if (!running) {
      const good = b.saved >= b.saveTarget
      out.push({
        tone: good ? 'good' : 'bad',
        text:
          b.saved >= 0
            ? `Kept ${birr(b.saved, true)} (${Math.round((b.saved / Math.max(1, b.income)) * 100)}%) against a target of ${birr(b.saveTarget, true)}.`
            : `Spent ${birr(-b.saved, true)} more than came in.`,
      })
    }
  }

  // Weeks over each weekly limit.
  const weeks: number[] = []
  for (let w = weekStart(p.start); w < (p.end ?? now); w += 7 * DAY) weeks.push(w)
  for (const [id, limit] of Object.entries(plans.weekly)) {
    if (!limit) continue
    const over = weeks.filter((w) => weekRows(txns, plans, w, now).find((r) => r.id === id)?.state === 'over').length
    if (over) out.push({ tone: 'bad', text: `${category(id).label} went over its weekly limit in ${over} of ${weeks.length} weeks.` })
  }

  const top = tot.byCategory.find((c) => c.id !== 'fees')
  if (top && tot.spent > 0) {
    out.push({ tone: 'info', text: `Most went to ${category(top.id).label}: ${birr(top.amount, true)} (${Math.round((top.amount / tot.spent) * 100)}% of spending).` })
  }
  const impulse = tot.byCategory.find((c) => c.id === 'impulse')
  if (impulse) out.push({ tone: 'bad', text: `Impulse buys: ${birr(impulse.amount, true)} over ${impulse.count} payments.` })
  const unknown = tot.byCategory.find((c) => c.id === 'unknown')
  if (unknown) {
    out.push({ tone: 'info', text: `Unknown: ${birr(unknown.amount, true)} over ${unknown.count} payments. Tag them in Review when you remember.` })
  }
  const big = tot.byCategory.find((c) => c.id === 'big')
  if (big) out.push({ tone: 'info', text: `Big one-offs: ${birr(big.amount, true)}, kept out of the weekly limits.` })
  if (tot.fees) out.push({ tone: 'info', text: `Fees and credit charges: ${birr(tot.fees)}.` })

  // Compared with the period before, scaled to the same number of days.
  const prev = all[index + 1]
  if (prev) {
    const prevTot = totals(txns.filter((t) => inPeriod(t, prev)))
    const scale = running ? span(p, now).elapsed / span(prev, now).total : 1
    const prevBy = new Map(prevTot.byCategory.map((c) => [c.id, c.amount * scale]))
    const changes = tot.byCategory
      .filter((c) => c.id !== 'fees' && c.id !== 'big')
      .map((c) => ({ id: c.id, now: c.amount, before: prevBy.get(c.id) ?? 0 }))
      .filter((c) => c.before > 0 && Math.abs(c.now - c.before) >= 30000 && Math.abs(c.now - c.before) / c.before >= 0.25)
      .sort((a, b) => Math.abs(b.now - b.before) - Math.abs(a.now - a.before))
      .slice(0, 2)
    for (const c of changes) {
      const up = c.now > c.before
      out.push({
        tone: up ? 'bad' : 'good',
        text: `${category(c.id).label} is ${up ? 'up' : 'down'} ${Math.round((Math.abs(c.now - c.before) / c.before) * 100)}% on the period before${running ? ' at this point' : ''}.`,
      })
    }
  }

  if (tot.untagged.count) {
    out.push({ tone: 'info', text: `${tot.untagged.count} payments (${birr(tot.untagged.amount, true)}) aren't tagged yet, so the picture is incomplete.` })
  }
  return out
}

// ── Accounts ────────────────────────────────────────────────────────────────

/** Latest known balance of each account. */
export function balances(txns: Txn[]): { source: string; balance: number; at: string }[] {
  const latest = new Map<string, Txn>()
  for (const t of txns) {
    if (t.balance === null) continue
    const prev = latest.get(t.source)
    if (!prev || at(t) >= at(prev)) latest.set(t.source, t)
  }
  return [...latest.values()]
    .map((t) => ({ source: t.source, balance: t.balance!, at: t.occurred_at }))
    .sort((a, b) => b.balance - a.balance)
}

/** What you owe on telebirr credit, from the latest loan message. */
export function owed(txns: Txn[]): { amount: number; at: string } | null {
  let last: Txn | null = null
  for (const t of txns) if (t.outstanding !== null && (!last || at(t) >= at(last))) last = t
  return last ? { amount: last.outstanding!, at: last.occurred_at } : null
}
