import { invoke } from "@tauri-apps/api/core";
import { openUrl } from "@tauri-apps/plugin-opener";

import type {
  AccountOverview,
  ChampSelectView,
  DraftInsight,
  LcuStatus,
  LiveGameView,
  LobbyView,
  MatchDetail,
  MatchSummary,
  MetaReport,
  PersonalBuild,
  Settings,
} from "$lib/types";

/**
 * Tarayıcı önizlemesi: `npm run dev` ile açılan sayfa Tauri IPC'ye erişemez.
 * Bu durumda gerçek LCU çağrıları yerine örnek veri döner. `import.meta.env.DEV`
 * üretim derlemesinde false olduğu için örnek veri pakete girmez.
 */
const preview =
  import.meta.env.DEV &&
  typeof window !== "undefined" &&
  !("__TAURI_INTERNALS__" in window);

const loadPreview = () => import("$lib/preview");

export const getStatus = async (): Promise<LcuStatus> =>
  preview ? (await loadPreview()).previewStatus : invoke<LcuStatus>("lcu_status");

export const getChampSelect = async (): Promise<ChampSelectView | null> =>
  preview
    ? (await loadPreview()).previewChampSelect
    : invoke<ChampSelectView | null>("champ_select");

export const getDraftInsights = async (): Promise<DraftInsight[]> =>
  preview
    ? (await loadPreview()).previewDraftInsights
    : invoke<DraftInsight[]>("draft_insights");

export const getRecentMatches = async (count = 20): Promise<MatchSummary[]> =>
  preview
    ? (await loadPreview()).previewMatches.slice(0, count)
    : invoke<MatchSummary[]>("recent_matches", { count });

export const getMatchDetail = async (matchId: string): Promise<MatchDetail> =>
  preview
    ? (await loadPreview()).previewMatchDetail(matchId)
    : invoke<MatchDetail>("match_detail", { gameId: Number(matchId) });

export const getPersonalBuild = async (championId: number): Promise<PersonalBuild> =>
  preview
    ? (await loadPreview()).previewBuild(championId)
    : invoke<PersonalBuild>("personal_build", { championId });

export const getLiveGame = async (): Promise<LiveGameView | null> =>
  preview
    ? (await loadPreview()).previewLiveGame
    : invoke<LiveGameView | null>("live_game");

export const getOpggMeta = async (
  champion: string,
  position: string,
): Promise<MetaReport> =>
  preview
    ? (await loadPreview()).previewMeta(champion, position)
    : invoke<MetaReport>("opgg_meta", { champion, position });

export const openExternal = async (url: string): Promise<void> => {
  if (preview) {
    window.open(url, "_blank", "noopener");
    return;
  }

  await openUrl(url);
};

export const getAccountOverview = async (): Promise<AccountOverview> =>
  preview
    ? (await loadPreview()).previewOverview
    : invoke<AccountOverview>("account_overview");

export const getLobby = async (): Promise<LobbyView | null> =>
  preview ? (await loadPreview()).previewLobby : invoke<LobbyView | null>("lobby_view");

export const getSettings = async (): Promise<Settings> =>
  preview
    ? { autoUpdate: true, experimentalMeta: false, autoAccept: false, autoPickChampionId: null, autoBanChampionId: null }
    : invoke<Settings>("get_settings");

export const saveSettings = async (settings: Settings): Promise<void> => {
  if (preview) return;
  await invoke<void>("save_settings", { settings });
};

export const clearCache = async (): Promise<void> => {
  if (preview) return;
  await invoke<void>("clear_cache");
};

/** Şampiyon seçimi kaplamasını göster/gizle. Konum backend'de hesaplanır. */
export const setOverlayVisible = async (visible: boolean): Promise<void> => {
  if (preview) return;
  await invoke<void>("set_overlay_visible", { visible });
};

/** Kaplamayı LoL istemcisinin yanına yeniden hizala. */
export const placeOverlay = async (): Promise<void> => {
  if (preview) return;
  await invoke<void>("place_overlay");
};
