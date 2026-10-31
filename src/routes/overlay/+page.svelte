<script lang="ts">
  import { onMount } from "svelte";
  import { listen } from "@tauri-apps/api/event";

  import {
    getChampSelect,
    getLiveGame,
    getOpggMeta,
    getPersonalBuild,
    getSettings,
    getStatus,
  } from "$lib/api";
  import { counterPlan } from "$lib/counter";
  import { loadDdragon } from "$lib/ddragon";
  import { championIcon, itemIcon, metaTierShort, opggLane, roleName } from "$lib/format";
  import type {
    ChampSelectView,
    ChampionInfo,
    LiveGameView,
    PersonalBuild,
    PlayerCard,
  } from "$lib/types";

  let champ = $state<ChampSelectView | null>(null);
  let liveGame = $state<LiveGameView | null>(null);
  let liveBuild = $state<PersonalBuild | null>(null);
  let phase = $state<string | null>(null);
  let liveError = $state("");
  let champions = $state<Record<number, ChampionInfo>>({});
  let items = $state<Record<number, string>>({});
  let version = $state("");
  let metaOn = $state(false);
  let loaded = $state(false);
  let fetchedAt = $state(0);
  let tick = $state(0);
  let tiers = $state<Record<string, number | null>>({});
  let generation = 0;
  let draftLoadingGeneration: number | null = null;
  let liveLoadingGeneration: number | null = null;

  function tierKey(enemy: PlayerCard): string {
    return `${enemy.championId}:${enemy.assignedPosition ?? ""}`;
  }

  function planOf(enemy: PlayerCard) {
    return counterPlan(champions[enemy.championId]);
  }

  async function loadTiers(view: ChampSelectView) {
    if (!metaOn) return;

    for (const enemy of view.enemies) {
      if (enemy.hidden || !enemy.championName || !enemy.assignedPosition) continue;

      const lane = opggLane(enemy.assignedPosition);
      if (!lane) continue;

      const key = tierKey(enemy);
      if (key in tiers) continue;

      try {
        const report = await getOpggMeta(enemy.championName, lane);
        tiers = { ...tiers, [key]: report.tier };
      } catch {
        tiers = { ...tiers, [key]: null };
      }
    }
  }

  async function refreshDraft(request: number) {
    if (draftLoadingGeneration === request || phase !== "ChampSelect") return;
    draftLoadingGeneration = request;
    try {
      const next = await getChampSelect();
      if (request !== generation || phase !== "ChampSelect") return;
      champ = next;
      fetchedAt = Date.now();
      if (next) void loadTiers(next);
    } catch {
      if (request !== generation) return;
      champ = null;
    } finally {
      if (draftLoadingGeneration === request) draftLoadingGeneration = null;
      if (request === generation) loaded = true;
    }
  }

  async function refreshLive(request: number) {
    if (
      liveLoadingGeneration === request ||
      !["GameStart", "InProgress", "Reconnect"].includes(phase ?? "")
    ) return;
    liveLoadingGeneration = request;
    liveError = "";

    try {
      const next = await getLiveGame();
      if (request !== generation) return;
      if (next) {
        liveGame = next;
        const player = next.allies.find((ally) => ally.isMe);
        if (player && player.championId > 0) {
          try {
            const build = await getPersonalBuild(player.championId);
            if (request === generation) liveBuild = build;
          } catch {
            if (request === generation) liveBuild = null;
          }
        }
      } else if (!liveGame) {
        // Oyun yeni başladıysa LCU verisi birkaç saniye gecikebilir.
        liveError = "Canlı maç verisi henüz hazır değil.";
      }
    } catch {
      if (request !== generation) return;
      if (!liveGame) liveError = "Canlı maç verisi alınamadı.";
    } finally {
      if (liveLoadingGeneration === request) liveLoadingGeneration = null;
      if (request === generation) loaded = true;
    }
  }

  async function showPhase(nextPhase: string | null) {
    generation += 1;
    const request = generation;
    phase = nextPhase;
    loaded = false;
    champ = null;
    liveGame = null;
    liveBuild = null;
    liveError = "";
    tiers = {};

    if (nextPhase === "ChampSelect") {
      await refreshDraft(request);
    } else if (["GameStart", "InProgress", "Reconnect"].includes(nextPhase ?? "")) {
      await refreshLive(request);
    } else {
      loaded = true;
    }
  }

  async function refresh() {
    try {
      const status = await getStatus();
      await showPhase(status.connected ? status.phase : null);
    } catch {
      await showPhase(null);
    }
  }

  async function refreshSettings() {
    try {
      const settings = await getSettings();
      metaOn = settings.experimentalMeta;
    } catch {
      metaOn = false;
    }
  }

  onMount(() => {
    loadDdragon().then((data) => {
      if (!data) return;
      champions = data.champions;
      items = data.items;
      version = data.version;
    });

    refreshSettings();
    refresh();

    const timer = setInterval(() => {
      if (phase === "ChampSelect") void refreshDraft(generation);
    }, 4_000);
    const liveTimer = setInterval(() => {
      if (["GameStart", "InProgress", "Reconnect"].includes(phase ?? "")) {
        void refreshLive(generation);
      }
    }, 15_000);
    const clock = setInterval(() => tick++, 1_000);

    // Kaplama açıldığında beklemeden yenile.
    const unlistenPhase = listen<string | null>("tlas:game-phase", (event) => {
      void showPhase(event.payload);
    }).catch(() => () => {});
    const unlisten = listen("tlas:overlay-shown", () => {
      refreshSettings();
      refresh();
    }).catch(() => () => {});

    return () => {
      clearInterval(timer);
      clearInterval(liveTimer);
      clearInterval(clock);
      unlistenPhase.then((fn) => fn());
      unlisten.then((fn) => fn());
    };
  });

  const timerSeconds = $derived.by(() => {
    void tick;
    if (champ?.timer == null) return null;
    const elapsed = fetchedAt ? Date.now() - fetchedAt : 0;
    return Math.max(0, Math.ceil((champ.timer - elapsed) / 1_000));
  });
  const inDraft = $derived(phase === "ChampSelect");
  const inLiveGame = $derived(["GameStart", "InProgress", "Reconnect"].includes(phase ?? ""));
  const myLivePlayer = $derived(liveGame?.allies.find((player) => player.isMe) ?? null);
  const myLiveChampion = $derived(
    myLivePlayer ? (champions[myLivePlayer.championId] ?? null) : null,
  );
</script>

<svelte:head>
  <title>Tlas kaplama</title>
</svelte:head>

<div class="overlay">
  <header class="head">
    <span class="title">{inDraft ? "Şampiyon Seçimi" : inLiveGame ? "Canlı maç" : "Tlas"}</span>
    {#if inDraft && timerSeconds != null}
      <span class="timer">{timerSeconds}s</span>
    {/if}
  </header>

  <div class="list">
    {#if !loaded}
      <p class="empty">Yükleniyor…</p>
    {:else if inDraft}
      {#if !champ || champ.enemies.length === 0}
        <p class="empty">Rakip şampiyonlar bekleniyor…</p>
      {:else}
        {#each champ.enemies as enemy, index (index)}
          {@const plan = planOf(enemy)}
          <div class="enemy">
            <div class="line">
              <span class="icon">
                {#if enemy.hidden}
                  <span class="unknown">?</span>
                {:else if champions[enemy.championId]}
                  <img
                    src={championIcon(champions[enemy.championId].id, version || "16.21.1")}
                    alt=""
                    loading="lazy"
                    onerror={(event) => ((event.currentTarget as HTMLImageElement).style.visibility = "hidden")}
                  />
                {/if}
              </span>

              <span class="who">
                <span class="name">
                  {enemy.hidden
                    ? "Gizli"
                    : (enemy.championName ?? `Şampiyon ${enemy.championId}`)}
                </span>
                <span class="sub">
                  {enemy.hidden ? "—" : roleName(enemy.assignedPosition)}
                </span>
              </span>

              <span
                class="tier"
                class:low={!enemy.hidden && (tiers[tierKey(enemy)] ?? 9) <= 2}
                class:high={!enemy.hidden && (tiers[tierKey(enemy)] ?? 0) >= 4}
              >
                {enemy.hidden ? "—" : metaTierShort(tiers[tierKey(enemy)])}
              </span>
            </div>

            {#if !enemy.hidden}
              <div class="plan">
                <span class="rune">{plan.rune}</span>
                <span class="items">
                  {#each plan.items as itemId (itemId)}
                    <span class="item">
                      <img
                        src={itemIcon(itemId, version || "16.21.1")}
                        alt=""
                        loading="lazy"
                        onerror={(event) => ((event.currentTarget as HTMLImageElement).style.visibility = "hidden")}
                      />
                    </span>
                  {/each}
                </span>
                <span class="note">{plan.note}</span>
              </div>
            {/if}
          </div>
        {/each}
      {/if}
    {:else if inLiveGame}
      {#if liveError}
        <p class="empty">{liveError}</p>
      {:else if !liveGame || !myLivePlayer}
        <p class="empty">Canlı maç bilgisi bekleniyor…</p>
      {:else}
        <section class="live-summary">
          <div class="live-player">
            <span class="icon">
              {#if myLiveChampion}
                <img
                  src={championIcon(myLiveChampion.id, version || "16.21.1")}
                  alt=""
                  onerror={(event) => ((event.currentTarget as HTMLImageElement).style.visibility = "hidden")}
                />
              {/if}
            </span>
            <span class="who">
              <span class="name">{myLiveChampion?.name ?? `Şampiyon ${myLivePlayer.championId}`}</span>
              <span class="sub">{roleName(myLivePlayer.position)} · {liveGame.queueName ?? "Canlı maç"}</span>
            </span>
          </div>

          <div class="live-build">
            <div class="build-heading">
              <strong>Kişisel build</strong>
              <span>{liveBuild?.games ? `${liveBuild.games} maç` : "yerel geçmiş"}</span>
            </div>
            {#if liveBuild && liveBuild.games > 0}
              <div class="build-row">
                <span class="build-label">Çekirdek</span>
                <span class="items">
                  {#each liveBuild.coreItems.slice(0, 4) as item (item.itemId)}
                    <img
                      class="item"
                      src={itemIcon(item.itemId, version || "16.21.1")}
                      alt={items[item.itemId] ?? `Item ${item.itemId}`}
                      title={items[item.itemId] ?? `Item ${item.itemId}`}
                      onerror={(event) => ((event.currentTarget as HTMLImageElement).style.visibility = "hidden")}
                    />
                  {:else}
                    <span class="sub">Kayıt yok</span>
                  {/each}
                </span>
              </div>
              <div class="build-row">
                <span class="build-label">Ayakkabı</span>
                <span class="items">
                  {#each liveBuild.boots.slice(0, 2) as item (item.itemId)}
                    <img
                      class="item"
                      src={itemIcon(item.itemId, version || "16.21.1")}
                      alt={items[item.itemId] ?? `Item ${item.itemId}`}
                      title={items[item.itemId] ?? `Item ${item.itemId}`}
                      onerror={(event) => ((event.currentTarget as HTMLImageElement).style.visibility = "hidden")}
                    />
                  {:else}
                    <span class="sub">Kayıt yok</span>
                  {/each}
                </span>
              </div>
            {:else}
              <p class="sub">Bu şampiyon için yerel maç geçmişi bulunamadı.</p>
            {/if}
          </div>
        </section>
      {/if}
    {:else}
      <p class="empty">LoL istemcisi bağlantısı bekleniyor…</p>
    {/if}
  </div>

  <footer class="foot">
    <span>{inLiveGame ? "Kişisel geçmiş · yerel veri" : "Karşı öneriler tahminidir."}</span>
    {#if inDraft && !metaOn}
      <span class="meta-off">Tier için Ayarlar → deneysel meta.</span>
    {/if}
  </footer>
</div>

<style>
  :global(html),
  :global(body) {
    overflow: hidden;
  }

  .overlay {
    display: flex;
    flex-direction: column;
    height: 100vh;
    background: var(--bg);
    border: 1px solid var(--border);
    user-select: none;
  }

  .head {
    display: flex;
    align-items: center;
    justify-content: space-between;
    gap: 8px;
    flex: 0 0 auto;
    height: 30px;
    padding: 0 10px;
    background: var(--panel-2);
    border-bottom: 1px solid var(--border);
  }

  .title {
    font-size: 11px;
    font-weight: 600;
    letter-spacing: 0.05em;
    text-transform: uppercase;
    color: var(--muted);
  }

  .timer {
    font-size: 11px;
    font-variant-numeric: tabular-nums;
    color: var(--muted);
  }

  .list {
    flex: 1 1 auto;
    min-height: 0;
    overflow-y: auto;
  }

  .list::-webkit-scrollbar {
    width: 8px;
  }

  .empty {
    margin: 0;
    padding: 28px 16px;
    text-align: center;
    color: var(--muted);
    font-size: 11px;
  }

  .enemy {
    padding: 6px 10px;
    border-bottom: 1px solid var(--border);
  }

  .enemy:last-child {
    border-bottom: 0;
  }

  .live-summary {
    padding: 12px;
  }

  .live-player {
    display: flex;
    align-items: center;
    gap: 9px;
    padding-bottom: 12px;
    border-bottom: 1px solid var(--border);
  }

  .live-build {
    margin-top: 12px;
  }

  .build-heading {
    display: flex;
    align-items: center;
    justify-content: space-between;
    color: var(--text);
    font-size: 11px;
  }

  .build-heading span {
    color: var(--muted-2);
    font-size: 10px;
  }

  .live-build .build-row {
    gap: 8px;
    padding: 8px 0;
  }

  .live-build .build-label {
    flex-basis: 66px;
  }

  .line {
    display: flex;
    align-items: center;
    gap: 8px;
  }

  .icon {
    flex: 0 0 auto;
    display: flex;
    align-items: center;
    justify-content: center;
    width: 30px;
    height: 30px;
    overflow: hidden;
    border: 1px solid var(--border);
    border-radius: 2px;
    background: var(--bg-elevated);
  }

  .icon img {
    display: block;
    width: 100%;
    height: 100%;
  }

  .unknown {
    color: var(--muted-2);
    font-size: 13px;
    font-weight: 600;
  }

  .who {
    display: flex;
    flex-direction: column;
    min-width: 0;
    gap: 1px;
  }

  .name {
    font-size: 12px;
    font-weight: 600;
    white-space: nowrap;
    overflow: hidden;
    text-overflow: ellipsis;
  }

  .sub {
    font-size: 10px;
    color: var(--muted);
  }

  .tier {
    margin-left: auto;
    min-width: 26px;
    padding: 2px 5px;
    border: 1px solid var(--border);
    border-radius: 2px;
    text-align: center;
    font-size: 10px;
    font-weight: 600;
    font-variant-numeric: tabular-nums;
    color: var(--muted);
  }

  .tier.low {
    border-color: #3d6449;
    color: var(--green);
  }

  .tier.high {
    border-color: #614141;
    color: var(--red);
  }

  .plan {
    display: flex;
    align-items: center;
    gap: 8px;
    margin-top: 5px;
    padding-left: 38px;
  }

  .rune {
    flex: 0 0 auto;
    max-width: 86px;
    overflow: hidden;
    text-overflow: ellipsis;
    white-space: nowrap;
    font-size: 10px;
    color: var(--text);
  }

  .items {
    display: flex;
    gap: 3px;
  }

  .item {
    display: block;
    width: 20px;
    height: 20px;
    overflow: hidden;
    border: 1px solid var(--border);
    border-radius: 2px;
    background: #1c1c1c;
  }

  .item img {
    display: block;
    width: 100%;
    height: 100%;
  }

  .note {
    margin-left: auto;
    font-size: 10px;
    color: var(--muted-2);
    white-space: nowrap;
  }

  .foot {
    display: flex;
    align-items: center;
    justify-content: space-between;
    gap: 8px;
    flex: 0 0 auto;
    height: 24px;
    padding: 0 10px;
    border-top: 1px solid var(--border);
    background: var(--bg-elevated);
    color: var(--muted-2);
    font-size: 10px;
    white-space: nowrap;
  }

  .meta-off {
    overflow: hidden;
    text-overflow: ellipsis;
  }
</style>
