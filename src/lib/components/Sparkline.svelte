<script lang="ts">
  let {
    points,
    width = 260,
    height = 46,
  }: {
    points: number[];
    width?: number;
    height?: number;
  } = $props();

  const geometry = $derived.by(() => {
    if (points.length < 2) return null;

    const min = Math.min(...points);
    const max = Math.max(...points);
    const span = Math.max(1, max - min);
    const step = width / (points.length - 1);

    const coords = points.map((value, index) => {
      const x = index * step;
      const y = height - 4 - ((value - min) / span) * (height - 8);
      return `${x.toFixed(1)},${y.toFixed(1)}`;
    });

    return coords.join(" ");
  });
</script>

{#if geometry}
  <svg
    class="spark"
    viewBox={`0 0 ${width} ${height}`}
    preserveAspectRatio="none"
    role="img"
    aria-label="LP eğilimi"
  >
    <polyline
      points={geometry}
      fill="none"
      stroke="var(--accent)"
      stroke-width="1.5"
      stroke-linejoin="round"
      stroke-linecap="round"
    />
  </svg>
{:else}
  <span class="empty-line">—</span>
{/if}

<style>
  .spark {
    display: block;
    width: 100%;
    height: 46px;
  }

  .empty-line {
    display: block;
    height: 46px;
    color: var(--muted-2);
  }
</style>
