<script lang="ts">
  import type { ChampionInfo, MatchSummary } from "$lib/types";
  import { championIcon, formatDuration, queueName, roleName, timeAgo } from "$lib/format";

  let {
    match,
    champions,
    version,
  }: {
    match: MatchSummary;
    champions: Record<number, ChampionInfo>;
    version: string;
  } = $props();

  const champion = $derived(champions[match.championId] ?? null);
  // Match-v5 bazı şampiyonlarda Data Dragon anahtarından farklı isim döndürür.
  const iconKey = $derived(champion?.id ?? match.championName);
  let iconFailed = $state(false);

  const kda = $derived(
    ((match.kills + match.assists) / Math.max(1, match.deaths)).toFixed(2),
  );
</script>

<div class="match">
  <span class="result" class:win={match.win}>
    {match.win ? "Galibiyet" : "Mağlubiyet"}
  </span>

  <div class="champion">
    {#if !iconFailed && iconKey}
      <img
        src={championIcon(iconKey, version)}
        alt={champion?.name ?? match.championName}
        loading="lazy"
        onerror={() => (iconFailed = true)}
      />
    {/if}
    <span class="name">
      {champion?.name ?? (match.championName || "Bilinmeyen şampiyon")}
    </span>
  </div>

  <span class="dim">{match.queueName ?? queueName(match.queueId)} · {roleName(match.role)}</span>

  <span class="num">
    {match.kills}/{match.deaths}/{match.assists}
    <small>{kda} KDA</small>
  </span>

  <span class="num">{match.cs}</span>

  <span class="num">
    {formatDuration(match.durationSecs)}
    <small>{timeAgo(match.gameCreation)}</small>
  </span>
</div>

<style>
  .match {
    padding: 6px 12px;
    border-bottom: 1px solid var(--border);
  }

  .match:last-child {
    border-bottom: 0;
  }

  .match:hover {
    background: #282828;
  }

  .result {
    font-size: 12px;
    font-weight: 500;
    color: var(--red);
  }

  .result.win {
    color: var(--green);
  }

  .champion {
    display: flex;
    align-items: center;
    gap: 8px;
    min-width: 0;
  }

  .champion img {
    width: 28px;
    height: 28px;
    flex: 0 0 auto;
    border: 1px solid var(--border);
    border-radius: 3px;
    background: #1c1c1c;
  }

  .name {
    font-size: 12px;
    white-space: nowrap;
    overflow: hidden;
    text-overflow: ellipsis;
  }

  .dim {
    color: var(--muted);
    font-size: 11px;
    white-space: nowrap;
    overflow: hidden;
    text-overflow: ellipsis;
  }

  .num {
    display: flex;
    flex-direction: column;
    align-items: flex-end;
    font-size: 12px;
    font-variant-numeric: tabular-nums;
    white-space: nowrap;
  }

  small {
    color: var(--muted);
    font-size: 10px;
  }
</style>
