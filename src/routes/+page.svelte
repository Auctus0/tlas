<script lang="ts">
  import { onMount } from "svelte";
  import { getVersion } from "@tauri-apps/api/app";
  import { listen } from "@tauri-apps/api/event";
  import { check, type Update } from "@tauri-apps/plugin-updater";
  import { relaunch } from "@tauri-apps/plugin-process";

  import MatchRow from "$lib/components/MatchRow.svelte";
  import MatchDetail from "$lib/components/MatchDetail.svelte";
  import PlayerCard from "$lib/components/PlayerCard.svelte";
  import DraftComp from "$lib/components/DraftComp.svelte";
  import Sparkline from "$lib/components/Sparkline.svelte";
  import {
    clearCache,
    getAccountOverview,
    getChampSelect,
    getLiveGame,
    getLobby,
    getMatchDetail,
    getOpggMeta,
    getPersonalBuild,
    getRecentMatches,
    getSettings,
    getStatus,
    openExternal,
    saveSettings,
  } from "$lib/api";
  import {
    championIcon,
    championOpggName,
    errorText,
    formatCompact,
    itemIcon,
    lpDelta,
    metaTier,
    opggChampionUrl,
    opggLane,
    percent,
    phaseText,
    profileIcon,
    queueName,
    roleName,
    timeAgo,
  } from "$lib/format";
  import type {
    AccountOverview,
    ChampionInfo,
    ChampSelectView,
    LcuStatus,
    LiveGameView,
    LobbyView,
    MatchDetail as MatchDetailData,
    MatchSummary,
    MetaReport,
    PersonalBuild,
    Settings,
  } from "$lib/types";

  type Tab = "home" | "draft" | "matches" | "settings";

  let tab = $state<Tab>("home");
  let status = $state<LcuStatus | null>(null);
  let champ = $state<ChampSelectView | null>(null);
  let matches = $state<MatchSummary[]>([]);
  let overview = $state<AccountOverview | null>(null);
  let lobby = $state<LobbyView | null>(null);
  let liveGame = $state<LiveGameView | null>(null);
  let matchDetail = $state<MatchDetailData | null>(null);
  let selectedMatchId = $state<string | null>(null);
  let build = $state<PersonalBuild | null>(null);
  let buildChampionId = $state<number | null>(null);
  let meta = $state<MetaReport | null>(null);
  let metaPosition = $state("mid");
  let settings = $state<Settings>({
    autoUpdate: true,
    experimentalMeta: false,
  });
  let champions = $state<Record<number, ChampionInfo>>({});
  let items = $state<Record<number, string>>({});
  let ddragonVersion = $state("16.21.1");
  let appVersion = $state(__APP_VERSION__);

  let statusLoading = $state(true);
  let champLoading = $state(false);
  let matchesLoading = $state(false);
  let matchesLoaded = $state(false);
  let matchesError = $state("");
  let homeLoading = $state(false);
  let matchDetailLoading = $state(false);
  let matchDetailError = $state("");
  let buildLoading = $state(false);
  let buildError = $state("");
  let metaLoading = $state(false);
  let metaError = $state("");
  let toast = $state("");
  let updateInfo = $state<{ version: string; body: string | null } | null>(null);
  let pendingUpdate = $state<Update | null>(null);
  let updateStatus = $state("");
  let updateBusy = $state(false);
  let downloaded = $state(0);
  let totalDownload = $state(0);

  let lastDraftFetch = 0;
  let draftFetchedAt = 0;
  let lastHomeFetch = 0;
  let lastLiveFetch = 0;
  let homeRetried = false;
  let matchRequest = 0;
  let buildRequest = 0;
  let metaRequest = 0;
  let tick = $state(0);
  let toastTimer: ReturnType<typeof setTimeout> | undefined;

  const tabs: { id: Tab; label: string }[] = [
    { id: "home", label: "Genel" },
    { id: "draft", label: "Şampiyon Seçimi" },
    { id: "matches", label: "Maçlar" },
    { id: "settings", label: "Ayarlar" },
  ];

  function showToast(message: string) {
    toast = message;
    if (toastTimer) clearTimeout(toastTimer);
    toastTimer = setTimeout(() => (toast = ""), 2_600);
  }

  async function loadSettings() {
    try {
      settings = await getSettings();
    } catch {
      // Varsayılanlarla devam et.
    }
  }

  async function loadChampions() {
    const cached = localStorage.getItem("tlas:champions:v2");

    if (cached) {
      try {
        const parsed = JSON.parse(cached) as {
          at: number;
          version: string;
          map: Record<number, ChampionInfo>;
        };

        if (Date.now() - parsed.at < 7 * 86_400_000) {
          champions = parsed.map;
          ddragonVersion = parsed.version;
          loadItems(parsed.version);
          return;
        }
      } catch {
        // Bozuk cache'i yoksay.
      }
    }

    try {
      const versions = (await fetch(
        "https://ddragon.leagueoflegends.com/api/versions.json",
      ).then((response) => response.json())) as string[];

      const version = versions[0];
      const data = await fetch(
        `https://ddragon.leagueoflegends.com/cdn/${version}/data/en_US/champion.json`,
      ).then((response) => response.json());

      const map: Record<number, ChampionInfo> = {};
      for (const champion of Object.values(data.data) as Array<{
        key: string;
        id: string;
        name: string;
        tags?: string[];
        info?: { attack: number; magic: number; defense: number; difficulty: number };
      }>) {
        map[Number(champion.key)] = {
          id: champion.id,
          name: champion.name,
          tags: champion.tags,
          info: champion.info,
        };
      }

      ddragonVersion = version;
      champions = map;
      localStorage.setItem(
        "tlas:champions:v2",
        JSON.stringify({ at: Date.now(), version, map }),
      );

      loadItems(version);
    } catch {
      // Offline: placeholder ikonlar kalır.
    }
  }

  async function loadItems(version: string) {
    try {
      const data = await fetch(
        `https://ddragon.leagueoflegends.com/cdn/${version}/data/en_US/item.json`,
      ).then((response) => response.json());

      const map: Record<number, string> = {};
      for (const [id, item] of Object.entries(
        data.data as Record<string, { name: string }>,
      )) {
        map[Number(id)] = item.name;
      }

      items = map;
    } catch {
      // Item adları yoksa yalnızca kimlik gösterilir.
    }
  }

  async function refreshStatus() {
    try {
      const next = await getStatus();
      const previousPhase = status?.phase ?? null;
      status = next;

      if (next.phase === "ChampSelect" && previousPhase !== "ChampSelect") {
        refreshChamp(true);
      }

      if (next.phase !== "ChampSelect" && previousPhase === "ChampSelect") {
        champ = null;
      }
    } catch (error) {
      status = {
        connected: false,
        phase: null,
        summonerName: null,
        platform: null,
        error: String(error),
      };
    } finally {
      statusLoading = false;
    }
  }

  async function refreshChamp(force = false) {
    if (champLoading) return;

    const now = Date.now();
    if (!force && now - lastDraftFetch < 4_000) return;
    lastDraftFetch = now;
    champLoading = true;

    try {
      champ = await getChampSelect();
      draftFetchedAt = Date.now();
    } catch {
      champ = null;
    } finally {
      champLoading = false;
    }
  }

  async function refreshHome(force = false) {
    if (!status?.connected) {
      overview = null;
      lobby = null;
      liveGame = null;
      return;
    }

    const now = Date.now();
    if (!force && now - lastHomeFetch < 8_000) return;
    if (homeLoading) return;

    homeLoading = true;

    try {
      const [nextOverview, nextLobby, nextLive] = await Promise.all([
        getAccountOverview().catch(() => null),
        getLobby().catch(() => null),
        getLiveGame().catch(() => null),
      ]);

      overview = nextOverview;
      lobby = nextLobby;
      liveGame = nextLive;
      lastHomeFetch = Date.now();

      // LCU geçmişi geç yükleyebilir; boş geldiyse bir kez daha dene.
      if (nextOverview && nextOverview.recentGames > 0) {
        homeRetried = false;
      } else if (nextOverview && !homeRetried) {
        homeRetried = true;
        setTimeout(() => {
          if (tab === "home" && status?.connected) refreshHome(true);
        }, 6_000);
      }
    } finally {
      homeLoading = false;
    }
  }

  async function loadLive(force = false) {
    const now = Date.now();
    if (!force && now - lastLiveFetch < 12_000) return;
    lastLiveFetch = now;

    try {
      liveGame = await getLiveGame();
    } catch {
      liveGame = null;
    }
  }

  async function openMatch(matchId: string) {
    if (selectedMatchId === matchId) {
      matchRequest += 1;
      selectedMatchId = null;
      matchDetail = null;
      matchDetailError = "";
      matchDetailLoading = false;
      return;
    }

    selectedMatchId = matchId;
    matchDetail = null;
    matchDetailError = "";
    matchDetailLoading = true;

    const request = ++matchRequest;

    try {
      const detail = await getMatchDetail(matchId);
      if (request !== matchRequest) return;
      matchDetail = detail;
    } catch (error) {
      if (request !== matchRequest) return;
      matchDetailError = errorText(error, "Maç detayı alınamadı.");
    } finally {
      if (request === matchRequest) matchDetailLoading = false;
    }
  }

  function closeBuild() {
    buildRequest += 1;
    metaRequest += 1;
    buildChampionId = null;
    build = null;
    buildError = "";
    meta = null;
    metaError = "";
  }

  async function openBuild(championId: number) {
    if (buildChampionId === championId) {
      closeBuild();
      return;
    }

    buildRequest += 1;
    const request = buildRequest;
    buildChampionId = championId;
    build = null;
    buildError = "";
    metaRequest += 1;
    meta = null;
    metaError = "";
    buildLoading = true;

    try {
      const next = await getPersonalBuild(championId);
      if (request !== buildRequest) return;

      build = next;
      const lane = opggLane(next.position);
      if (lane) metaPosition = lane;
    } catch (error) {
      if (request !== buildRequest) return;
      buildError = errorText(error, "Kişisel build alınamadı.");
    } finally {
      if (request === buildRequest) buildLoading = false;
    }
  }

  async function fetchMeta() {
    if (buildChampionId == null) return;

    const info = champions[buildChampionId];
    if (!info) {
      metaError = "Şampiyon bilgisi yüklenmedi.";
      return;
    }

    metaLoading = true;
    metaError = "";
    metaRequest += 1;
    const request = metaRequest;

    try {
      const next = await getOpggMeta(championOpggName(info.name), metaPosition);
      if (request !== metaRequest) return;
      meta = next;
    } catch (error) {
      if (request !== metaRequest) return;
      meta = null;
      metaError = errorText(error, "Meta verisi alınamadı.");
    } finally {
      if (request === metaRequest) metaLoading = false;
    }
  }

  async function openOpgg() {
    if (buildChampionId == null) return;

    const info = champions[buildChampionId];
    if (!info) return;

    try {
      await openExternal(opggChampionUrl(info.name, build?.position));
    } catch (error) {
      showToast(`Bağlantı açılamadı: ${errorText(error, "tarayıcı açılamadı")}`);
    }
  }

  async function loadMatches(force = false) {
    if (matchesLoading) return;
    if (matchesLoaded && !force) return;
    matchesLoading = true;
    matchesError = "";

    try {
      matches = await getRecentMatches(20);
      matchesLoaded = true;
    } catch (error) {
      matchesError = errorText(
        error,
        "LoL istemcisi açık olmalı.",
      );
    } finally {
      matchesLoading = false;
    }
  }

  async function persistSettings() {
    try {
      await saveSettings(settings);
      showToast("Ayarlar kaydedildi");
    } catch (error) {
      showToast(`Kaydedilemedi: ${errorText(error, "ayarlar yazılamadı")}`);
    }
  }

  async function wipeCache() {
    try {
      await clearCache();
      matches = [];
      matchesLoaded = false;
      champ = null;
      selectedMatchId = null;
      matchDetail = null;
      closeBuild();
      showToast("Önbellek temizlendi");
    } catch (error) {
      showToast(`Temizlenemedi: ${errorText(error, "önbellek silinemedi")}`);
    }
  }

  async function checkUpdates(silent = false) {
    if (status?.phase === "InProgress" || status?.phase === "ChampSelect") {
      if (!silent) showToast("Oyun sırasında güncelleme kontrol edilmez.");
      return;
    }

    updateBusy = true;
    if (!silent) updateStatus = "Kontrol ediliyor…";

    try {
      const update = await check();

      if (!update) {
        updateInfo = null;
        pendingUpdate = null;
        if (!silent) updateStatus = "Güncelsin.";
        return;
      }

      pendingUpdate = update;
      updateInfo = { version: update.version, body: update.body ?? null };
      updateStatus = `v${update.version} hazır.`;
    } catch (error) {
      if (!silent) updateStatus = `Kontrol başarısız: ${errorText(error, "sunucuya ulaşılamadı")}`;
    } finally {
      updateBusy = false;
    }
  }

  async function installUpdate() {
    if (status?.phase === "InProgress" || status?.phase === "ChampSelect") {
      showToast("Oyun sırasında güncelleme kurulmaz.");
      return;
    }

    updateBusy = true;
    downloaded = 0;
    totalDownload = 0;

    try {
      const update = pendingUpdate ?? (await check());

      if (!update) {
        updateInfo = null;
        pendingUpdate = null;
        updateStatus = "Güncelsin.";
        return;
      }

      await update.downloadAndInstall((event) => {
        if (event.event === "Started") {
          totalDownload = event.data.contentLength ?? 0;
        } else if (event.event === "Progress") {
          downloaded += event.data.chunkLength;
          const percent =
            totalDownload > 0 ? ` · %${Math.round((downloaded / totalDownload) * 100)}` : "";
          updateStatus = `İndiriliyor… ${(downloaded / 1_000_000).toFixed(1)} MB${percent}`;
        } else if (event.event === "Finished") {
          updateStatus = "Kuruluyor…";
        }
      });

      await relaunch();
    } catch (error) {
      updateStatus = `Kurulum başarısız: ${errorText(error, "indirme tamamlanamadı")}`;
    } finally {
      updateBusy = false;
    }
  }

  const phase = $derived(status?.phase ?? null);
  const avatarInitial = $derived(
    (overview?.summoner.name ?? status?.summonerName ?? "?").trim().charAt(0).toUpperCase() ||
      "?",
  );
  const timerSeconds = $derived.by(() => {
    void tick;
    if (!champ?.timer) return null;
    const elapsed = draftFetchedAt ? Date.now() - draftFetchedAt : 0;
    return Math.max(0, Math.ceil((champ.timer - elapsed) / 1_000));
  });
  const enemiesHidden = $derived(
    (champ?.enemies.length ?? 0) > 0 && champ!.enemies.every((player) => player.hidden),
  );
  const weakChampions = $derived.by(() => {
    if (!overview) return [];

    return overview.champions
      .filter((champion) => champion.games >= 2 && champion.winrate < 45)
      .slice(0, 3)
      .map(
        (champion) =>
          champions[champion.championId]?.name ?? `Şampiyon ${champion.championId}`,
      );
  });
  const weakNote = $derived(
    weakChampions.length > 0
      ? `Düşük galibiyet oranı: ${weakChampions.join(", ")}`
      : "",
  );
  const lpSeries = $derived(
    [...(overview?.lpHistory ?? [])].reverse().map((point) => point.lp),
  );
  const buildChampion = $derived(
    buildChampionId == null ? null : (champions[buildChampionId] ?? null),
  );
  const liveActive = $derived(
    phase === "InProgress" || phase === "GameStart" || phase === "Reconnect",
  );

  $effect(() => {
    if (!liveActive) {
      liveGame = null;
      return;
    }

    loadLive(true);
    const timer = setInterval(() => loadLive(true), 15_000);
    return () => clearInterval(timer);
  });

  $effect(() => {
    if (tab === "matches" && matches.length === 0 && !matchesLoading && !matchesError) {
      loadMatches();
    }
  });

  $effect(() => {
    if (tab !== "home" || !status?.connected) return;

    // İlk açılışta LCU maç geçmişini henüz yüklememiş olabilir; kısa gecikme.
    const timer = setTimeout(() => refreshHome(), 1_200);
    return () => clearTimeout(timer);
  });

  onMount(() => {
    loadSettings();
    loadChampions();
    refreshStatus();

    getVersion()
      .then((version) => (appVersion = version))
      .catch(() => {
        // Geliştirme ortamında fallback.
      });

    const statusTimer = setInterval(refreshStatus, 3_000);
    const draftTimer = setInterval(() => {
      if (status?.phase === "ChampSelect") refreshChamp();
    }, 5_000);
    const clockTimer = setInterval(() => tick++, 1_000);
    const homeTimer = setInterval(() => {
      if (tab === "home" && status?.connected) refreshHome();
    }, 20_000);
    const updateTimer = setInterval(
      () => {
        if (settings.autoUpdate) checkUpdates(true);
      },
      4 * 60 * 60 * 1_000,
    );
    const firstUpdate = setTimeout(() => {
      if (settings.autoUpdate) checkUpdates(true);
    }, 60_000);

    const unlistenUpdate = listen("tlas:check-update", () => checkUpdates(false)).catch(
      () => {
        // Tarayıcı önizlemesinde Tauri olay kanalı bulunmaz.
        return () => {};
      },
    );

    return () => {
      clearInterval(statusTimer);
      clearInterval(draftTimer);
      clearInterval(clockTimer);
      clearInterval(homeTimer);
      clearInterval(updateTimer);
      clearTimeout(firstUpdate);
      if (toastTimer) clearTimeout(toastTimer);
      unlistenUpdate.then((fn) => fn());
    };
  });
</script>

<svelte:head>
  <title>Tlas</title>
</svelte:head>

<div class="app">
  <header class="topbar">
    <span class="brand">Tlas</span>

    <nav class="tabs" aria-label="Ana menü">
      {#each tabs as item}
        <button
          class="tab"
          class:active={tab === item.id}
          onclick={() => (tab = item.id)}
        >
          {item.label}
        </button>
      {/each}
    </nav>

    <span class="status" class:online={status?.connected}>
      <span class="dot"></span>
      {statusLoading
        ? "Bağlanıyor…"
        : status?.connected
          ? phaseText(status.phase)
          : "LoL kapalı"}
    </span>
  </header>

  <main class="main">
    {#if tab === "home"}
      <div class="page-head">
        <div>
          <h2>Genel</h2>
          <p class="sub">Hesap, derece ve canlı lobi</p>
        </div>
        <button
          class="btn"
          onclick={() => {
            refreshStatus();
            refreshHome(true);
          }}
          disabled={homeLoading}
        >
          {homeLoading ? "Yükleniyor…" : "Yenile"}
        </button>
      </div>

      <div class="panel">
        <div class="account">
          <div class="avatar">
            {#if overview?.summoner.iconId}
              <img
                src={profileIcon(overview.summoner.iconId, ddragonVersion)}
                alt=""
                onerror={(event) => ((event.currentTarget as HTMLImageElement).style.visibility = "hidden")}
              />
            {:else}
              <span class="avatar-fallback">{avatarInitial}</span>
            {/if}
          </div>

          <div class="who">
            <strong>{overview?.summoner.name ?? status?.summonerName ?? "Bağlı hesap yok"}</strong>
            <span class="dim">
              {#if overview?.summoner.level}Seviye {overview.summoner.level} · {/if}
              {status?.platform ?? "—"} · {status?.connected ? phaseText(phase) : "LoL kapalı"}
            </span>
          </div>

          <div class="chips">
            {#each overview?.ranks ?? [] as rank (rank.queue)}
              <div class="chip">
                <span class="chip-label">{rank.label}</span>
                <span class="chip-value">
                  {#if rank.tier}
                    {rank.tier}{rank.division ? ` ${rank.division}` : ""} · {rank.lp ?? 0} LP
                  {:else}
                    Derecesiz
                  {/if}
                </span>
                <span class="chip-sub">
                  {rank.wins}G {rank.losses}M · {percent(rank.winrate)}
                </span>
              </div>
            {/each}
          </div>
        </div>
      </div>

      {#if overview}
        <section class="panel">
          <div class="panel-head">
            <span>Bugün</span>
            <span class="head-note">
              {overview.session.games > 0
                ? `${overview.session.wins}G ${overview.session.losses}M`
                : "maç yok"}
            </span>
          </div>
          <div class="panel-body">
            <div class="stat-grid">
              <div class="stat">
                <strong>{overview.session.games}</strong>
                <small>Maç</small>
              </div>
              <div class="stat">
                <strong>{percent(overview.session.winrate)}</strong>
                <small>Galibiyet</small>
              </div>
              <div class="stat">
                <strong>
                  {overview.session.avgKda != null ? overview.session.avgKda.toFixed(2) : "—"}
                </strong>
                <small>KDA</small>
              </div>
              <div class="stat">
                <strong
                  class:ok={(overview.session.lpDelta ?? 0) > 0}
                  class:warn={(overview.session.lpDelta ?? 0) < 0}
                >
                  {overview.session.lpDelta != null
                    ? `${overview.session.lpDelta > 0 ? "+" : ""}${overview.session.lpDelta} LP`
                    : "—"}
                </strong>
                <small>LP</small>
              </div>
            </div>

            {#if overview.session.streak > 0}
              <p class="hint">{overview.session.streak} maçlık galibiyet serisi.</p>
            {:else if overview.session.streak < 0}
              <p class="hint">{-overview.session.streak} maçlık mağlubiyet serisi.</p>
            {:else if overview.session.games === 0}
              <p class="hint">Bugün henüz maç oynanmadı.</p>
            {/if}
          </div>
        </section>
      {/if}

      {#if liveGame}
        <section class="panel">
          <div class="panel-head">
            <span>Canlı maç · {liveGame.queueName ?? queueName(liveGame.queueId)}</span>
            <span class="head-note">
              {liveGame.allies.length + liveGame.enemies.length} oyuncu
            </span>
          </div>
          <div class="live-grid">
            <div class="live-team">
              <span class="live-title">Takımın</span>
              {#each liveGame.allies as player (player.championId)}
                <div class="live-row">
                  <span class="member-icon">
                    {#if champions[player.championId]}
                      <img
                        src={championIcon(champions[player.championId].id, ddragonVersion)}
                        alt=""
                        loading="lazy"
                        onerror={(event) => ((event.currentTarget as HTMLImageElement).style.visibility = "hidden")}
                      />
                    {/if}
                  </span>
                  <span class="member-name">
                    {champions[player.championId]?.name ?? `Şampiyon ${player.championId}`}
                  </span>
                  <span class="member-pos">{player.displayName}</span>
                </div>
              {/each}
            </div>

            <div class="live-team">
              <span class="live-title">Rakip</span>
              {#each liveGame.enemies as player (player.championId)}
                <div class="live-row">
                  <span class="member-icon">
                    {#if champions[player.championId]}
                      <img
                        src={championIcon(champions[player.championId].id, ddragonVersion)}
                        alt=""
                        loading="lazy"
                        onerror={(event) => ((event.currentTarget as HTMLImageElement).style.visibility = "hidden")}
                      />
                    {/if}
                  </span>
                  <span class="member-name">
                    {champions[player.championId]?.name ?? `Şampiyon ${player.championId}`}
                  </span>
                  <span class="member-pos">{player.displayName}</span>
                </div>
              {/each}
            </div>
          </div>
        </section>
      {/if}

      {#if lobby && lobby.members.length > 0}
        <div class="panel">
          <div class="panel-head">
            <span>Canlı lobi · {lobby.queueName ?? queueName(lobby.queueId)}</span>
            <span class="head-note">{lobby.members.length} oyuncu</span>
          </div>
          <div class="rows">
            {#each lobby.members as member}
              <div class="row">
                <span class="member">
                  <span class="member-icon">
                    {#if member.iconId}
                      <img
                        src={profileIcon(member.iconId, ddragonVersion)}
                        alt=""
                        onerror={(event) => ((event.currentTarget as HTMLImageElement).style.visibility = "hidden")}
                      />
                    {/if}
                  </span>
                  <span class="member-name" class:me={member.isSelf}>{member.name}</span>
                  <span class="member-pos">{roleName(member.position)}</span>
                </span>
                <span class="value">
                  {#if member.tier}
                    {member.tier}{member.division ? ` ${member.division}` : ""} · {member.lp ?? 0} LP
                    <span class="dim">· {member.wins ?? 0}G {member.losses ?? 0}M</span>
                  {:else}
                    <span class="dim">Derecesiz</span>
                  {/if}
                </span>
              </div>
            {/each}
          </div>
        </div>
      {/if}

      <div class="grid-2">
        <section class="panel">
          <div class="panel-head">
            <span>Son maçlar</span>
            <span class="head-note">son {overview?.recentGames ?? 0} maç</span>
          </div>
          <div class="panel-body">
            {#if overview && overview.recentGames > 0}
              <div class="form-strip" aria-label="Son maçlar">
                {#each overview.recentForm as win}
                  <span class="form-dot" class:win aria-hidden="true"></span>
                {/each}
              </div>
              <div class="rows">
                <div class="row">
                  <span class="key">Galibiyet / Mağlubiyet</span>
                  <span class="value">{overview.recentWins}G {overview.recentGames - overview.recentWins}M</span>
                </div>
                <div class="row">
                  <span class="key">Galibiyet oranı</span>
                  <span class="value">{percent(overview.recentWinrate)}</span>
                </div>
                <div class="row">
                  <span class="key">Seri</span>
                  <span class="value" class:ok={overview.streak > 0} class:warn={overview.streak < 0}>
                    {#if overview.streak > 0}
                      {overview.streak} galibiyet
                    {:else if overview.streak < 0}
                      {-overview.streak} mağlubiyet
                    {:else}
                      —
                    {/if}
                  </span>
                </div>
              </div>
            {:else}
              <div class="empty">Maç geçmişi yok.</div>
            {/if}
          </div>
        </section>

        <section class="panel">
          <div class="panel-head">
            <span>LP geçmişi</span>
            {#if overview && overview.lpHistory.length >= 2}
              <span class="head-note">son {overview.lpHistory.length} kayıt</span>
            {/if}
          </div>
          {#if overview && overview.lpHistory.length >= 2}
            <div class="spark-box">
              <Sparkline points={lpSeries} />
            </div>
          {/if}
          <div class="rows">
            {#if overview && overview.lpHistory.length > 0}
              {#each overview.lpHistory as point (point.ts)}
                <div class="row">
                  <span class="key">
                    {point.tier}{point.division ? ` ${point.division}` : ""} · {point.lp} LP
                    <span class="dim">· {timeAgo(point.ts * 1000)}</span>
                  </span>
                  <span
                    class="value"
                    class:ok={(point.delta ?? 0) > 0}
                    class:warn={(point.delta ?? 0) < 0}
                  >
                    {lpDelta(point.delta)}
                  </span>
                </div>
              {/each}
            {:else}
              <div class="empty">Henüz kayıt yok.</div>
            {/if}
          </div>
        </section>
      </div>

      <section class="panel">
        <div class="panel-head">
          <span>Şampiyonlar</span>
          <span class="head-note">son {overview?.recentGames ?? 0} maç</span>
        </div>
        {#if overview && overview.champions.length > 0}
          <div class="champ-head">
            <span>Şampiyon</span>
            <span>Maç</span>
            <span>Galibiyet</span>
            <span>KDA</span>
            <span>Ustalık</span>
          </div>
          {#each overview.champions as champion (champion.championId)}
            <button
              type="button"
              class="champ-row"
              class:selected={buildChampionId === champion.championId}
              aria-expanded={buildChampionId === champion.championId}
              onclick={() => openBuild(champion.championId)}
            >
              <span class="member">
                <span class="member-icon">
                  {#if champions[champion.championId]}
                    <img
                      src={championIcon(champions[champion.championId].id, ddragonVersion)}
                      alt=""
                      loading="lazy"
                      onerror={(event) => ((event.currentTarget as HTMLImageElement).style.visibility = "hidden")}
                    />
                  {/if}
                </span>
                <span class="member-name">
                  {champions[champion.championId]?.name ?? `Şampiyon ${champion.championId}`}
                </span>
              </span>
              <span class="num">{champion.games}</span>
              <span class="num">{percent(champion.winrate)}</span>
              <span class="num">{champion.kda.toFixed(2)}</span>
              <span class="num" class:dim={champion.points == null}>
                {champion.points != null ? `${(champion.points / 1000).toFixed(0)}B` : "—"}
              </span>
            </button>
          {/each}
        {:else}
          <div class="empty">Veri yok.</div>
        {/if}
      </section>

      {#if buildChampionId != null}
        <section class="panel">
          <div class="panel-head">
            <span>
              Kişisel build · {buildChampion?.name ?? `Şampiyon ${buildChampionId}`}
            </span>
            <button class="btn small ghost" type="button" onclick={closeBuild}>Kapat</button>
          </div>

          {#if buildLoading}
            <div class="empty">Build yükleniyor…</div>
          {:else if buildError}
            <div class="empty">
              <strong>Build alınamadı</strong>
              {buildError}
            </div>
          {:else if build}
            <div class="panel-body">
              {#if build.games > 0}
                <div class="stat-grid">
                  <div class="stat">
                    <strong>{build.games}</strong>
                    <small>Maç</small>
                  </div>
                  <div class="stat">
                    <strong>{percent(build.winrate)}</strong>
                    <small>Galibiyet</small>
                  </div>
                  <div class="stat">
                    <strong>{build.kda.toFixed(2)}</strong>
                    <small>KDA</small>
                  </div>
                  <div class="stat">
                    <strong>{Math.round(build.avgCs)}</strong>
                    <small>Ort. CS</small>
                  </div>
                </div>

                <div class="build-row">
                  <span class="build-label">Çekirdek</span>
                  <span class="build-items">
                    {#each build.coreItems as item (item.itemId)}
                      <span class="build-item" title={`${items[item.itemId] ?? item.itemId} · ${item.count} maç · ${percent(item.winrate)}`}>
                        <img
                          src={itemIcon(item.itemId, ddragonVersion)}
                          alt={items[item.itemId] ?? `Item ${item.itemId}`}
                          loading="lazy"
                          onerror={(event) => ((event.currentTarget as HTMLImageElement).style.visibility = "hidden")}
                        />
                        <small>{item.count}×</small>
                      </span>
                    {:else}
                      <span class="dim">Yok</span>
                    {/each}
                  </span>
                </div>

                <div class="build-row">
                  <span class="build-label">Ayakkabı</span>
                  <span class="build-items">
                    {#each build.boots as item (item.itemId)}
                      <span class="build-item" title={`${items[item.itemId] ?? item.itemId} · ${item.count} maç`}>
                        <img
                          src={itemIcon(item.itemId, ddragonVersion)}
                          alt={items[item.itemId] ?? `Item ${item.itemId}`}
                          loading="lazy"
                          onerror={(event) => ((event.currentTarget as HTMLImageElement).style.visibility = "hidden")}
                        />
                        <small>{item.count}×</small>
                      </span>
                    {:else}
                      <span class="dim">Yok</span>
                    {/each}
                  </span>
                </div>
              {:else}
                <p class="hint">Bu şampiyonla son 50 maçta kayıt yok.</p>
              {/if}

              <div class="meta-box">
                <div class="meta-head">
                  <span>OP.GG</span>
                  <button class="btn small" type="button" onclick={openOpgg}>
                    Tarayıcıda aç
                  </button>
                </div>

                {#if settings.experimentalMeta}
                  <div class="meta-controls">
                    <select bind:value={metaPosition} aria-label="Pozisyon">
                      <option value="top">Üst</option>
                      <option value="jungle">Orman</option>
                      <option value="mid">Orta</option>
                      <option value="adc">Alt</option>
                      <option value="support">Destek</option>
                    </select>
                    <button class="btn small" type="button" onclick={fetchMeta} disabled={metaLoading}>
                      {metaLoading ? "Getiriliyor…" : "Meta getir"}
                    </button>
                  </div>

                  {#if metaError}
                    <p class="hint warn-text">{metaError}</p>
                  {:else if meta}
                    <div class="meta-stats">
                      <span>{metaTier(meta.tier)}</span>
                      <span>Galibiyet {percent(meta.winrate)}</span>
                      <span>Seçilme {percent(meta.pickrate)}</span>
                      <span>Ban {percent(meta.banrate)}</span>
                      {#if meta.cached}<span class="dim">önbellek</span>{/if}
                    </div>
                    {#if meta.rune || meta.coreItems.length > 0}
                      <p class="hint">
                        {#if meta.rune}Rün: {meta.rune}. {/if}
                        {#if meta.coreItems.length > 0}
                          Çekirdek: {meta.coreItems.join(", ")}.
                        {/if}
                      </p>
                    {/if}
                  {/if}

                  <p class="hint">
                    Yalnızca şampiyon adı ve pozisyon gönderilir. Riot ID gönderilmez;
                    yanıtlar 24 saat cihazda saklanır.
                  </p>
                {:else}
                  <p class="hint">
                    Deneysel meta kapalı. Ayarlar sekmesinden açabilirsin.
                  </p>
                {/if}
              </div>
            </div>
          {/if}
        </section>
      {/if}

      <div class="actions">
        <button
          class="btn primary"
          onclick={() => {
            tab = "draft";
            refreshChamp(true);
          }}
        >
          Şampiyon seçimi
        </button>
        <button
          class="btn"
          onclick={() => {
            tab = "matches";
            loadMatches(true);
          }}
        >
          Maç geçmişi
        </button>
        <button class="btn" onclick={() => checkUpdates(false)} disabled={updateBusy}>
          Güncelleme kontrolü
        </button>
        <button class="btn ghost" onclick={() => (tab = "settings")}>Ayarlar</button>
      </div>
    {:else if tab === "draft"}
      <div class="page-head">
        <div>
          <h2>Şampiyon Seçimi</h2>
          <p class="sub">
            {#if champ}
              {champ.allies.length} takım · {champ.enemies.length} rakip
              {#if timerSeconds != null}· {timerSeconds} sn{/if}
            {:else}
              Draft bekleniyor
            {/if}
          </p>
        </div>
        <button
          class="btn"
          onclick={() => refreshChamp(true)}
          disabled={champLoading}
        >
          {champLoading ? "Yükleniyor…" : "Yenile"}
        </button>
      </div>

      {#if !champ}
        <div class="panel">
          <div class="empty">
            <strong>Şampiyon seçimi bekleniyor</strong>
            Sıra başladığında takım ve rakip listesi burada görünür.
          </div>
        </div>
      {:else}
        {#if enemiesHidden}
          <div class="notice">
            Rakip isimleri gizli.
          </div>
        {/if}

        <div class="teams">
          <section class="panel team">
            <div class="panel-head">
              Takımın
              <span>{champ.allies.length}</span>
            </div>
            <div class="panel-body">
              {#each champ.allies as player}
                <PlayerCard {player} {champions} version={ddragonVersion} />
              {/each}
            </div>
          </section>

          <section class="panel team">
            <div class="panel-head">
              Rakip
              <span>{champ.enemies.length}</span>
            </div>
            <div class="panel-body">
              {#each champ.enemies as player}
                <PlayerCard {player} {champions} version={ddragonVersion} />
              {/each}
            </div>
          </section>
        </div>

        <div class="grid-2">
          <DraftComp
            title="Takımın kompozisyonu"
            players={champ.allies}
            {champions}
            note={weakNote}
          />
          <DraftComp
            title="Rakip kompozisyonu"
            players={champ.enemies}
            {champions}
          />
        </div>
      {/if}
    {:else if tab === "matches"}
      <div class="page-head">
        <div>
          <h2>Maçlar</h2>
          <p class="sub">Son 20 maç</p>
        </div>
        <button class="btn" onclick={() => loadMatches(true)} disabled={matchesLoading}>
          {matchesLoading ? "Yükleniyor…" : "Yenile"}
        </button>
      </div>

      {#if matchesError && matches.length > 0}
        <div class="notice">
          {matchesError}
          <button class="btn small ghost" onclick={() => (tab = "settings")}>
            Ayarlar
          </button>
        </div>
      {/if}

      {#if matchesLoading && matches.length === 0}
        <div class="panel">
          <div class="empty">Maçlar yükleniyor…</div>
        </div>
      {:else if matches.length === 0}
        <div class="panel">
          <div class="empty">
            {#if matchesError}
              <strong>Maç geçmişi alınamadı</strong>
              {matchesError}
              <div class="empty-actions">
                <button class="btn small" onclick={() => (tab = "settings")}>
                  Ayarlar
                </button>
              </div>
            {:else}
              <strong>Maç bulunamadı</strong>
              Maç yok.
            {/if}
          </div>
        </div>
      {:else}
        <div class="panel">
          <div class="match-head">
            <span>Sonuç</span>
            <span>Şampiyon</span>
            <span>Kuyruk / Rol</span>
            <span>KDA</span>
            <span>CS</span>
            <span>Süre</span>
          </div>
          {#each matches as match (match.matchId)}
            <MatchRow
              {match}
              {champions}
              version={ddragonVersion}
              selected={selectedMatchId === match.matchId}
              onselect={() => openMatch(match.matchId)}
            />
          {/each}
        </div>

        {#if selectedMatchId}
          {#if matchDetailLoading}
            <div class="panel">
              <div class="empty">Maç detayı yükleniyor…</div>
            </div>
          {:else if matchDetailError}
            <div class="panel">
              <div class="empty">
                <strong>Maç detayı alınamadı</strong>
                {matchDetailError}
              </div>
            </div>
          {:else if matchDetail}
            <MatchDetail
              detail={matchDetail}
              {champions}
              {items}
              version={ddragonVersion}
              onclose={() => openMatch(matchDetail!.matchId)}
            />
          {/if}
        {/if}
      {/if}
    {:else}
      <div class="page-head">
        <div>
          <h2>Ayarlar</h2>
          <p class="sub">Uygulama</p>
        </div>
        <button class="btn primary" onclick={persistSettings}>Kaydet</button>
      </div>

      <div class="settings-grid">
        <section class="panel">
          <div class="panel-head">Veri kaynağı</div>
          <div class="panel-body">
            <p class="hint">
              API anahtarı yok. Oyuncu ve maç verisi LoL istemcisinden (LCU), şampiyon
              adları ve görseller Data Dragon'dan alınır.
            </p>

            <div class="rows">
              <div class="row">
                <span class="key">LoL istemcisi</span>
                <span class="value" class:ok={status?.connected} class:warn={!status?.connected}>
                  {status?.connected ? "Bağlı" : "Kapalı"}
                </span>
              </div>
              <div class="row">
                <span class="key">Hesap</span>
                <span class="value">{status?.summonerName ?? "—"}</span>
              </div>
              <div class="row">
                <span class="key">Sunucu</span>
                <span class="value">{status?.platform ?? "—"}</span>
              </div>
            </div>
          </div>
        </section>

        <section class="panel">
          <div class="panel-head">Güncellemeler</div>
          <div class="panel-body">
            <label class="switch">
              <input type="checkbox" bind:checked={settings.autoUpdate} />
              Arka planda otomatik kontrol
            </label>

            <div class="actions">
              <button class="btn" onclick={() => checkUpdates(false)} disabled={updateBusy}>
                {updateBusy ? "Kontrol ediliyor…" : "Şimdi kontrol et"}
              </button>
            </div>

            {#if updateStatus}
              <p class="hint">{updateStatus}</p>
            {/if}

            {#if updateInfo}
              <div class="update-box">
                <strong>v{updateInfo.version}</strong>
                {#if updateInfo.body}
                  <p class="hint">{updateInfo.body}</p>
                {/if}
                <div class="actions">
                  <button
                    class="btn primary small"
                    onclick={installUpdate}
                    disabled={updateBusy || phase === "InProgress" || phase === "ChampSelect"}
                  >
                    İndir ve yeniden başlat
                  </button>
                </div>
              </div>
            {/if}

            <p class="hint">Maç veya şampiyon seçimi sırasında kurulmaz.</p>
          </div>
        </section>

        <section class="panel">
          <div class="panel-head">Deneysel meta</div>
          <div class="panel-body">
            <label class="switch">
              <input type="checkbox" bind:checked={settings.experimentalMeta} />
              OP.GG meta verisini kullan
            </label>

            <p class="hint">
              Varsayılan kapalı. Açıkken yalnızca şampiyon adı ve pozisyon OP.GG MCP
              sunucusuna gönderilir. Riot ID, PUUID veya hesap bilgisi gönderilmez;
              yanıtlar 24 saat cihazda tutulur.
            </p>

            <p class="hint">
              Kapalıyken OP.GG bağlantısı yalnızca tarayıcıda açılır.
            </p>
          </div>
        </section>

        <section class="panel">
          <div class="panel-head">Yerel veri</div>
          <div class="panel-body">
            <p class="hint">
              Scout 10 dk, hesap özeti 60 sn, ustalık 1 saat, kuyruk adları 1 gün,
              meta 24 saat önbellekte tutulur. LP geçmişi cihazda saklanır.
            </p>
            <div class="actions">
              <button class="btn ghost" onclick={wipeCache}>Önbelleği temizle</button>
            </div>
          </div>
        </section>

        <section class="panel">
          <div class="panel-head">Hakkında</div>
          <div class="panel-body">
            <p><strong>Tlas</strong> v{appVersion}</p>
          </div>
        </section>
      </div>
    {/if}
  </main>

  {#if toast}
    <div class="toast">{toast}</div>
  {/if}

  <footer class="statusbar">
    <span>v{appVersion}</span>
  </footer>
</div>