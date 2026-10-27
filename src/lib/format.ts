const PHASES: Record<string, string> = {
  None: "Boşta",
  Lobby: "Lobi",
  Matchmaking: "Eşleşme aranıyor",
  ReadyCheck: "Hazır kontrolü",
  ChampSelect: "Şampiyon seçimi",
  GameStart: "Oyun başlıyor",
  InProgress: "Maçta",
  Reconnect: "Yeniden bağlanıyor",
  WaitingForStats: "İstatistik bekleniyor",
  EndOfGame: "Maç sonu",
  TerminatedInError: "Hata",
};

const QUEUES: Record<number, string> = {
  0: "Custom",
  400: "Normal Draft",
  420: "Solo/Duo",
  430: "Normal Blind",
  440: "Flex",
  450: "ARAM",
  490: "Quickplay",
  700: "Clash",
  720: "ARAM Clash",
  900: "URF",
  1700: "Arena",
  1900: "URF",
};

const ROLES: Record<string, string> = {
  TOP: "Üst",
  JUNGLE: "Orman",
  MIDDLE: "Orta",
  BOTTOM: "Alt",
  UTILITY: "Destek",
};

export function phaseText(phase: string | null | undefined): string {
  if (!phase) return "Bağlı değil";
  return PHASES[phase] ?? phase;
}

export function queueName(queueId: number): string {
  return QUEUES[queueId] ?? `Queue ${queueId}`;
}

export function roleName(role: string | null | undefined): string {
  if (!role) return "—";
  return ROLES[role] ?? role;
}

export function formatDuration(seconds: number): string {
  const safe = Math.max(0, Math.floor(seconds));
  const minutes = Math.floor(safe / 60);
  const rest = (safe % 60).toString().padStart(2, "0");
  return `${minutes}:${rest}`;
}

export function timeAgo(timestamp: number): string {
  const diff = Date.now() - timestamp;

  if (diff < 60_000) return "az önce";
  if (diff < 3_600_000) return `${Math.floor(diff / 60_000)} dk önce`;
  if (diff < 86_400_000) return `${Math.floor(diff / 3_600_000)} sa önce`;
  if (diff < 7 * 86_400_000) return `${Math.floor(diff / 86_400_000)} gün önce`;

  return new Date(timestamp).toLocaleDateString("tr-TR", {
    day: "2-digit",
    month: "short",
  });
}

export function championIcon(championKey: string, version: string): string {
  return `https://ddragon.leagueoflegends.com/cdn/${version}/img/champion/${encodeURIComponent(championKey)}.png`;
}

/**
 * Arka uçtan gelen hatayı kullanıcıya gösterilecek hâle getirir.
 * Teknik ayrıntı (invoke, ağ katmanı) yerine sade bir cümle döner.
 */
export function errorText(error: unknown, fallback: string): string {
  const raw = error instanceof Error ? error.message : String(error ?? "");
  const message = raw.replace(/^Error:\s*/i, "").trim();

  if (!message) return fallback;
  if (/invoke|__TAURI|undefined|null is not|is not a function|Failed to fetch/i.test(message)) {
    return fallback;
  }

  return message;
}
