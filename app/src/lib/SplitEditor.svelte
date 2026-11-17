<script lang="ts">
  // Split a payment into parts with their own categories, e.g. 2,000 paid:
  // 1,000 friends gave back in cash (not counted), 300 transport, and the
  // rest under the payment's own category.
  import { api, type Split, type Txn } from './api'
  import { CASH_BACK, birr, category, spendCategories, toSantim } from './money'
  import { reload, toast } from './store.svelte'

  let { t }: { t: Txn } = $props()

  /** Lent, unknown and fees aren't parts you'd split off. */
  const choices = $derived([CASH_BACK, ...spendCategories().filter((c) => !['lent', 'unknown', 'fees'].includes(c.id))])

  let editing = $state(false)
  let rows = $state<{ amount: string; category: string }[]>([])
  let busy = $state(false)

  // Another payment opened: start closed.
  $effect(() => {
    void t.id
    editing = false
  })

  /** What can be split: the amount minus what came back through linked repayments. */
  const open = $derived(Math.max(0, t.amount - t.repaid))
  const parsed = $derived(rows.map((r) => ({ amount: toSantim(r.amount) ?? 0, category: r.category })))
  const used = $derived(parsed.reduce((s, p) => s + p.amount, 0))
  const left = $derived(open - used)
  const valid = $derived(parsed.every((p) => p.amount > 0 && p.category !== '') && left >= 0)

  const birrText = (santim: number) => (santim / 100).toFixed(2).replace(/\.00$/, '')

  function start() {
    rows = t.splits.length ? t.splits.map((p) => ({ amount: birrText(p.amount), category: p.category })) : [{ amount: '', category: '' }]
    editing = true
  }

  async function save(parts: Split[]) {
    busy = true
    try {
      await api.setSplits(t.id, parts)
      await reload()
      toast(parts.length ? 'Split saved' : 'Split removed')
      editing = false
    } finally {
      busy = false
    }
  }
</script>

{#if editing}
  <section class="box">
    <strong>✂ Split this payment</strong>
    <p class="small muted">
      ETB {birr(open)} to split{t.repaid ? ` (${birr(t.repaid)} already came back)` : ''}. Each part gets its own category;
      “{CASH_BACK.label}” isn't counted as spending.
    </p>
    {#each rows as r, i (i)}
      <div class="part">
        <input class="amt" inputmode="decimal" placeholder="Amount" bind:value={r.amount} aria-label="Part {i + 1} amount" />
        <select bind:value={r.category} aria-label="Part {i + 1} category">
          <option value="" disabled>Pick…</option>
          {#each choices as c (c.id)}<option value={c.id}>{c.emoji} {c.label}</option>{/each}
        </select>
        <button class="ghost" onclick={() => rows.splice(i, 1)} aria-label="Remove part {i + 1}">✕</button>
      </div>
    {/each}
    <button class="ghost small add" onclick={() => rows.push({ amount: left > 0 ? birrText(left) : '', category: '' })}>+ Add a part</button>
    <p class="small" class:bad={left < 0}>
      {#if left < 0}
        That's {birr(-left)} more than the payment.
      {:else}
        Rest: <strong>ETB {birr(left)}</strong> → {t.category ? `${category(t.category).emoji} ${category(t.category).label}` : left ? 'pick its category above' : 'nothing left'}
      {/if}
    </p>
    <div class="row">
      <button class="primary small" disabled={!valid || busy} onclick={() => save(parsed)}>Save split</button>
      <button class="small" onclick={() => (editing = false)}>Cancel</button>
      {#if t.splits.length}
        <span class="spacer"></span>
        <button class="ghost small" disabled={busy} onclick={() => save([])}>Remove split</button>
      {/if}
    </div>
  </section>
{:else if t.splits.length}
  <section class="box">
    <div class="row"><strong>✂ Split</strong><span class="spacer"></span><button class="ghost small" onclick={start}>Edit</button></div>
    {#each t.splits as p, i (i)}
      <div class="line"><span>{category(p.category).emoji} {category(p.category).label}</span><span>{birr(p.amount)}</span></div>
    {/each}
    <div class="line muted">
      <span>Rest · {t.category ? category(t.category).label : 'untagged'}</span>
      <span>{birr(Math.max(0, open - t.splits.reduce((s, p) => s + p.amount, 0)))}</span>
    </div>
  </section>
{:else}
  <button class="ghost small split" onclick={start}>✂ Split this payment</button>
{/if}

<style>
  .box {
    display: flex;
    flex-direction: column;
    gap: 0.4rem;
    background: var(--surface-2);
    border-radius: 8px;
    padding: 0.6rem 0.75rem;
  }
  p {
    margin: 0;
  }
  .part {
    display: flex;
    gap: 0.4rem;
    align-items: center;
  }
  .amt {
    width: 6.5rem;
    flex: none;
  }
  select {
    flex: 1;
    min-width: 0;
  }
  .add,
  .split {
    align-self: flex-start;
    padding: 0;
    color: var(--accent);
  }
  .line {
    display: flex;
    justify-content: space-between;
    gap: 0.75rem;
    font-size: 0.9rem;
  }
  .bad {
    color: var(--danger);
  }
  .small {
    font-size: 0.82rem;
  }
</style>
