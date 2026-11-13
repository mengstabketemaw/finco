<script lang="ts">
  // Spending per day of a budget period, two ways:
  // - Daily: one bar per day against the daily budget line. Days far above the
  //   rest (a one-off 18,000 transfer) are clipped with their value on top, so
  //   ordinary days stay readable.
  // - Running total: cumulative spending against the plan's even pace.
  // Hover or focus shows a day; tap or Enter opens its payments below.
  import TxnRow from './TxnRow.svelte'
  import type { Txn } from './api'
  import { at, type Period } from './budget'
  import { birr, dayLabel, title } from './money'

  let {
    list,
    period,
    days,
    elapsed,
    running,
    spendable,
  }: {
    list: Txn[]
    period: Period
    /** Days in the period (expected, for a running salary cycle). */
    days: number
    /** Days that have happened, today included. */
    elapsed: number
    running: boolean
    /** What the plan lets you spend over the whole period; null without a plan. */
    spendable: number | null
  } = $props()

  const DAY = 86_400_000
  const TZ = 3 * 3_600_000
  const H = 170
  const PAD = { top: 18, right: 8, bottom: 22, left: 44 }

  let view = $state<'daily' | 'running'>('daily')
  /** Measured from the container; nothing is drawn until it's known. */
  let width = $state(0)
  let hover = $state<number | null>(null)
  let selected = $state<number | null>(null)

  const startDay = $derived(Math.floor((period.start + TZ) / DAY))
  const dayIso = (i: number) => new Date((startDay + i) * DAY).toISOString().slice(0, 10)

  /** Payments per day, spending and fees only. */
  const byDay = $derived.by(() => {
    const out: Txn[][] = Array.from({ length: days }, () => [])
    for (const t of list) {
      if (t.flow !== 'spend' && !t.fee) continue
      const i = Math.floor((at(t) + TZ) / DAY) - startDay
      if (i >= 0 && i < days) out[i].push(t)
    }
    return out.map((ts) => ts.sort((a, b) => cost(b) - cost(a)))
  })
  const cost = (t: Txn) => (t.flow === 'spend' ? t.counted : 0) + t.fee
  const totals = $derived(byDay.map((ts) => ts.reduce((s, t) => s + cost(t), 0)))
  const cumulative = $derived(totals.reduce<number[]>((acc, v, i) => [...acc, (acc[i - 1] ?? 0) + v], []))
  const allowance = $derived(spendable !== null ? spendable / days : null)

  // Geometry.
  const plotW = $derived(Math.max(100, width - PAD.left - PAD.right))
  const plotH = H - PAD.top - PAD.bottom
  const band = $derived(plotW / days)
  const barW = $derived(Math.max(2, band - 2))
  const x = (i: number) => PAD.left + i * band

  /** Rounds up to 1, 2 or 5 × a power of ten, for tidy axis labels. */
  function nice(v: number): number {
    if (v <= 0) return 100
    const p = 10 ** Math.floor(Math.log10(v))
    return ([1, 2, 5, 10].find((m) => m * p >= v) ?? 10) * p
  }

  // Daily view: scale to typical days and clip the outliers.
  const dailyMax = $derived.by(() => {
    const nonzero = totals.filter((v) => v > 0).sort((a, b) => a - b)
    if (!nonzero.length) return nice(allowance ?? 10000)
    const p85 = nonzero[Math.floor((nonzero.length - 1) * 0.85)]
    const cap = Math.max(p85 * 1.4, (allowance ?? 0) * 1.5)
    const max = nonzero[nonzero.length - 1]
    return nice(max <= cap * 1.2 ? max : cap)
  })
  const runningMax = $derived(nice(Math.max(cumulative[cumulative.length - 1] ?? 0, spendable ?? 0)))
  const yMax = $derived(view === 'daily' ? dailyMax : runningMax)
  const y = (v: number) => PAD.top + plotH - (Math.min(v, yMax) / yMax) * plotH
  const ticks = $derived([0, yMax / 2, yMax])

  // Running view: lines.
  const spentPath = $derived(
    cumulative
      .slice(0, running ? elapsed : days)
      .map((v, i) => `${i ? 'L' : 'M'}${x(i) + band / 2},${y(v)}`)
      .join(''),
  )
  const planPath = $derived(spendable !== null ? `M${x(0)},${y(0)}L${x(days)},${y(spendable)}` : '')

  /** Mondays, for x-axis labels. */
  const mondays = $derived(
    Array.from({ length: days }, (_, i) => i).filter((i) => new Date((startDay + i) * DAY).getUTCDay() === 1),
  )

  function pick(i: number) {
    selected = selected === i ? null : i
  }

  function onKey(e: KeyboardEvent, i: number) {
    if (e.key === 'Enter' || e.key === ' ') {
      e.preventDefault()
      pick(i)
    } else if (e.key === 'ArrowRight' || e.key === 'ArrowLeft') {
      e.preventDefault()
      const next = Math.max(0, Math.min(days - 1, i + (e.key === 'ArrowRight' ? 1 : -1)))
      ;(e.currentTarget as SVGElement).parentElement?.querySelectorAll<SVGElement>('.hit')[next]?.focus()
    }
  }

  const shown = $derived(hover ?? selected)
  const tipLeft = $derived(shown === null ? 0 : Math.min(Math.max(x(shown) + band / 2, 90), width - 90))
  const future = (i: number) => running && i >= elapsed
</script>

<div class="head row">
  <h3>Day by day</h3>
  <span class="spacer"></span>
  <div class="toggle" role="group" aria-label="Chart view">
    <button class:active={view === 'daily'} onclick={() => (view = 'daily')}>Daily</button>
    <button class:active={view === 'running'} onclick={() => (view = 'running')}>Running total</button>
  </div>
</div>

<div class="chart" bind:clientWidth={width}>
  {#if width > 0}
  <svg {width} height={H} role="img" aria-label={view === 'daily' ? 'Spending per day' : 'Spending so far against the plan'}>
    <!-- Grid and y labels -->
    {#each ticks as t}
      <line class="grid" x1={PAD.left} x2={width - PAD.right} y1={y(t)} y2={y(t)} />
      <text class="axis" x={PAD.left - 6} y={y(t) + 4} text-anchor="end">{birr(t, true)}</text>
    {/each}

    {#if view === 'daily'}
      {#each totals as v, i}
        {@const clipped = v > yMax}
        <rect
          class="bar"
          class:future={future(i)}
          class:lit={shown === i}
          class:faded={shown !== null && shown !== i}
          x={x(i) + (band - barW) / 2}
          y={y(v)}
          width={barW}
          height={Math.max(0, PAD.top + plotH - y(v))}
          rx={Math.min(3, barW / 2)}
        />
        {#if clipped}
          <!-- A break across the top: this bar continues off the chart. -->
          <line class="break" x1={x(i)} x2={x(i) + band} y1={PAD.top + 6} y2={PAD.top + 2} />
          <text class="clip-label" x={x(i) + band / 2} y={PAD.top - 5} text-anchor="middle">{birr(v, true)}</text>
        {/if}
      {/each}
      {#if allowance !== null}
        <line class="ref" x1={PAD.left} x2={width - PAD.right} y1={y(allowance)} y2={y(allowance)} />
        <text class="ref-label" x={width - PAD.right} y={y(allowance) - 5} text-anchor="end">daily budget {birr(allowance, true)}</text>
      {/if}
    {:else}
      {#if planPath}
        <path class="plan" d={planPath} />
        <text class="ref-label" x={width - PAD.right} y={y(spendable ?? 0) + 14} text-anchor="end">plan {birr(spendable ?? 0, true)}</text>
      {/if}
      <path class="spent" d={spentPath} />
      {#if shown !== null && !future(shown)}
        <line class="crosshair" x1={x(shown) + band / 2} x2={x(shown) + band / 2} y1={PAD.top} y2={PAD.top + plotH} />
        <circle class="dot" cx={x(shown) + band / 2} cy={y(cumulative[shown])} r="4.5" />
      {/if}
    {/if}

    <!-- X labels: the first day and each Monday -->
    <text class="axis" x={x(0)} y={H - 6}>{dayLabel(dayIso(0)).replace(/^\w+ /, '')}</text>
    {#each mondays.filter((i) => x(i) - x(0) > 60) as i}
      <line class="tick" x1={x(i)} x2={x(i)} y1={PAD.top + plotH} y2={PAD.top + plotH + 4} />
      <text class="axis" x={x(i)} y={H - 6} text-anchor="middle">{dayLabel(dayIso(i)).replace(/^\w+ /, '')}</text>
    {/each}

    <!-- Hit areas: a full-height column per day, bigger than any bar. -->
    {#each totals as v, i}
      <rect
        class="hit"
        x={x(i)}
        y={PAD.top}
        width={band}
        height={plotH}
        tabindex={future(i) ? -1 : 0}
        role="button"
        aria-label="{dayLabel(dayIso(i))}: ETB {birr(v)} spent"
        aria-pressed={selected === i}
        onpointerenter={() => (hover = i)}
        onpointerleave={() => (hover = null)}
        onfocus={() => (hover = i)}
        onblur={() => (hover = null)}
        onclick={() => !future(i) && pick(i)}
        onkeydown={(e) => onKey(e, i)}
      />
    {/each}
  </svg>
  {/if}

  {#if hover !== null && !future(hover)}
    {@const i = hover}
    <div class="tip" style:left="{tipLeft}px" role="tooltip">
      {#if view === 'daily'}
        <strong>ETB {birr(totals[i])}</strong>
        <span class="muted">{dayLabel(dayIso(i))} · {byDay[i].length} payment{byDay[i].length === 1 ? '' : 's'}</span>
        {#if allowance !== null && totals[i] > allowance}<span class="warn">▲ {birr(totals[i] - allowance, true)} over the daily budget</span>{/if}
        {#each byDay[i].slice(0, 3) as t (t.id)}
          <span class="line"><span class="name">{title(t)}</span><span>{birr(cost(t), true)}</span></span>
        {/each}
        {#if byDay[i].length > 3}<span class="muted">+{byDay[i].length - 3} more · tap to see all</span>{/if}
      {:else}
        <strong>ETB {birr(cumulative[i])}</strong>
        <span class="muted">spent by {dayLabel(dayIso(i))}</span>
        {#if spendable !== null}
          {@const plan = (spendable * (i + 1)) / days}
          <span class={cumulative[i] > plan ? 'warn' : 'good'}>
            {cumulative[i] > plan ? '▲' : '▼'} {birr(Math.abs(cumulative[i] - plan), true)} {cumulative[i] > plan ? 'ahead of' : 'under'} plan ({birr(plan, true)})
          </span>
        {/if}
      {/if}
    </div>
  {/if}
</div>

{#if selected !== null}
  {@const i = selected}
  <div class="day">
    <div class="row">
      <strong>{dayLabel(dayIso(i))}</strong>
      <span class="muted small">ETB {birr(totals[i])} · {byDay[i].length} payment{byDay[i].length === 1 ? '' : 's'}</span>
      <span class="spacer"></span>
      <button class="ghost" onclick={() => (selected = null)} aria-label="Close day">✕</button>
    </div>
    {#if byDay[i].length === 0}
      <p class="muted small">Nothing spent this day.</p>
    {:else}
      {#each byDay[i] as t (t.id)}<TxnRow {t} />{/each}
    {/if}
  </div>
{:else}
  <p class="hint muted small">Tap a day to see its payments.</p>
{/if}

<style>
  .head {
    margin-bottom: 0.4rem;
  }
  .toggle {
    display: inline-flex;
    border: 1px solid var(--border);
    border-radius: 6px;
    overflow: hidden;
  }
  .toggle button {
    border: none;
    border-radius: 0;
    font-size: 0.8rem;
    padding: 0.2rem 0.6rem;
    background: transparent;
  }
  .toggle button.active {
    background: var(--surface-2);
    font-weight: 600;
  }
  .chart {
    position: relative;
    width: 100%;
    min-width: 0;
    min-height: 170px;
  }
  svg {
    display: block;
    overflow: visible;
  }
  .grid {
    stroke: var(--surface-2);
    stroke-width: 1;
  }
  .axis {
    fill: var(--muted);
    font-size: 11px;
  }
  .tick {
    stroke: var(--border);
  }
  .bar {
    fill: var(--accent);
    transition: opacity 0.12s;
  }
  .bar.future {
    fill: var(--surface-2);
  }
  .bar.faded {
    opacity: 0.45;
  }
  .bar.lit {
    opacity: 1;
    filter: brightness(1.12);
  }
  .break {
    stroke: var(--surface);
    stroke-width: 3;
  }
  .clip-label {
    fill: var(--text);
    font-size: 11px;
    font-weight: 600;
  }
  .ref {
    stroke: var(--warn);
    stroke-width: 1.5;
    stroke-dasharray: 5 4;
  }
  .ref-label {
    fill: var(--warn);
    font-size: 11px;
  }
  .plan {
    stroke: var(--muted);
    stroke-width: 1.5;
    stroke-dasharray: 5 4;
    fill: none;
  }
  .spent {
    stroke: var(--accent);
    stroke-width: 2;
    fill: none;
    stroke-linejoin: round;
  }
  .crosshair {
    stroke: var(--text);
    stroke-width: 1;
    opacity: 0.35;
  }
  .dot {
    fill: var(--accent);
    stroke: var(--surface);
    stroke-width: 2;
  }
  .hit {
    fill: transparent;
    cursor: pointer;
    outline: none;
  }
  .hit:focus-visible {
    fill: var(--accent-bg);
    fill-opacity: 0.6;
  }
  .tip {
    position: absolute;
    top: 0;
    transform: translate(-50%, -100%);
    margin-top: -4px;
    z-index: 2;
    display: flex;
    flex-direction: column;
    gap: 0.1rem;
    min-width: 11rem;
    max-width: 16rem;
    padding: 0.5rem 0.65rem;
    border-radius: 8px;
    background: var(--surface);
    border: 1px solid var(--border);
    box-shadow: 0 6px 20px rgb(0 0 0 / 0.15);
    font-size: 0.8rem;
    pointer-events: none;
  }
  .tip strong {
    font-size: 1rem;
  }
  .tip .line {
    display: flex;
    gap: 0.75rem;
    justify-content: space-between;
  }
  .tip .name {
    overflow: hidden;
    text-overflow: ellipsis;
    white-space: nowrap;
  }
  .warn {
    color: var(--warn);
  }
  .good {
    color: var(--in);
  }
  .day {
    margin-top: 0.6rem;
    padding-top: 0.5rem;
    border-top: 1px solid var(--surface-2);
  }
  .day p {
    margin: 0.3rem 0;
  }
  .hint {
    margin: 0.3rem 0 0;
    text-align: center;
  }
  h3 {
    margin: 0;
  }
</style>
