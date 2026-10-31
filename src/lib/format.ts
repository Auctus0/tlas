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
  0: "Özel Oyun",
  400: "Normal",
  420: "Dereceli Tek/Çift",
  430: "Normal",
  440: "Dereceli Esnek",
  450: "ARAM",
  480: "Tam Gaz",
  490: "Hızlı Oyun",
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

export function formatCompact(value: number): string {
  if (!Number.isFinite(value)) return "—";
  if (Math.abs(value) >= 1_000) {
    return `${(value / 1_000).toFixed(1).replace(".", ",")}B`;
  }
  return `${Math.round(value)}`;
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

export function itemIcon(itemId: number, version: string): string {
  return `https://ddragon.leagueoflegends.com/cdn/${version}/img/item/${itemId}.png`;
}

export function profileIcon(iconId: number, version: string): string {
  return `https://ddragon.leagueoflegends.com/cdn/${version}/img/profileicon/${iconId}.png`;
}

/** Data Dragon şampiyon adını OP.GG'nin beklediği ada çevirir. */
export function championOpggName(name: string): string {
  return name
    .toUpperCase()
    .replace(/['’.]/g, "")
    .replace(/&/g, "")
    .replace(/[^A-Z0-9]+/g, "_")
    .replace(/^_+|_+$/g, "");
}

export function opggLane(position: string | null | undefined): string | null {
  switch (position) {
    case "TOP":
      return "top";
    case "JUNGLE":
      return "jungle";
    case "MIDDLE":
      return "mid";
    case "BOTTOM":
      return "adc";
    case "UTILITY":
      return "support";
    default:
      return null;
  }
}

export function opggChampionUrl(name: string, position?: string | null): string {
  const slug = championOpggName(name).toLowerCase();
  const lane = opggLane(position);
  const suffix = lane ? `/${lane}` : "";

  return `https://www.op.gg/champions/${encodeURIComponent(slug)}/build${suffix}?region=tr`;
}

export function metaTier(tier: number | null): string {
  return tier == null ? "—" : `${tier}. tier`;
}

/** Kaplama gibi dar alanlar için kısa tier etiketi. */
export function metaTierShort(tier: number | null | undefined): string {
  return tier == null ? "—" : `T${tier}`;
}

export function percent(value: number | null | undefined): string {
  if (value == null || !Number.isFinite(value)) return "—";
  return `%${value.toFixed(0)}`;
}

export function lpDelta(delta: number | null): string {
  if (delta == null) return "—";
  if (delta === 0) return "0 LP";
  return `${delta > 0 ? "+" : ""}${delta} LP`;
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
