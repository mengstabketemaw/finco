<script lang="ts">
  import type { Category } from './money'

  let {
    categories,
    selected = null,
    onpick,
  }: { categories: Category[]; selected?: string | null; onpick: (id: string) => void } = $props()
</script>

<div class="grid">
  {#each categories as c (c.id)}
    <button class:active={selected === c.id} onclick={() => onpick(c.id)}>
      <span class="emoji" aria-hidden="true">{c.emoji}</span>
      <span>{c.label}</span>
    </button>
  {/each}
</div>

<style>
  .grid {
    display: grid;
    grid-template-columns: repeat(auto-fill, minmax(5.6rem, 1fr));
    gap: 0.4rem;
  }
  button {
    display: flex;
    flex-direction: column;
    align-items: center;
    gap: 0.15rem;
    padding: 0.55rem 0.3rem;
    border-radius: var(--radius);
    font-size: 0.85rem;
  }
  .emoji {
    font-size: 1.3rem;
    line-height: 1.2;
  }
  .active {
    background: var(--accent-bg);
    border-color: var(--accent);
    font-weight: 600;
  }
</style>
