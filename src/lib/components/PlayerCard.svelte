<script lang="ts">
  import type { ChampionInfo, PlayerCard } from "$lib/types";
  import { championIcon, roleName } from "$lib/format";

  let {
    player,
    champions,
    version,
  }: {
    player: PlayerCard;
    champions: Record<number, ChampionInfo>;
    version: string;
  } = $props();

  const champion = $derived(champions[player.championId] ?? null);
  const icon = $derived(champion ? championIcon(champion.id, version) : null);
  let iconFailed = $state(false);
  const winrate = $derived(
    player.recentWinrate == null ? null : Math.round(player.recentWinrate),
  );
</script>

<div class="player" class:hidden={player.hidden}>
  <div class="portrait">
    {#if icon && !iconFailed}
      <img
        src={icon}
        alt={champion?.name ?? "Şampiyon"}
        loading="lazy"
        onerror={() => (iconFailed = true)}
      />
    {:else}
      <span>—</span>
    {/if}
  </div>

  <div class="identity">
    <span class="name" title={player.hidden ? "Gizli oyuncu" : player.displayName}>
      {player.hidden ? "Gizli oyuncu" : player.displayName}
    </span>
    <span class="meta">
      {roleName(player.assignedPosition)}
      {#if !player.hidden && player.rank}
        · {player.rank}
      {/if}
    </span>
  </div>

  <div class="record">
    {#if player.hidden}
      <span>Bilgi yok</span>
    {:else}
      {#if player.wins != null && player.losses != null}
        <span>{player.wins}G {player.losses}M</span>
      {/if}
      {#if winrate != null}
        <span>Son {player.recentGames}: %{winrate}</span>
      {/if}
    {/if}
  </div>
</div>

<style>
  .player {
    display: flex;
    align-items: center;
    gap: 10px;
    padding: 8px 12px;
    border-bottom: 1px solid var(--border);
  }

  .player:last-child {
    border-bottom: 0;
  }

  .player:hover {
    background: #282828;
  }

  .player.hidden {
    opacity: 0.6;
  }

  .portrait {
    display: grid;
    place-items: center;
    width: 32px;
    height: 32px;
    flex: 0 0 auto;
    overflow: hidden;
    border: 1px solid var(--border);
    border-radius: 3px;
    background: #1c1c1c;
  }

  .portrait img {
    display: block;
    width: 100%;
    height: 100%;
    object-fit: cover;
  }

  .portrait span {
    color: var(--muted-2);
    font-size: 12px;
  }

  .identity {
    display: flex;
    flex-direction: column;
    flex: 1;
    min-width: 0;
  }

  .name {
    font-size: 12px;
    font-weight: 500;
    white-space: nowrap;
    overflow: hidden;
    text-overflow: ellipsis;
  }

  .meta {
    color: var(--muted);
    font-size: 11px;
    white-space: nowrap;
    overflow: hidden;
    text-overflow: ellipsis;
  }

  .record {
    display: flex;
    flex-direction: column;
    align-items: flex-end;
    color: var(--muted);
    font-size: 11px;
    white-space: nowrap;
  }
</style>
