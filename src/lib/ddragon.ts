import type { ChampionInfo } from "$lib/types";

/**
 * Data Dragon verisi iki pencere tarafından da kullanılır (ana pencere + kaplama).
 * Şampiyonlar ve itemler sürüm bazlı localStorage'da 7 gün tutulur.
 */
export interface DdragonData {
  version: string;
  champions: Record<number, ChampionInfo>;
  items: Record<number, string>;
}

interface ChampionCache {
  at: number;
  version: string;
  map: Record<number, ChampionInfo>;
}

interface ItemCache {
  at: number;
  version: string;
  map: Record<number, string>;
}

const CHAMPION_KEY = "tlas:champions:v2";
const ITEM_KEY = "tlas:items:v1";
const CACHE_TTL = 7 * 86_400_000;

let resolved: DdragonData | null = null;
let inFlight: Promise<DdragonData | null> | null = null;

function readCache<T extends { at: number; version: string }>(key: string): T | null {
  try {
    const raw = localStorage.getItem(key);
    if (!raw) return null;

    const parsed = JSON.parse(raw) as T;
    if (typeof parsed.at !== "number" || Date.now() - parsed.at > CACHE_TTL) {
      return null;
    }

    return parsed;
  } catch {
    return null;
  }
}

async function fetchChampions(): Promise<ChampionCache | null> {
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

    const cache: ChampionCache = { at: Date.now(), version, map };
    localStorage.setItem(CHAMPION_KEY, JSON.stringify(cache));
    return cache;
  } catch {
    return null;
  }
}

async function fetchItems(version: string): Promise<Record<number, string>> {
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

    localStorage.setItem(
      ITEM_KEY,
      JSON.stringify({ at: Date.now(), version, map } satisfies ItemCache),
    );

    return map;
  } catch {
    return {};
  }
}

export async function loadDdragon(): Promise<DdragonData | null> {
  if (resolved) return resolved;
  if (inFlight) return inFlight;

  inFlight = (async () => {
    const champions = readCache<ChampionCache>(CHAMPION_KEY) ?? (await fetchChampions());
    if (!champions) return null;

    const cachedItems = readCache<ItemCache>(ITEM_KEY);
    const items =
      cachedItems && cachedItems.version === champions.version
        ? cachedItems.map
        : await fetchItems(champions.version);

    resolved = { version: champions.version, champions: champions.map, items };
    return resolved;
  })();

  try {
    return await inFlight;
  } finally {
    inFlight = null;
  }
}
