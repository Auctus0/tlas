import { invoke } from "@tauri-apps/api/core";

import type { ChampSelectView, LcuStatus, MatchSummary, Settings } from "$lib/types";

export const getStatus = () => invoke<LcuStatus>("lcu_status");
export const getChampSelect = () => invoke<ChampSelectView | null>("champ_select");
export const getRecentMatches = (count = 20) =>
  invoke<MatchSummary[]>("recent_matches", { count });
export const getSettings = () => invoke<Settings>("get_settings");
export const saveSettings = (settings: Settings) =>
  invoke<void>("save_settings", { settings });
export const clearCache = () => invoke<void>("clear_cache");
