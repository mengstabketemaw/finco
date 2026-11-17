import type { Kind, Txn } from './api'
import { cats, type Category } from './categories.svelte'

// ── Formatting ──────────────────────────────────────────────────────────────

const two = new Intl.NumberFormat('en-US', { minimumFractionDigits: 2, maximumFractionDigits: 2 })
const whole = new Intl.NumberFormat('en-US', { maximumFractionDigits: 0 })

/** 123456 santim → "1,234.56"; with `round`, "1,235". */
export function birr(santim: number, round = false): string {
  return (round ? whole : two).format(santim / 100)
}

/** Parse what someone typed ("1,500", "20.5") into santim. */
export function toSantim(text: string): number | null {
  const n = Number(text.replace(/,/g, '').trim())
  return Number.isFinite(n) && text.trim() !== '' ? Math.round(n * 100) : null
}

export const day = (t: Txn) => t.occurred_at.slice(0, 10)
export const time = (t: Txn) => t.occurred_at.slice(11, 16)

/** "Thu 30 Oct" from a date or an ISO timestamp. */
export function dayLabel(ymd: string): string {
  ymd = ymd.slice(0, 10)
  const [y, m, d] = ymd.split('-').map(Number)
  return new Date(y, m - 1, d).toLocaleDateString('en-GB', { weekday: 'short', day: 'numeric', month: 'short' })
}


// ── Labels ──────────────────────────────────────────────────────────────────

export const SOURCES: Record<string, string> = {
  telebirr: 'telebirr',
  cbe: 'CBE',
  cbebirr: 'CBE Birr',
  zemen: 'Zemen',
  yaya: 'YaYa',
  addis: 'Addis Bank',
}

const KIND_LABEL: Record<Kind, string> = {
  payment: 'Payment',
  package: 'Package',
  airtime: 'Airtime',
  transfer_out: 'Transfer',
  transfer_in: 'Received',
  deposit: 'Deposit',
  debit: 'Debit',
  credit: 'Credit',
  fee: 'Fee',
  loan_draw: 'Loan taken',
  loan_use: 'Credit used',
  loan_repay: 'Credit repaid',
}

/** The line people recognise a transaction by. */
export function title(t: Txn): string {
  if (t.kind === 'package') return t.description ?? 'Package'
  if (t.kind === 'airtime') return 'Airtime'
  if (t.kind === 'loan_use') return `${t.description ?? 'Credit'} used`
  if (t.counterparty === 'telebirr' && t.target_number && t.flow !== 'internal') return `telebirr ${t.target_number}`
  if (t.flow === 'internal') return `${SOURCES[t.source] ?? t.source} ${t.kind === 'transfer_in' || t.kind === 'credit' ? '←' : '→'} ${t.counterparty ?? 'own account'}`
  return tidy(t.counterparty) ?? t.description ?? KIND_LABEL[t.kind]
}

/** Smaller line under the title. */
export function subtitle(t: Txn, ownNumbers: string[]): string {
  const parts = [SOURCES[t.source] ?? t.source, time(t)]
  if (t.kind === 'package' || t.kind === 'airtime') {
    parts.push(t.target_number && ownNumbers.includes(t.target_number) ? 'your number' : `for ${t.target_number ?? '?'}`)
  } else if (t.flow !== 'internal' && t.flow !== 'loan') {
    parts.push(KIND_LABEL[t.kind])
  }
  return parts.join(' · ')
}

/** "SISAY BAYEW" and "sisay bayew" both → "Sisay Bayew". Merchant names keep their case. */
function tidy(name: string | null): string | null {
  if (!name) return null
  const looksLikePerson = /^[A-Za-z]+(?: [A-Za-z]+){1,2}$/.test(name)
  if (!looksLikePerson || /\b(PLC|SC|LTD)\b/.test(name)) return name
  return name.toLowerCase().replace(/\b\w/g, (c) => c.toUpperCase())
}

/**
 * Who a payment went to, for learning categories: the account (or merchant
 * code), else the name, else the number a package was for. Bank debits with
 * no detail have no key and are never auto-tagged.
 */
export function recipientKey(t: Txn): string | null {
  if (t.kind === 'package' || t.kind === 'airtime') return t.target_number ? `number:${t.target_number}` : null
  if (t.counterparty_account) return `${t.source}:acct:${t.counterparty_account}:${(t.counterparty ?? '').toLowerCase()}`
  if (t.counterparty) return `${t.source}:name:${t.counterparty.toLowerCase()}`
  return null
}

/**
 * A person's full name ("mahlet kebede tesfaye"), titles dropped: the same
 * person paid from telebirr, CBE and Zemen has a different key in each but one name.
 */
export function personName(t: Txn): string | null {
  const n = (t.counterparty?.trim().toLowerCase() ?? '').replace(/^(miss|mrs?|ms|dr)\.? /, '')
  return /^[a-z]+( [a-z]+){1,3}$/.test(n) ? n : null
}

/** Same recipient, or the same person under another account. */
export function sameRecipient(a: Txn, b: Txn): boolean {
  const k = recipientKey(a)
  if (k !== null && k === recipientKey(b)) return true
  const n = personName(a)
  return n !== null && n === personName(b)
}

// ── Categories ──────────────────────────────────────────────────────────────

export type { Category } from './categories.svelte'

/** Spending categories you can pick (not the removed ones). */
export const spendCategories = (): Category[] => cats.spend.filter((c) => !c.hidden)
/** Income categories you can pick. */
export const incomeCategories = (): Category[] => cats.income.filter((c) => !c.hidden)

/** Categories that get a weekly limit: day-to-day spending, not bills or one-offs. */
export const weeklyCategories = (): Category[] =>
  spendCategories().filter((c) => !['rent', 'electricity', 'big', 'fees', 'lent', 'unknown'].includes(c.id))

/** A part of a payment friends paid back in cash: not counted. Only in splits. */
export const CASH_BACK: Category = { id: 'cash_back', label: 'Paid back in cash', emoji: '💵' }

/** Not a category: a tile that opens the picker of money in that paid a payment back. */
export const MONEY_BACK: Category = { id: '__money_back', label: 'Money back', emoji: '↩' }

/** Lent money that never came back: spending after all. Set only by "Write off". */
const WRITTEN_OFF: Category = { id: 'written_off', label: 'Written off', emoji: '🫥' }

/** Categories about this one transaction, never remembered for the recipient. */
export const ONE_OFF = new Set(['lent', 'borrowed', 'written_off', 'unknown'])
const UNTAGGED: Category = { id: '', label: 'Untagged', emoji: '?' }

const SPECIAL = new Map([WRITTEN_OFF, CASH_BACK, MONEY_BACK].map((c) => [c.id, c]))

/** A category by id, removed ones too; an id no list knows any more shows as itself. */
export function category(id: string | null): Category {
  if (!id) return UNTAGGED
  return cats.spend.find((c) => c.id === id) ?? cats.income.find((c) => c.id === id) ?? SPECIAL.get(id) ?? { id, label: id, emoji: '•' }
}

/**
 * Where a payment's counted money went: its split parts (each capped to what
 * is left, in order), then the rest under the payment's own category (null:
 * untagged). Parts paid back in cash are already out of `counted`.
 */
export function pieces(t: Txn): { category: string | null; amount: number }[] {
  let left = t.counted
  const out: { category: string | null; amount: number }[] = []
  for (const p of t.splits ?? []) {
    if (p.category === CASH_BACK.id) continue
    const a = Math.min(p.amount, left)
    if (a > 0) out.push({ category: p.category, amount: a })
    left -= a
  }
  if (left > 0) out.push({ category: t.category, amount: left })
  return out
}

/** What's counted but not covered by split parts: the payment's own category's share. */
export function rest(t: Txn): number {
  const parts = (t.splits ?? []).filter((p) => p.category !== CASH_BACK.id).reduce((s, p) => s + p.amount, 0)
  return Math.max(0, t.counted - parts)
}

/** A payment with counted money that has no category. */
export const needsTag = (t: Txn) => t.flow === 'spend' && !t.category && rest(t) > 0

export function emojiFor(t: Txn): string {
  if (t.settles.length) return '↩'
  if (t.flow === 'internal') return '⇄'
  if (t.flow === 'loan') return '💳'
  return t.category ? category(t.category).emoji : t.flow === 'income' ? '＋' : '?'
}
