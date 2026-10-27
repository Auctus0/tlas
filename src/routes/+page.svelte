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
    getChampSelect,
    getRecentMatches,
    getSettings,
    getStatus,
    saveSettings,
  } from "$lib/api";
  import { errorText, phaseText } from "$lib/format";
  import type {
    ChampionInfo,
    ChampSelectView,
    LcuStatus,
    MatchSummary,
    Settings,
  } from "$lib/types";

  type Tab = "home" | "draft" | "matches" | "settings";

  let tab = $state<Tab>("home");
  let status = $state<LcuStatus | null>(null);
  let champ = $state<ChampSelectView | null>(null);
  let matches = $state<MatchSummary[]>([]);
  let settings = $state<Settings>({
    riotApiKey: "",
    platform: "tr1",
    autoUpdate: true,
  });
  let champions = $state<Record<number, ChampionInfo>>({});
  let ddragonVersion = $state("16.21.1");
  let appVersion = $state("0.1.0");

  let statusLoading = $state(true);
  let champLoading = $state(false);
  let matchesLoading = $state(false);
  let matchesLoaded = $state(false);
  let matchesError = $state("");
  let toast = $state("");
  let showKey = $state(false);
  let updateInfo = $state<{ version: string; body: string | null } | null>(null);
  let pendingUpdate = $state<Update | null>(null);
  let updateStatus = $state("");
  let updateBusy = $state(false);
  let downloaded = $state(0);
  let totalDownload = $state(0);

  let lastDraftFetch = 0;
  let draftFetchedAt = 0;
  let tick = $state(0);
  let toastTimer: ReturnType<typeof setTimeout> | undefined;

  const platforms = [
    ["tr1", "Türkiye"],
    ["euw1", "EU West"],
    ["eun1", "EU Nordic & East"],
    ["na1", "North America"],
    ["kr", "Korea"],
    ["br1", "Brazil"],
    ["la1", "LA North"],
    ["la2", "LA South"],
    ["me1", "Middle East"],
    ["jp1", "Japan"],
    ["oc1", "Oceania"],
    ["ph2", "Philippines"],
    ["sg2", "Singapore"],
    ["th2", "Thailand"],
    ["tw2", "Taiwan"],
    ["vn2", "Vietnam"],
    ["ru", "Russia"],
  ];

  const tabs: { id: Tab; label: string }[] = [
    { id: "home", label: "Genel" },
    { id: "draft", label: "Champ Select" },
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
        "Maçlar alınamadı. League istemcisi ve API anahtarı gerekli.",
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
  const timerSeconds = $derived.by(() => {
    void tick;
    if (!champ?.timer) return null;
    const elapsed = draftFetchedAt ? Date.now() - draftFetchedAt : 0;
    return Math.max(0, Math.ceil((champ.timer - elapsed) / 1_000));
  });
  const apiReady = $derived(settings.riotApiKey.trim().length > 0);
  const enemiesHidden = $derived(
    (champ?.enemies.length ?? 0) > 0 && champ!.enemies.every((player) => player.hidden),
  );

  $effect(() => {
    if (tab === "matches" && matches.length === 0 && !matchesLoading && !matchesError) {
      loadMatches();
    }
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
          : "League kapalı"}
    </span>
  </header>

  <main class="main">
    {#if tab === "home"}
      <div class="page-head">
        <div>
          <h2>Genel</h2>
          <p class="sub">League istemcisi ve hesap durumu</p>
        </div>
        <button class="btn" onclick={refreshStatus} disabled={statusLoading}>
          Yenile
        </button>
      </div>

      <div class="panel">
        <div class="panel-head">Durum</div>
        <div class="rows">
          <div class="row">
            <span class="key">League Client</span>
            <span class="value" class:ok={status?.connected} class:warn={!status?.connected}>
              {status?.connected ? "Bağlı" : "Kapalı"}
            </span>
          </div>
          <div class="row">
            <span class="key">Oyun fazı</span>
            <span class="value">{status?.connected ? phaseText(phase) : "—"}</span>
          </div>
          <div class="row">
            <span class="key">Hesap</span>
            <span class="value">{status?.summonerName ?? "—"}</span>
          </div>
          <div class="row">
            <span class="key">Platform</span>
            <span class="value">{status?.platform ?? settings.platform}</span>
          </div>
          <div class="row">
            <span class="key">Riot API</span>
            <span class="value" class:ok={apiReady} class:warn={!apiReady}>
              {apiReady ? "Anahtar tanımlı" : "Anahtar yok"}
            </span>
          </div>
          <div class="row">
            <span class="key">Sürüm</span>
            <span class="value">v{appVersion}</span>
          </div>
        </div>
      </div>

      <div class="actions">
        <button
          class="btn primary"
          onclick={() => {
            tab = "draft";
            refreshChamp(true);
          }}
        >
          Champ Select
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

      <p class="hint">
        Tlas yalnızca LCU, Riot Web API ve Data Dragon kullanır. Bellek okuma,
        enjeksiyon veya girdi otomasyonu yoktur.
      </p>
    {:else if tab === "draft"}
      <div class="page-head">
        <div>
          <h2>Champ Select</h2>
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

      {#if !apiReady}
        <div class="notice">
          Riot API anahtarı tanımlı değil. Rank ve son maç verileri gösterilmez.
          <button class="btn small ghost" onclick={() => (tab = "settings")}>
            Ayarlar
          </button>
        </div>
      {/if}

      {#if !champ}
        <div class="panel">
          <div class="empty">
            <strong>Şampiyon seçimi bekleniyor</strong>
            League'de champ select başladığında takım ve rakip listesi burada görünür.
          </div>
        </div>
      {:else}
        {#if enemiesHidden}
          <div class="notice">
            Rakip isimleri gizli. Tlas gizli bilgiyi açığa çıkarmaz.
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
          <p class="sub">Son 20 maç özeti</p>
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
              Bir maç oynadıktan sonra tekrar dene.
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
          <p class="sub">API, güncelleme ve yerel veri</p>
        </div>
        <button class="btn primary" onclick={persistSettings}>Kaydet</button>
      </div>

      <div class="settings-grid">
        <section class="panel">
          <div class="panel-head">Riot API</div>
          <div class="panel-body">
            <div class="field">
              <label for="api-key">API anahtarı</label>
              <div class="inline">
                <input
                  id="api-key"
                  type={showKey ? "text" : "password"}
                  bind:value={settings.riotApiKey}
                  placeholder="RGAPI-..."
                  autocomplete="off"
                  spellcheck="false"
                />
                <button class="btn small" onclick={() => (showKey = !showKey)}>
                  {showKey ? "Gizle" : "Göster"}
                </button>
              </div>
            </div>

            <div class="field">
              <label for="platform">Platform</label>
              <select id="platform" bind:value={settings.platform}>
                {#each platforms as [value, label]}
                  <option {value}>{label}</option>
                {/each}
              </select>
            </div>

            <p class="hint">
              Development key 24 saatte bir yenilenir. Anahtar yalnızca bu cihazda
              saklanır.
            </p>
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

            <p class="hint">Oyun veya champ select sırasında güncelleme kurulmaz.</p>
          </div>
        </section>

        <section class="panel">
          <div class="panel-head">Yerel veri</div>
          <div class="panel-body">
            <p class="hint">
              Scout verisi 5 dakika, maç detayları 30 gün önbellekte tutulur.
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
            <p class="hint">
              Draft scout ve maç özeti. Riot Games ile bağlantılı değildir. Tlas isn't
              endorsed by Riot Games and doesn't reflect the views or opinions of Riot
              Games or anyone officially involved in producing or managing Riot Games
              properties.
            </p>
          </div>
        </section>
      </div>
    {/if}
  </main>

  {#if toast}
    <div class="toast">{toast}</div>
  {/if}

  <footer class="statusbar">
    <span>Ban-safe: bellek okuma, enjeksiyon ve girdi otomasyonu yok</span>
    <span>v{appVersion}</span>
  </footer>
</div>