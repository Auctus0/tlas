<script lang="ts">
  import type { ChampionInfo, PlayerCard } from "$lib/types";
  import { analyseComp } from "$lib/composition";

  let {
    title,
    players,
    champions,
    note = "",
  }: {
    title: string;
    players: PlayerCard[];
    champions: Record<number, ChampionInfo>;
    note?: string;
  } = $props();

  const profile = $derived(analyseComp(players, champions));
</script>

<section class="panel comp">
  <div class="panel-head">
    <span>{title}</span>
    <span class="head-note">{profile.known} şampiyon</span>
  </div>

  <div class="panel-body">
    {#if profile.known > 0}
      <div class="comp-stats">
        <span class="comp-stat">
          <strong>{profile.ad}</strong>
          <small>Fiziksel</small>
        </span>
        <span class="comp-stat">
          <strong>{profile.ap}</strong>
          <small>Büyü</small>
        </span>
        <span class="comp-stat">
          <strong>{profile.tank}</strong>
          <small>Tank</small>
        </span>
        <span class="comp-stat">
          <strong>{profile.frontline}</strong>
          <small>Ön saf</small>
        </span>
        <span class="comp-stat">
          <strong>{profile.ranged}</strong>
          <small>Menzilli</small>
        </span>
      </div>

      {#if profile.missing.length > 0}
        <p class="comp-missing">{profile.missing.join(" · ")}</p>
      {:else}
        <p class="hint">Denge tamam.</p>
      {/if}
    {:else}
      <p class="hint">Şampiyon bilgisi bekleniyor.</p>
    {/if}

    {#if note}
      <p class="hint">{note}</p>
    {/if}
  </div>
</section>

<style>
  .comp-stats {
    display: grid;
    grid-template-columns: repeat(5, minmax(0, 1fr));
    gap: 6px;
  }

  .comp-stat {
    display: flex;
    flex-direction: column;
    gap: 1px;
    padding: 6px 8px;
    border: 1px solid var(--border);
    border-radius: 3px;
    background: #202020;
  }

  .comp-stat strong {
    font-size: 14px;
    font-variant-numeric: tabular-nums;
  }

  .comp-stat small {
    color: var(--muted);
    font-size: 10px;
    letter-spacing: 0.03em;
    text-transform: uppercase;
  }

  .comp-missing {
    margin: 8px 0 0;
    color: var(--red);
    font-size: 11px;
  }

  @media (max-width: 680px) {
    .comp-stats {
      grid-template-columns: repeat(3, minmax(0, 1fr));
    }
  }
</style>
