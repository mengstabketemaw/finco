<script lang="ts">
  import DayChart from './DayChart.svelte'
  import PeriodBar from './PeriodBar.svelte'
  import { STATE_TEXT, balances, billStates, budget, inPeriod, insights, owed, periods, span, totals, weekLabel, weekRows, weekStart } from './budget'
  import { SOURCES, birr, category, dayLabel, title } from './money'
  import { data, toggleBillPaid, ui } from './store.svelte'

  const all = $derived(periods(data.txns, data.plans))
  const index = $derived(Math.min(ui.period, all.length - 1))
  const p = $derived(all[index])
  const list = $derived(data.txns.filter((t) => inPeriod(t, p)))
  const running = $derived(p.end === null)
  const s = $derived(span(p))
  const tot = $derived(totals(list))
  const b = $derived(budget(list, p, data.plans))
  const bills = $derived(billStates(list, p, data.plans).filter((x) => x.bill.amount > 0 || x.paid))
  const week = $derived(weekStart(Date.now()))
  const rows = $derived(running ? weekRows(data.txns, data.plans, week) : [])
  const notes = $derived(insights(data.txns, all, index, data.plans))
  const maxCat = $derived(Math.max(1, ...tot.byCategory.map((c) => c.amount)))
  const accounts = $derived(balances(data.txns))
  const debt = $derived(owed(data.txns))
  const goal = $derived(data.plans.goal)
  const unknownHere = $derived(tot.byCategory.find((c) => c.id === 'unknown'))
  /** Lent and borrowed money not paid back yet, any period. */
  const open = (cat: string) =>
    data.txns
      .filter((t) => t.category === cat && t.amount > t.repaid)
      .sort((a, b) => b.occurred_at.localeCompare(a.occurred_at))
  const owedToMe = $derived(open('lent'))
  const iOwe = $derived(open('borrowed'))
  const sum = (ts: typeof owedToMe) => ts.reduce((s, t) => s + t.amount - t.repaid, 0)
</script>

<div class="head"><PeriodBar /></div>

{#if data.plans.salary === 0}
  <button class="callout" onclick={() => (ui.tab = 'plans')}>
    <span class="spacer"><strong>Set your salary</strong><br /><span class="small">Then the budget runs from one salary to the next.</span></span>
    <span aria-hidden="true">Plans ›</span>
  </button>
{/if}

<section class="summary">
  {#if running && p.kind === 'cycle'}
    <div class="card big">
      <span class="muted">Left to spend</span>
      <strong class:over={b.left < 0}>{birr(b.left, true)}</strong>
      <span class="muted small">
        {#if b.perDay !== null && b.left > 0}≈ {birr(b.perDay, true)} a day for {s.left + 1} days{:else if b.left < 0}over what this salary allows{/if}
      </span>
    </div>
    <div class="card">
      <span class="muted">Spent</span>
      <strong>{birr(b.spent, true)}</strong>
      <span class="muted small">of {birr(b.income, true)} in</span>
    </div>
    <div class="card">
      <span class="muted">Savings</span>
      <strong class:good={b.projectedSaved !== null && b.projectedSaved >= b.saveTarget} class:over={b.projectedSaved !== null && b.projectedSaved < b.saveTarget}>{birr(b.saveTarget, true)}</strong>
      <span class="muted small">{data.plans.save_percent}% kept aside</span>
    </div>
  {:else}
    <div class="card big">
      <span class="muted">Spent</span>
      <strong>{birr(b.spent, true)}</strong>
      <span class="muted small">fees included</span>
    </div>
    <div class="card">
      <span class="muted">In</span>
      <strong class="in">{birr(b.income, true)}</strong>
    </div>
    <div class="card">
      <span class="muted">Kept</span>
      <strong class:good={b.saved >= b.saveTarget && b.saved > 0} class:over={b.saved < 0}>{birr(b.saved, true)}</strong>
      {#if p.kind === 'cycle'}<span class="muted small">target {birr(b.saveTarget, true)}</span>{/if}
    </div>
  {/if}
</section>

{#if tot.untagged.count > 0}
  <button class="callout" onclick={() => (ui.tab = 'review')}>
    <span class="badge" aria-hidden="true">?</span>
    <span class="spacer">
      <strong>{tot.untagged.count} payments need a category</strong><br />
      <span class="small">ETB {birr(tot.untagged.amount, true)} you can't see in the breakdown yet</span>
    </span>
    <span aria-hidden="true">Review ›</span>
  </button>
{/if}
{#if unknownHere}
  <button class="callout quiet" onclick={() => ((ui.reviewUnknown = true), (ui.tab = 'review'))}>
    <span class="badge" aria-hidden="true">❔</span>
    <span class="spacer">
      <strong>{unknownHere.count} marked Unknown</strong><br />
      <span class="small">ETB {birr(unknownHere.amount, true)}. Tag them when you remember.</span>
    </span>
    <span aria-hidden="true">Review ›</span>
  </button>
{/if}

<div class="cols">
  <div class="stack">
    {#if running}
      <section class="card">
        <div class="row"><h3>This week</h3><span class="spacer"></span><span class="muted small">{weekLabel(week)}</span></div>
        {#if rows.length === 0}
          <p class="small muted">No weekly limits yet. <button class="ghost link" onclick={() => (ui.tab = 'plans')}>Set them in Plans</button></p>
        {:else}
          <ul class="bars">
            {#each rows as r (r.id)}
              {@const cat = category(r.id)}
              <li>
                <div class="row">
                  <span aria-hidden="true">{cat.emoji}</span>
                  <span>{cat.label}</span>
                  <span class="spacer"></span>
                  <span class="small state {r.state}">{STATE_TEXT[r.state]}</span>
                  <span class="amt">{birr(r.spent, true)} <span class="muted small">/ {birr(r.limit, true)}</span></span>
                </div>
                <div class="track">
                  <div class="fill {r.state}" style:width="{Math.min(100, (r.spent / r.limit) * 100)}%"></div>
                  <div class="pace" style:left="{r.pace * 100}%" title="Where you'd be at an even pace"></div>
                </div>
              </li>
            {/each}
          </ul>
        {/if}
      </section>
    {/if}

    <section class="card">
      <h3>Where it went</h3>
      {#if tot.byCategory.length === 0}
        <p class="empty">Nothing tagged yet.</p>
      {:else}
        <ul class="bars">
          {#each tot.byCategory as c (c.id)}
            {@const cat = category(c.id)}
            <li>
              <div class="row">
                <span aria-hidden="true">{cat.emoji}</span>
                <span>{cat.label}</span>
                <span class="spacer"></span>
                <span class="amt">{birr(c.amount, true)}</span>
              </div>
              <div class="track"><div class="fill" style:width="{(c.amount / maxCat) * 100}%"></div></div>
            </li>
          {/each}
        </ul>
      {/if}
    </section>

    <section class="card">
      <DayChart
        {list}
        period={p}
        days={s.total}
        elapsed={s.elapsed}
        {running}
        spendable={p.kind === 'cycle' ? b.income - b.saveTarget : null}
      />
    </section>
  </div>

  <div class="stack">
    {#if bills.length}
      <section class="card">
        <h3>Bills</h3>
        <ul class="list">
          {#each bills as x (x.bill.id)}
            <li class="row">
              <span aria-hidden="true">{category(x.bill.category).emoji}</span>
              <span>{x.bill.name}</span>
              <span class="spacer"></span>
              <span class="amt">{birr(x.paidAmount || x.bill.amount, true)}</span>
              {#if x.paid && !x.manual}
                <span class="tag paid">paid</span>
              {:else}
                <button class="tag" class:paid={x.manual} onclick={() => toggleBillPaid(p.key, x.bill.id)} title={x.manual ? 'Marked paid by hand. Tap to undo.' : 'Paid in cash? Mark it paid.'}>
                  {x.manual ? 'paid ✓' : 'mark paid'}
                </button>
              {/if}
            </li>
          {/each}
        </ul>
        {#if b.reserved > 0}<p class="small muted">{birr(b.reserved, true)} kept aside for unpaid bills.</p>{/if}
      </section>
    {/if}

    <section class="card">
      <h3>{running ? 'So far' : 'How it went'}</h3>
      <ul class="notes">
        {#each notes as n}
          <li class={n.tone}>{n.text}</li>
        {/each}
      </ul>
    </section>

    {#if goal}
      <section class="card">
        <h3>{goal.name}</h3>
        <div class="track tall"><div class="fill good" style:width="{Math.min(100, (goal.saved / goal.target) * 100)}%"></div></div>
        <p class="small">ETB {birr(goal.saved, true)} of {birr(goal.target, true)}</p>
      </section>
    {/if}

    {#if owedToMe.length || iOwe.length}
      <section class="card">
        <h3>Lent &amp; borrowed</h3>
        {#each [['Owed to me', owedToMe], ['I owe', iOwe]] as const as [label, list]}
          {#if list.length}
            <div class="row owe-head"><strong>{label}</strong><span class="spacer"></span><strong>{birr(sum(list), true)}</strong></div>
            <ul class="list">
              {#each list as t (t.id)}
                <li>
                  <button class="ghost owe" onclick={() => (ui.open = t.id)}>
                    <span>{title(t)} <span class="muted small">{dayLabel(t.occurred_at)}</span></span>
                    <span>{birr(t.amount - t.repaid, true)}</span>
                  </button>
                </li>
              {/each}
            </ul>
          {/if}
        {/each}
      </section>
    {/if}

    <section class="card">
      <h3>Credit &amp; fees</h3>
      <dl>
        <dt>Fees this period</dt>
        <dd>ETB {birr(tot.fees)}</dd>
        {#if debt}
          <dt>Owed on telebirr credit</dt>
          <dd class:near={debt.amount > 0}>ETB {birr(debt.amount)}</dd>
        {/if}
      </dl>
    </section>

    <section class="card">
      <h3>Balances</h3>
      <dl>
        {#each accounts as a (a.source)}
          <dt>{SOURCES[a.source] ?? a.source} <span class="muted small">{dayLabel(a.at)}</span></dt>
          <dd>{birr(a.balance)}</dd>
        {/each}
      </dl>
    </section>
  </div>
</div>

<style>
  .head {
    display: flex;
    justify-content: center;
    margin-bottom: 0.75rem;
  }
  .summary {
    display: grid;
    grid-template-columns: 1.4fr 1fr 1fr;
    gap: 0.6rem;
  }
  .summary .card {
    display: flex;
    flex-direction: column;
    padding: 0.75rem 0.9rem;
  }
  .summary strong {
    font-size: 1.35rem;
  }
  .summary .big strong {
    font-size: 1.9rem;
    line-height: 1.15;
  }
  .good {
    color: var(--in);
  }
  .over {
    color: var(--danger);
  }
  .near {
    color: var(--warn);
  }
  .callout {
    display: flex;
    align-items: center;
    gap: 0.75rem;
    width: 100%;
    text-align: left;
    margin: 0.75rem 0 0;
    padding: 0.75rem 0.9rem;
    border-radius: var(--radius);
    background: var(--warn-bg);
    border-color: transparent;
    color: var(--warn);
  }
  .callout:hover:not(:disabled) {
    background: var(--warn-bg);
    filter: brightness(0.98);
  }
  .callout.quiet {
    background: var(--surface-2);
    color: var(--muted);
  }
  .callout.quiet:hover:not(:disabled) {
    background: var(--surface-2);
  }
  .callout .badge {
    background: var(--surface);
    font-weight: 700;
  }
  .callout strong {
    color: var(--text);
  }
  .head + .callout {
    margin: 0 0 0.75rem;
  }
  .cols > *,
  .summary > * {
    min-width: 0;
  }
  .cols {
    display: grid;
    grid-template-columns: 1.5fr 1fr;
    gap: 0.75rem;
    margin-top: 0.75rem;
    align-items: start;
  }
  h3 {
    margin-bottom: 0.6rem;
  }
  .row h3 {
    margin: 0;
  }
  .row:has(h3) {
    margin-bottom: 0.6rem;
  }
  ul {
    list-style: none;
    margin: 0;
    padding: 0;
  }
  .bars {
    display: flex;
    flex-direction: column;
    gap: 0.65rem;
  }
  .amt {
    font-weight: 600;
  }
  .state.ok {
    color: var(--muted);
  }
  .state.ahead,
  .state.near {
    color: var(--warn);
  }
  .state.over {
    color: var(--danger);
  }
  .track {
    position: relative;
    height: 6px;
    margin-top: 0.25rem;
    border-radius: 3px;
    background: var(--surface-2);
  }
  .track.tall {
    height: 10px;
    border-radius: 5px;
  }
  .fill {
    height: 100%;
    border-radius: inherit;
    background: var(--bar);
  }
  .fill.ok,
  .fill.good {
    background: var(--accent);
  }
  .fill.ahead,
  .fill.near {
    background: var(--warn);
  }
  .fill.over {
    background: var(--danger);
  }
  .pace {
    position: absolute;
    top: -3px;
    bottom: -3px;
    width: 2px;
    background: var(--text);
    opacity: 0.45;
  }
  .list {
    display: flex;
    flex-direction: column;
    gap: 0.45rem;
  }
  .tag {
    font-size: 0.75rem;
    padding: 0.1rem 0.5rem;
    border-radius: 999px;
  }
  .tag.paid {
    background: var(--in-bg);
    color: var(--in);
    border-color: transparent;
  }
  .owe-head {
    margin: 0.2rem 0 0.3rem;
  }
  .owe-head + .list {
    margin-bottom: 0.5rem;
  }
  .owe {
    display: flex;
    justify-content: space-between;
    width: 100%;
    padding: 0.15rem 0;
    text-align: left;
  }
  .notes {
    display: flex;
    flex-direction: column;
    gap: 0.5rem;
  }
  .notes li {
    padding-left: 0.7rem;
    border-left: 3px solid var(--border);
    font-size: 0.92rem;
  }
  .notes li.good {
    border-color: var(--in);
  }
  .notes li.bad {
    border-color: var(--danger);
  }
  .link {
    padding: 0;
    color: var(--accent);
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
    font-weight: 600;
  }
  p {
    margin: 0.4rem 0 0;
  }
  @media (max-width: 760px) {
    .cols {
      grid-template-columns: 1fr;
    }
  }
  @media (max-width: 480px) {
    .summary {
      grid-template-columns: 1fr 1fr;
    }
    .summary .big {
      grid-column: 1 / -1;
    }
  }
</style>
