<script lang="ts">
  import { onMount } from "svelte";
  import { getVersion } from "@tauri-apps/api/app";
  import { listen } from "@tauri-apps/api/event";
  import { check, type Update } from "@tauri-apps/plugin-updater";
  import { relaunch } from "@tauri-apps/plugin-process";

  import MatchRow from "$lib/components/MatchRow.svelte";
  import PlayerCard from "$lib/components/PlayerCard.svelte";
  import {
    clearCache,
    getAccountOverview,
    getChampSelect,
    getLobby,
    getRecentMatches,
    getSettings,
    getStatus,
    saveSettings,
  } from "$lib/api";
  import {
    championIcon,
    errorText,
    lpDelta,
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
    LobbyView,
    MatchSummary,
    Settings,
  } from "$lib/types";

  type Tab = "home" | "draft" | "matches" | "settings";

  let tab = $state<Tab>("home");
  let status = $state<LcuStatus | null>(null);
  let champ = $state<ChampSelectView | null>(null);
  let matches = $state<MatchSummary[]>([]);
  let overview = $state<AccountOverview | null>(null);
  let lobby = $state<LobbyView | null>(null);
  let settings = $state<Settings>({
    autoUpdate: true,
  });
  let champions = $state<Record<number, ChampionInfo>>({});
  let ddragonVersion = $state("16.21.1");
  let appVersion = $state(__APP_VERSION__);

  let statusLoading = $state(true);
  let champLoading = $state(false);
  let matchesLoading = $state(false);
  let matchesLoaded = $state(false);
  let matchesError = $state("");
  let homeLoading = $state(false);
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
  let homeRetried = false;
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
    const cached = localStorage.getItem("tlas:champions");

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
      }>) {
        map[Number(champion.key)] = { id: champion.id, name: champion.name };
      }

      ddragonVersion = version;
      champions = map;
      localStorage.setItem(
        "tlas:champions",
        JSON.stringify({ at: Date.now(), version, map }),
      );
    } catch {
      // Offline: placeholder ikonlar kalır.
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
      return;
    }

    const now = Date.now();
    if (!force && now - lastHomeFetch < 8_000) return;
    if (homeLoading) return;

    homeLoading = true;

    try {
      const [nextOverview, nextLobby] = await Promise.all([
        getAccountOverview().catch(() => null),
        getLobby().catch(() => null),
      ]);

      overview = nextOverview;
      lobby = nextLobby;
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
          <div class="panel-head">LP geçmişi</div>
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
            <div class="champ-row">
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
            </div>
          {/each}
        {:else}
          <div class="empty">Veri yok.</div>
        {/if}
      </section>

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
            <MatchRow {match} {champions} version={ddragonVersion} />
          {/each}
        </div>
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
          <div class="panel-head">Yerel veri</div>
          <div class="panel-body">
            <p class="hint">
              Scout 10 dk, kuyruk adları 1 gün önbellekte tutulur. LP geçmişi cihazda
              saklanır.
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