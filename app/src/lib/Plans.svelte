<script lang="ts">
  import { api, type Plans } from './api'
  import { isSalary, weekRows, weekStart } from './budget'
  import { SPEND_CATEGORIES, WEEKLY_CATEGORIES, birr, category, dayLabel, toSantim } from './money'
  import { data, reload, toast } from './store.svelte'

  /** A salary lasts about this many weeks. */
  const WEEKS = 30 / 7

  // Text being edited, so half-typed numbers aren't reformatted underfoot.
  let salary = $state('')
  let percent = $state(20)
  let bills = $state<{ id: string; name: string; category: string; amount: string }[]>([])
  let weekly = $state<Record<string, string>>({})
  let goalName = $state('')
  let goalTarget = $state('')
  let goalSaved = $state('')

  $effect(() => {
    const p = data.plans
    salary = p.salary ? birr(p.salary, true) : ''
    percent = p.save_percent
    bills = p.bills.map((b) => ({ ...b, amount: b.amount ? birr(b.amount, true) : '' }))
    weekly = Object.fromEntries(Object.entries(p.weekly).map(([k, v]) => [k, birr(v, true)]))
    goalName = p.goal?.name ?? ''
    goalTarget = p.goal ? birr(p.goal.target, true) : ''
    goalSaved = p.goal ? birr(p.goal.saved, true) : ''
  })

  const amount = (text: string | undefined) => toSantim(text ?? '') ?? 0
  const salaryAmt = $derived(amount(salary))
  const saving = $derived(Math.round((salaryAmt * percent) / 100))
  const billsAmt = $derived(bills.reduce((s, b) => s + amount(b.amount), 0))
  const weeklyAmt = $derived(Object.values(weekly).reduce((s, v) => s + amount(v), 0))
  const dayToDay = $derived(Math.round(weeklyAmt * WEEKS))
  const buffer = $derived(salaryAmt - saving - billsAmt - dayToDay)

  const lastSalary = $derived(
    data.txns.filter((t) => isSalary(t, salaryAmt)).sort((a, b) => b.occurred_at.localeCompare(a.occurred_at))[0],
  )
  const thisWeek = $derived(new Map(weekRows(data.txns, { ...data.plans, weekly: Object.fromEntries(WEEKLY_CATEGORIES.map((c) => [c.id, 1])) }, weekStart(Date.now())).map((r) => [r.id, r.spent])))

  async function save() {
    const target = toSantim(goalTarget)
    const plans: Plans = {
      ...$state.snapshot(data.plans),
      salary: salaryAmt,
      save_percent: percent,
      bills: bills
        .filter((b) => b.name.trim())
        .map((b) => ({ id: b.id, name: b.name.trim(), category: b.category, amount: amount(b.amount) })),
      weekly: Object.fromEntries(Object.entries(weekly).map(([k, v]) => [k, amount(v)] as const).filter(([, v]) => v > 0)),
      goal: goalName.trim() && target ? { name: goalName.trim(), target, saved: toSantim(goalSaved) ?? 0 } : null,
    }
    await api.savePlans(plans)
    await reload()
    toast('Plans saved')
  }

  function addBill() {
    bills.push({ id: crypto.randomUUID(), name: '', category: 'other', amount: '' })
  }
</script>

<div class="row top">
  <h2>Plans</h2>
  <span class="spacer"></span>
  <button class="primary" onclick={save}>Save plans</button>
</div>

<div class="cols">
  <div class="stack">
    <section class="card stack">
      <h3>Salary</h3>
      <label class="field">
        <span class="small muted">Monthly salary (ETB)</span>
        <input inputmode="decimal" bind:value={salary} placeholder="60,000" />
      </label>
      <p class="small muted">
        {#if lastSalary}Last seen {dayLabel(lastSalary.occurred_at)}: {birr(lastSalary.amount, true)} into Zemen. Each one starts a new budget period.
        {:else if salaryAmt}No Zemen deposit of about {birr(salaryAmt, true)} found yet.{:else}A Zemen deposit of about this amount starts a new budget period.{/if}
      </p>

      <label class="field">
        <span class="small muted">Keep aside from each salary</span>
        <div class="row"><input type="range" min="0" max="60" step="5" bind:value={percent} /><strong>{percent}%</strong></div>
      </label>
      <p class="small">= ETB {birr(saving, true)} a month you don't plan to spend.</p>
    </section>

    <section class="card stack">
      <h3>Fixed bills</h3>
      <p class="small muted">Paid once per salary. Their money is kept aside until a payment tagged with their category appears, or you mark them paid.</p>
      {#each bills as b, i (b.id)}
        <div class="bill">
          <input bind:value={b.name} placeholder="Name" aria-label="Bill name" />
          <input class="num" inputmode="decimal" bind:value={b.amount} placeholder="0" aria-label="{b.name} amount" />
          <select bind:value={b.category} aria-label="{b.name} category">
            {#each SPEND_CATEGORIES as c (c.id)}<option value={c.id}>{c.emoji} {c.label}</option>{/each}
          </select>
          <button class="ghost" onclick={() => bills.splice(i, 1)} aria-label="Remove {b.name}">✕</button>
        </div>
      {/each}
      <div><button onclick={addBill}>Add bill</button></div>
    </section>

    <section class="card">
      <h3>Does it add up?</h3>
      <dl>
        <dt>Salary</dt><dd>{birr(salaryAmt, true)}</dd>
        <dt>Kept aside ({percent}%)</dt><dd>− {birr(saving, true)}</dd>
        <dt>Fixed bills</dt><dd>− {birr(billsAmt, true)}</dd>
        <dt>Weekly limits × {WEEKS.toFixed(1)} weeks</dt><dd>− {birr(dayToDay, true)}</dd>
        <dt class="total">{buffer >= 0 ? 'Unplanned buffer' : 'Planned more than you earn'}</dt>
        <dd class="total" class:good={buffer >= 0} class:over={buffer < 0}>{birr(Math.abs(buffer), true)}</dd>
      </dl>
    </section>
  </div>

  <div class="stack">
    <section class="card">
      <h3>Weekly limits</h3>
      <p class="small muted">Monday to Sunday. Leave empty for no limit. Bars show this week so far.</p>
      <ul>
        {#each WEEKLY_CATEGORIES as c (c.id)}
          {@const limit = amount(weekly[c.id])}
          {@const spent = thisWeek.get(c.id) ?? 0}
          <li>
            <span aria-hidden="true">{c.emoji}</span>
            <span class="label">{category(c.id).label}</span>
            <span class="muted small">{birr(spent, true)} /</span>
            <input class="num" inputmode="decimal" bind:value={weekly[c.id]} placeholder="—" aria-label="{c.label} weekly limit" />
            {#if limit}
              <div class="track"><div class="fill" class:over={spent > limit} class:near={spent <= limit && spent >= limit * 0.9} style:width="{Math.min(100, (spent / limit) * 100)}%"></div></div>
            {/if}
          </li>
        {/each}
      </ul>
    </section>

    <section class="card stack">
      <h3>Savings goal</h3>
      <label class="field"><span class="small muted">What for</span><input bind:value={goalName} placeholder="Emergency fund" /></label>
      <div class="two">
        <label class="field"><span class="small muted">Target (ETB)</span><input inputmode="decimal" bind:value={goalTarget} placeholder="100,000" /></label>
        <label class="field"><span class="small muted">Saved so far (ETB)</span><input inputmode="decimal" bind:value={goalSaved} placeholder="0" /></label>
      </div>
    </section>
  </div>
</div>

<style>
  .top {
    margin-bottom: 0.75rem;
  }
  .cols > *,
  .bill > *,
  li > * {
    min-width: 0;
  }
  .bill input,
  .bill select,
  li input {
    width: 100%;
  }
  .cols {
    display: grid;
    grid-template-columns: 1fr 1fr;
    gap: 0.75rem;
    align-items: start;
  }
  @media (max-width: 760px) {
    .cols {
      grid-template-columns: 1fr;
    }
  }
  .field {
    display: flex;
    flex-direction: column;
    gap: 0.2rem;
    min-width: 0;
  }
  .field > input:not([type='range']) {
    width: 100%;
  }
  .two > * {
    min-width: 0;
  }
  .two {
    display: grid;
    grid-template-columns: 1fr 1fr;
    gap: 0.5rem;
  }
  input[type='range'] {
    flex: 1;
    accent-color: var(--accent);
  }
  p {
    margin: 0;
  }
  .bill {
    display: grid;
    grid-template-columns: 1fr 6rem 9rem auto;
    gap: 0.4rem;
  }
  @media (max-width: 480px) {
    .bill {
      grid-template-columns: 1fr 5.5rem auto;
    }
    .bill select {
      grid-column: 1 / 3;
      grid-row: 2;
    }
  }
  .num {
    text-align: right;
  }
  ul {
    list-style: none;
    margin: 0.5rem 0 0;
    padding: 0;
    display: flex;
    flex-direction: column;
    gap: 0.5rem;
  }
  li {
    display: grid;
    grid-template-columns: 1.5rem 1fr auto 5.5rem;
    align-items: center;
    gap: 0.15rem 0.5rem;
  }
  .track {
    grid-column: 2 / -1;
    height: 4px;
    border-radius: 2px;
    background: var(--surface-2);
  }
  .fill {
    height: 100%;
    border-radius: inherit;
    background: var(--accent);
  }
  .fill.near {
    background: var(--warn);
  }
  .fill.over {
    background: var(--danger);
  }
  dl {
    display: grid;
    grid-template-columns: 1fr auto;
    gap: 0.35rem 1rem;
    margin: 0;
  }
  dd {
    margin: 0;
    text-align: right;
  }
  .total {
    font-weight: 700;
    border-top: 1px solid var(--border);
    padding-top: 0.35rem;
  }
  .good {
    color: var(--in);
  }
  .over {
    color: var(--danger);
  }
</style>
