<script lang="ts">
  let { size = 18 }: { size?: number } = $props();
</script>

<svg width={size} height={size} viewBox="0 0 32 32" aria-hidden="true">
  <defs>
    <radialGradient id="nebula-core" cx="40%" cy="38%" r="65%">
      <stop offset="0%" stop-color="#f3d9ff" />
      <stop offset="45%" stop-color="#9b7bff" />
      <stop offset="100%" stop-color="#3b2a8f" />
    </radialGradient>
    <linearGradient id="nebula-ring" x1="0" y1="0" x2="1" y2="1">
      <stop offset="0%" stop-color="#ff9ad5" />
      <stop offset="100%" stop-color="#6ad1ff" />
    </linearGradient>
  </defs>
  <circle cx="16" cy="16" r="9" fill="url(#nebula-core)" />
  <ellipse
    cx="16"
    cy="16"
    rx="14.5"
    ry="5.2"
    fill="none"
    stroke="url(#nebula-ring)"
    stroke-width="2"
    transform="rotate(-24 16 16)"
  />
</svg>
