import { invoke } from "@tauri-apps/api/core";

import type {
  AccountOverview,
  ChampSelectView,
  LcuStatus,
  LobbyView,
  MatchSummary,
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

export const getRecentMatches = async (count = 20): Promise<MatchSummary[]> =>
  preview
    ? (await loadPreview()).previewMatches.slice(0, count)
    : invoke<MatchSummary[]>("recent_matches", { count });

export const getAccountOverview = async (): Promise<AccountOverview> =>
  preview
    ? (await loadPreview()).previewOverview
    : invoke<AccountOverview>("account_overview");

export const getLobby = async (): Promise<LobbyView | null> =>
  preview ? (await loadPreview()).previewLobby : invoke<LobbyView | null>("lobby_view");

export const getSettings = async (): Promise<Settings> =>
  preview ? { autoUpdate: true } : invoke<Settings>("get_settings");

export const saveSettings = async (settings: Settings): Promise<void> => {
  if (preview) return;
  await invoke<void>("save_settings", { settings });
};

export const clearCache = async (): Promise<void> => {
  if (preview) return;
  await invoke<void>("clear_cache");
};
