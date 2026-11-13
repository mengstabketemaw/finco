// What the UI asks of the backend. Inside the app this will be Tauri
// commands over finco-core and SQLite; in a plain browser (`npm run dev`) it
// is the mock in mock.ts, so the screens can be designed on real data.

import { invoke } from '@tauri-apps/api/core'
import { mock } from './mock'

export type Kind =
  | 'payment'
  | 'package'
  | 'airtime'
  | 'transfer_out'
  | 'transfer_in'
  | 'deposit'
  | 'debit'
  | 'credit'
  | 'fee'
  | 'loan_draw'
  | 'loan_use'
  | 'loan_repay'

export type Flow = 'spend' | 'income' | 'internal' | 'loan'

/** Money is in santim (1/100 birr). Mirrors finco_core::Txn. */
export interface Txn {
  id: string
  /** ISO 8601 with the +03:00 offset, so its first 10 characters are the local date. */
  occurred_at: string
  source: string
  kind: Kind
  flow: Flow
  amount: number
  fee: number
  balance: number | null
  outstanding: number | null
  counterparty: string | null
  counterparty_account: string | null
  account: string | null
  description: string | null
  target_number: string | null
  reference: string | null
  category: string | null
  pair_id: string | null
  note?: string | null
  /** Ids of earlier transactions this one pays back. */
  settles: string[]
  /** Paid back so far by later transactions. */
  repaid: number
  /**
   * What counts toward spending or income: amount minus repayments; 0 for
   * repayments, lent and borrowed money, own-account transfers and loans.
   */
  counted: number
}

/** An earlier transaction a repayment might settle. */
export interface Suggestion {
  txn: Txn
  /** Still open before this repayment. */
  remaining: number
  score: number
  likely: boolean
}

export interface Bill {
  id: string
  name: string
  /** Paying it = a payment tagged with this category. */
  category: string
  amount: number
}

/** All amounts in santim. */
export interface Plans {
  /**
   * Expected salary. A Zemen deposit within 5% of it is recognised as the
   * salary and starts a new budget cycle; 0 means calendar months.
   */
  salary: number
  /** Share of the salary to keep (pay yourself first), 0–100. */
  save_percent: number
  /** Fixed bills, paid once per salary cycle. */
  bills: Bill[]
  /** Bills marked paid by hand (paid in cash), per cycle start. */
  bills_paid: Record<string, string[]>
  /** Monday–Sunday limit per category id. */
  weekly: Record<string, number>
  goal: { name: string; target: number; saved: number } | null
}

export interface Settings {
  /** Your phone numbers, as 09XXXXXXXX. Packages and airtime for them are your own. */
  own_numbers: string[]
  /**
   * Untagged payments of exactly this many santim get this category
   * (taxi fares are always 15, 20, 25 or 50 birr).
   */
  amount_rules: Record<string, string>
}

export interface Api {
  transactions(): Promise<Txn[]>
  /**
   * Set the category of one transaction. With `remember`, future and
   * untagged payments to the same recipient get it too. Returns how many
   * other transactions were tagged along.
   */
  categorize(id: string, category: string | null, remember: boolean): Promise<number>
  setNote(id: string, note: string): Promise<void>
  /** Record that `repayId` pays back `settles` (empty: nothing). */
  link(repayId: string, settles: string[]): Promise<void>
  /** What a transaction might pay back, best first. */
  suggest(repayId: string): Promise<Suggestion[]>
  plans(): Promise<Plans>
  savePlans(plans: Plans): Promise<void>
  settings(): Promise<Settings>
  saveSettings(settings: Settings): Promise<void>
}

export interface SyncInfo {
  status: {
    url: string | null
    account: string | null
    pending: number
    last_received: string | null
    files: string
  }
  default_url: string
  default_key: string
}

export interface SmsInfo {
  messages: number
  unrecognized: { sender: string; body: string; received_at: number; reason: string }[]
  reads_sms: boolean
}

export const inApp = typeof window !== 'undefined' && '__TAURI_INTERNALS__' in window

/** The app: Tauri commands over finco-core and SQLite. */
const tauri: Api = {
  transactions: () => invoke('transactions'),
  categorize: (id, category, remember) => invoke('categorize', { id, category, remember }),
  setNote: (id, note) => invoke('set_note', { id, note }),
  link: (repayId, settles) => invoke('link', { repayId, settles }),
  suggest: (repayId) => invoke('suggest', { repayId }),
  plans: () => invoke('get_plans'),
  savePlans: (plans) => invoke('save_plans', { plans }),
  settings: () => invoke('get_settings'),
  saveSettings: (settings) => invoke('save_settings', { settings }),
}

export const api: Api = inApp ? tauri : mock

/** Only inside the app. */
export const appOnly = {
  smsInfo: () => invoke<SmsInfo>('sms_info'),
  syncStatus: () => invoke<SyncInfo>('sync_status'),
  syncLogin: (url: string, key: string, email: string, password: string, signup: boolean) =>
    invoke<string>('sync_login', { url, key, email, password, signup }),
  syncLogout: () => invoke<void>('sync_logout'),
}
