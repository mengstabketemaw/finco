// The categories you pick from. They live in the settings (so they sync)
// and can be added, renamed and removed in Settings; these are the defaults.

export interface Category {
  id: string
  label: string
  emoji: string
  /** Removed in Settings: not offered any more, but old payments keep it. */
  hidden?: boolean
}

export interface CategoryLists {
  spend: Category[]
  income: Category[]
}

export const DEFAULT_CATEGORIES: CategoryLists = {
  spend: [
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
  ],
  income: [
    { id: 'salary', label: 'Salary', emoji: '💼' },
    { id: 'family_in', label: 'From family', emoji: '👪' },
    { id: 'refund', label: 'Refund', emoji: '↩️' },
    { id: 'borrowed', label: 'Borrowed', emoji: '🫴' },
    { id: 'other_in', label: 'Other', emoji: '•' },
  ],
}

/** The app depends on these (lent and borrowed money, fees, "don't know"): they can be renamed, not removed. */
export const FIXED = new Set(['lent', 'unknown', 'fees', 'borrowed'])

/** The current lists, filled from the settings on every reload. */
export const cats = $state<CategoryLists>(structuredClone(DEFAULT_CATEGORIES))

/** Use the lists saved in the settings (or the defaults), keeping fixed ones present. */
export function setCategories(saved: CategoryLists | undefined) {
  const lists = saved ?? DEFAULT_CATEGORIES
  for (const kind of ['spend', 'income'] as const) {
    const list = structuredClone(lists[kind] ?? DEFAULT_CATEGORIES[kind])
    for (const d of DEFAULT_CATEGORIES[kind]) {
      if (FIXED.has(d.id) && !list.some((c) => c.id === d.id)) list.push(structuredClone(d))
    }
    cats[kind] = list.map((c) => (FIXED.has(c.id) ? { ...c, hidden: false } : c))
  }
}

/** A new id from a name, unique among both lists. */
export function newId(label: string): string {
  const base =
    'c_' +
    (label
      .toLowerCase()
      .replace(/[^a-z0-9]+/g, '_')
      .replace(/^_|_$/g, '') || 'category')
  const taken = new Set([...cats.spend, ...cats.income].map((c) => c.id))
  let id = base
  for (let i = 2; taken.has(id); i++) id = `${base}_${i}`
  return id
}
