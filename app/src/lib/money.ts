import type { Kind, Txn } from './api'

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

// ── Categories ──────────────────────────────────────────────────────────────

export interface Category {
  id: string
  label: string
  emoji: string
}

export const SPEND_CATEGORIES: Category[] = [
  { id: 'food', label: 'Food', emoji: '🍲' },
  { id: 'groceries', label: 'Groceries', emoji: '🛒' },
  { id: 'snacks', label: 'Coffee & snacks', emoji: '☕' },
  { id: 'transport', label: 'Transport', emoji: '🚕' },
  { id: 'internet', label: 'Internet & phone', emoji: '📶' },
  { id: 'rent', label: 'Rent', emoji: '🏠' },
  { id: 'electricity', label: 'Electricity', emoji: '💡' },
  { id: 'family', label: 'Family', emoji: '👪' },
  { id: 'friends', label: 'Friends', emoji: '🤝' },
  { id: 'shopping', label: 'Shopping', emoji: '🛍️' },
  { id: 'health', label: 'Health', emoji: '💊' },
  { id: 'education', label: 'Education', emoji: '📚' },
  { id: 'fun', label: 'Fun', emoji: '🎉' },
  { id: 'impulse', label: 'Impulse', emoji: '⚡' },
  { id: 'big', label: 'Big one-offs', emoji: '💸' },
  { id: 'lent', label: 'Lent', emoji: '🤲' },
  { id: 'fees', label: 'Fees', emoji: '🏦' },
  { id: 'other', label: 'Other', emoji: '•' },
  { id: 'unknown', label: 'Unknown', emoji: '❔' },
]

/** Categories that get a weekly limit: day-to-day spending, not bills or one-offs. */
export const WEEKLY_CATEGORIES = SPEND_CATEGORIES.filter(
  (c) => !['rent', 'electricity', 'big', 'fees', 'lent', 'unknown'].includes(c.id),
)

/** Lent money that never came back: spending after all. Set only by "Write off". */
const WRITTEN_OFF: Category = { id: 'written_off', label: 'Written off', emoji: '🫥' }

export const INCOME_CATEGORIES: Category[] = [
  { id: 'salary', label: 'Salary', emoji: '💼' },
  { id: 'family_in', label: 'From family', emoji: '👪' },
  { id: 'refund', label: 'Refund', emoji: '↩️' },
  { id: 'borrowed', label: 'Borrowed', emoji: '🫴' },
  { id: 'other_in', label: 'Other', emoji: '•' },
]

const ALL = new Map([...SPEND_CATEGORIES, ...INCOME_CATEGORIES, WRITTEN_OFF].map((c) => [c.id, c]))

/** Categories about this one transaction, never remembered for the recipient. */
export const ONE_OFF = new Set(['lent', 'borrowed', 'written_off', 'unknown'])
const UNTAGGED: Category = { id: '', label: 'Untagged', emoji: '?' }

export function category(id: string | null): Category {
  return (id && ALL.get(id)) || UNTAGGED
}

export function emojiFor(t: Txn): string {
  if (t.settles.length) return '↩'
  if (t.flow === 'internal') return '⇄'
  if (t.flow === 'loan') return '💳'
  return t.category ? category(t.category).emoji : t.flow === 'income' ? '＋' : '?'
}
