import type { ChampionInfo, PlayerCard } from "$lib/types";

/**
 * Taslak kompozisyonu için kaba denge özeti. Data Dragon `tags` ve `info`
 * alanları kullanılır; kesin hasar tipi değil, eksikleri görünür kılmak içindir.
 */
export interface CompProfile {
  known: number;
  ad: number;
  ap: number;
  tank: number;
  frontline: number;
  ranged: number;
  missing: string[];
}

const EMPTY: CompProfile = {
  known: 0,
  ad: 0,
  ap: 0,
  tank: 0,
  frontline: 0,
  ranged: 0,
  missing: [],
};

export function analyseComp(
  players: PlayerCard[],
  champions: Record<number, ChampionInfo>,
): CompProfile {
  let known = 0;
  let ad = 0;
  let ap = 0;
  let tank = 0;
  let frontline = 0;
  let ranged = 0;

  for (const player of players) {
    const info = champions[player.championId];
    const tags = info?.tags ?? [];

    if (!info || tags.length === 0) continue;

    known += 1;

    const attack = info.info?.attack ?? 0;
    const magic = info.info?.magic ?? 0;
    const defense = info.info?.defense ?? 0;

    if (tags.includes("Tank") || defense >= 7) {
      tank += 1;
      frontline += 1;
    } else if (tags.includes("Fighter")) {
      frontline += 1;
    }

    if (tags.includes("Mage")) {
      ap += 1;
    } else if (tags.includes("Marksman")) {
      ad += 1;
    } else if (magic > attack) {
      ap += 1;
    } else {
      ad += 1;
    }

    if (tags.includes("Marksman") || tags.includes("Mage")) {
      ranged += 1;
    }
  }

  if (known === 0) return EMPTY;

  const missing: string[] = [];

  if (known >= 3) {
    if (ad === 0) missing.push("Fiziksel hasar yok");
    else if (ap === 0) missing.push("Büyü hasarı yok");
    else if (ad >= 4 && ap <= 1) missing.push("Hasar fiziksel ağırlıklı");
    else if (ap >= 4 && ad <= 1) missing.push("Hasar büyü ağırlıklı");
  }

  if (frontline === 0) {
    if (known >= 3) missing.push("Ön saf yok");
  } else if (frontline === 1 && known >= 4) {
    missing.push("Ön saf zayıf");
  }

  if (ranged === 0 && known >= 3) missing.push("Menzilli baskı yok");

  return { known, ad, ap, tank, frontline, ranged, missing };
}
