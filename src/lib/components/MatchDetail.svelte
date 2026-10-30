<script lang="ts">
  import type { ChampionInfo, MatchDetail } from "$lib/types";
  import {
    championIcon,
    formatCompact,
    formatDuration,
    itemIcon,
    queueName,
    roleName,
    timeAgo,
  } from "$lib/format";

  let {
    detail,
    champions,
    items,
    version,
    onclose,
  }: {
    detail: MatchDetail;
    champions: Record<number, ChampionInfo>;
    items: Record<number, string>;
    version: string;
    onclose: () => void;
  } = $props();

  const myTeam = $derived(
    detail.teams.find((team) => team.players.some((player) => player.isMe)) ??
      detail.teams[0],
  );
</script>

<section class="panel detail">
  <div class="panel-head">
    <span>Maç detayı</span>
    <button class="btn small ghost" type="button" onclick={onclose}>Kapat</button>
  </div>

  <div class="detail-meta">
    <span class:ok={myTeam?.win === true} class:warn={myTeam ? !myTeam.win : false}>
      {myTeam?.win ? "Galibiyet" : "Mağlubiyet"}
    </span>
    <span>{detail.queueName ?? queueName(detail.queueId)}</span>
    <span>{formatDuration(detail.durationSecs)}</span>
    <span class="dim">{timeAgo(detail.gameCreation)}</span>
  </div>

  {#each detail.teams as team (team.teamId)}
    <div class="team-block">
      <div class="team-title">
        <span>{team.teamId === 100 ? "Mavi takım" : "Kırmızı takım"}</span>
        <span class:ok={team.win} class:warn={!team.win}>
          {team.win ? "Galibiyet" : "Mağlubiyet"}
        </span>
        <span class="dim">{team.kills} kill</span>
      </div>

      <div class="sb-head" aria-hidden="true">
        <span>Rol</span>
        <span>Oyuncu</span>
        <span>KDA</span>
        <span>CS</span>
        <span>Altın</span>
        <span>Hasar</span>
        <span>Görüş</span>
        <span>Item</span>
      </div>

      {#each team.players as player (player.puuid ?? `${player.teamId}-${player.displayName}`)}
        <div class="sb-row" class:me={player.isMe}>
          <span class="pos">{roleName(player.position)}</span>

          <span class="champ">
            {#if champions[player.championId]}
              <img
                src={championIcon(champions[player.championId].id, version)}
                alt=""
                loading="lazy"
                onerror={(event) => ((event.currentTarget as HTMLImageElement).style.visibility = "hidden")}
              />
            {/if}
            <span class="pname" title={player.displayName}>{player.displayName}</span>
          </span>

          <span class="num">{player.kills}/{player.deaths}/{player.assists}</span>
          <span class="num">{player.cs}</span>
          <span class="num">{formatCompact(player.gold)}</span>
          <span class="num">{formatCompact(player.damage)}</span>
          <span class="num">{player.vision}</span>

          <span class="items">
            {#each player.items as itemId (itemId)}
              <img
                class="item"
                src={itemIcon(itemId, version)}
                alt={items[itemId] ?? `Item ${itemId}`}
                title={items[itemId] ?? `Item ${itemId}`}
                loading="lazy"
                onerror={(event) => ((event.currentTarget as HTMLImageElement).style.visibility = "hidden")}
              />
            {/each}
          </span>
        </div>
      {/each}
    </div>
  {/each}
</section>

<style>
  .detail-meta {
    display: flex;
    flex-wrap: wrap;
    gap: 14px;
    padding: 8px 12px;
    border-bottom: 1px solid var(--border);
    color: var(--muted);
    font-size: 12px;
  }

  .ok {
    color: var(--green);
  }

  .warn {
    color: var(--red);
  }

  .dim {
    color: var(--muted-2);
  }

  .team-block {
    border-bottom: 1px solid var(--border);
  }

  .team-block:last-child {
    border-bottom: 0;
  }

  .team-title {
    display: flex;
    align-items: center;
    gap: 10px;
    padding: 6px 12px;
    background: #222222;
    color: var(--muted);
    font-size: 11px;
    font-weight: 600;
    letter-spacing: 0.04em;
    text-transform: uppercase;
  }

  .sb-head,
  .sb-row {
    display: grid;
    grid-template-columns: 44px minmax(0, 1fr) 64px 46px 58px 62px 40px 132px;
    align-items: center;
    gap: 8px;
    padding: 0 12px;
  }

  .sb-head {
    height: 24px;
    border-bottom: 1px solid var(--border);
    color: var(--muted-2);
    font-size: 10px;
    letter-spacing: 0.04em;
    text-transform: uppercase;
  }

  .sb-head span:not(:first-child):not(:nth-child(2)),
  .num,
  .items {
    text-align: right;
  }

  .sb-row {
    height: 34px;
    border-bottom: 1px solid var(--border);
    font-size: 12px;
  }

  .sb-row:last-child {
    border-bottom: 0;
  }

  .sb-row.me {
    background: var(--accent-soft);
  }

  .pos {
    color: var(--muted);
    font-size: 11px;
  }

  .champ {
    display: flex;
    align-items: center;
    gap: 8px;
    min-width: 0;
  }

  .champ img {
    width: 24px;
    height: 24px;
    flex: 0 0 auto;
    border: 1px solid var(--border);
    border-radius: 2px;
    background: #1c1c1c;
  }

  .pname {
    overflow: hidden;
    text-overflow: ellipsis;
    white-space: nowrap;
  }

  .num {
    font-variant-numeric: tabular-nums;
    white-space: nowrap;
  }

  .items {
    display: flex;
    justify-content: flex-end;
    gap: 2px;
  }

  .item {
    width: 18px;
    height: 18px;
    border: 1px solid var(--border);
    border-radius: 2px;
    background: #1c1c1c;
  }

  @media (max-width: 780px) {
    .sb-head,
    .sb-row {
      grid-template-columns: 40px minmax(0, 1fr) 58px 44px 58px 122px;
    }

    .sb-head span:nth-child(6),
    .sb-head span:nth-child(7),
    .sb-row > :nth-child(6),
    .sb-row > :nth-child(7) {
      display: none;
    }
  }
</style>
