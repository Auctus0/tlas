import type { ChampionInfo } from "$lib/types";

/**
 * Rakip şampiyonun Data Dragon etiketlerinden türetilen savunma/karşı önerisi.
 * Kesin counter verisi değildir; şampiyonun hasar tipi ve sınıfına göre
 * deterministik bir başlangıç noktası verir.
 */
export interface CounterPlan {
  items: number[];
  rune: string;
  note: string;
}

const EMPTY: CounterPlan = { items: [], rune: "—", note: "Veri yok" };

export function counterPlan(info: ChampionInfo | null | undefined): CounterPlan {
  if (!info) return EMPTY;

  const tags = info.tags ?? [];
  const attack = info.info?.attack ?? 0;
  const magic = info.info?.magic ?? 0;
  const defense = info.info?.defense ?? 0;

  const isTank = tags.includes("Tank") || defense >= 7;
  const isMarksman = tags.includes("Marksman");
  const isMage = tags.includes("Mage") || magic >= attack + 3;
  const isAssassin = tags.includes("Assassin");
  const isFighter = tags.includes("Fighter");
  const isSupport = tags.includes("Support");

  if (isTank) {
    return {
      items: [3036, 3071, 3047],
      rune: "Cut Down",
      note: "Tank · zırh kır",
    };
  }

  if (isMarksman) {
    return {
      items: [3047, 3143, 3075],
      rune: "Bone Plating",
      note: "Otomatik saldırı",
    };
  }

  if (isAssassin) {
    return magic > attack
      ? {
          items: [3157, 3111, 3156],
          rune: "Bone Plating",
          note: "Büyü patlaması",
        }
      : {
          items: [3026, 3047, 3143],
          rune: "Bone Plating",
          note: "Fiziksel patlama",
        };
  }

  if (isMage) {
    return {
      items: [3111, 3156, 4401],
      rune: "Nullifying Orb",
      note: "Büyü hasarı",
    };
  }

  if (isFighter) {
    return {
      items: [3071, 3047, 3143],
      rune: "Bone Plating",
      note: "Yakın dövüş",
    };
  }

  if (isSupport) {
    return {
      items: [3111, 3047],
      rune: "Second Wind",
      note: "Destek",
    };
  }

  return {
    items: [3071, 3047],
    rune: "Uyarlanabilir",
    note: "Genel",
  };
}
