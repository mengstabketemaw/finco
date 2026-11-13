<script lang="ts">
  import { api, inApp } from './api'
  import SyncCard from './SyncCard.svelte'
  import { SOURCES, SPEND_CATEGORIES, birr, category, toSantim } from './money'
  import { data, reload, toast } from './store.svelte'

  let numbers = $state('')
  // Amount rules as editable rows: [birr text, category id].
  let rules = $state<[string, string][]>([])
  $effect(() => {
    numbers = data.settings.own_numbers.join(', ')
    rules = Object.entries(data.settings.amount_rules)
      .sort(([a], [b]) => Number(a) - Number(b))
      .map(([santim, cat]) => [birr(Number(santim), true), cat])
  })

  function norm(n: string): string {
    const d = n.replace(/\D/g, '')
    if (d.length === 12 && d.startsWith('251')) return '0' + d.slice(3)
    if (d.length === 9) return '0' + d
    return d
  }

  async function save() {
    const own_numbers = [...new Set(numbers.split(/[\s,]+/).filter(Boolean).map(norm))]
    const amount_rules: Record<string, string> = {}
    for (const [text, cat] of rules) {
      const santim = toSantim(text)
      if (santim && cat) amount_rules[String(santim)] = cat
    }
    await api.saveSettings({ ...data.settings, own_numbers, amount_rules })
    await reload()
    toast('Settings saved')
  }

  const sources = $derived([...new Set(data.txns.map((t) => t.source))])
</script>

<h2>Settings</h2>

<div class="stack narrow">
  <section class="card stack">
    <h3>Your phone numbers</h3>
    <p class="small muted">Packages and airtime bought for these numbers are tagged “Other” without asking.</p>
    <input bind:value={numbers} placeholder="09XXXXXXXX, 07XXXXXXXX" />
    <div><button class="primary" onclick={save}>Save</button></div>
  </section>

  <section class="card stack">
    <h3>Tag by amount</h3>
    <p class="small muted">An untagged transfer of exactly this amount gets this category, like fixed taxi fares. Tagging one by hand still wins.</p>
    {#each rules as rule, i}
      <div class="row">
        <span class="small muted">ETB</span>
        <input class="amount" inputmode="decimal" bind:value={rule[0]} aria-label="Amount" />
        <span class="small muted">→</span>
        <select bind:value={rule[1]} aria-label="Category">
          {#each SPEND_CATEGORIES as c (c.id)}<option value={c.id}>{c.emoji} {c.label}</option>{/each}
        </select>
        <button class="ghost" onclick={() => rules.splice(i, 1)} aria-label="Remove {rule[0]} → {category(rule[1]).label}">✕</button>
      </div>
    {/each}
    <div class="row">
      <button onclick={() => rules.push(['', rules.at(-1)?.[1] ?? 'transport'])}>Add amount</button>
      <button class="primary" onclick={save}>Save</button>
    </div>
  </section>

  <section class="card stack">
    <h3>Accounts</h3>
    <p class="small muted">Found in your messages. Transfers between them aren't counted as spending.</p>
    <div class="row">
      {#each sources as s}<span class="chip pill">{SOURCES[s] ?? s}</span>{/each}
    </div>
  </section>

  {#if inApp}
    <SyncCard />
  {:else}
    <section class="card stack">
      <h3>Sync</h3>
      <p class="small muted">Preview mode: {data.txns.length} transactions loaded from <code>dev-data.json</code>. Sync and SMS reading are in the app.</p>
    </section>
  {/if}
</div>

<style>
  h2 {
    margin-bottom: 0.75rem;
  }
  .narrow {
    max-width: 640px;
  }
  p {
    margin: 0;
  }
  .amount {
    width: 6rem;
    text-align: right;
  }
  .pill {
    background: var(--surface-2);
  }
</style>
