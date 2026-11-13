<script lang="ts">
  // Line icons for the phone tab bar, drawn in the text colour.
  let { name }: { name: 'home' | 'review' | 'activity' | 'plans' | 'settings' } = $props()

  const paths: Record<typeof name, string> = {
    home: 'M4 11 12 4l8 7M6 9.5V20h12V9.5',
    review: 'M9.2 9a3 3 0 1 1 4.3 2.7c-.9.5-1.5 1.2-1.5 2.3M12 17.5v.01M12 21a9 9 0 1 0 0-18 9 9 0 0 0 0 18Z',
    activity: 'M8 6h12M8 12h12M8 18h12M4 6h.01M4 12h.01M4 18h.01',
    plans: 'M12 21a9 9 0 1 0 0-18 9 9 0 0 0 0 18ZM12 16a4 4 0 1 0 0-8 4 4 0 0 0 0 8ZM12 12h.01',
    settings: 'M4 7h10M18 7h2M4 17h4M12 17h8M16 5v4M10 15v4',
  }
</script>

<svg viewBox="0 0 24 24" width="22" height="22" fill="none" stroke="currentColor" stroke-width="1.8" stroke-linecap="round" stroke-linejoin="round" aria-hidden="true">
  <path d={paths[name]} />
</svg>
