/**
 * Tarayıcı önizlemesi için örnek veri.
 *
 * Yalnızca `npm run dev` + Tauri dışı tarayıcıda kullanılır (bkz. api.ts).
 * Üretim derlemesinde `import.meta.env.DEV` false olduğu için bu dosya pakete
 * girmez; buradaki değerler hiçbir zaman gerçek istemciye gitmez.
 */
import type {
  AccountOverview,
  ChampSelectView,
  LcuStatus,
  LobbyView,
  MatchSummary,
  PlayerCard,
} from "$lib/types";

const now = Date.now();

export const previewStatus: LcuStatus = {
  connected: true,
  phase: "Lobby",
  summonerName: "örnek oyuncu#TR1",
  platform: "TR",
  error: null,
};

export const previewOverview: AccountOverview = {
  summoner: {
    name: "örnek oyuncu#TR1",
    level: 99,
    iconId: 6923,
  },
  ranks: [
    {
      queue: "solo",
      label: "Dereceli Tek/Çift",
      tier: "Altın",
      division: "II",
      lp: 91,
      wins: 41,
      losses: 33,
      winrate: 55.41,
      provisional: false,
    },
    {
      queue: "flex",
      label: "Dereceli Esnek",
      tier: "Altın",
      division: "I",
      lp: 39,
      wins: 26,
      losses: 26,
      winrate: 50,
      provisional: false,
    },
  ],
  lpHistory: [
    { ts: now / 1000 - 3_600, tier: "Altın", division: "II", lp: 91, delta: 21 },
    { ts: now / 1000 - 9_400, tier: "Altın", division: "II", lp: 70, delta: -18 },
    { ts: now / 1000 - 86_400, tier: "Altın", division: "II", lp: 88, delta: 24 },
    { ts: now / 1000 - 90_000, tier: "Altın", division: "II", lp: 64, delta: -16 },
    { ts: now / 1000 - 172_800, tier: "Altın", division: "III", lp: 80, delta: 19 },
    { ts: now / 1000 - 180_000, tier: "Altın", division: "III", lp: 61, delta: null },
  ],
  champions: [
    { championId: 141, games: 20, wins: 13, winrate: 65, kda: 2.59, points: 135_400 },
    { championId: 64, games: 8, wins: 4, winrate: 50, kda: 2.11, points: 96_200 },
    { championId: 412, games: 6, wins: 4, winrate: 66.67, kda: 3.42, points: 61_800 },
    { championId: 103, games: 5, wins: 2, winrate: 40, kda: 2.75, points: 48_050 },
    { championId: 222, games: 4, wins: 3, winrate: 75, kda: 3.08, points: 33_900 },
    { championId: 157, games: 3, wins: 1, winrate: 33.33, kda: 1.64, points: 27_100 },
  ],
  recentGames: 24,
  recentWins: 21,
  recentWinrate: 53,
  recentForm: [
    false, false, true, true, true, false, true, true, false, true, true, true, false, true,
    true, false, true, true, true, false, true, true, true, false,
  ],
  streak: -2,
};

export const previewLobby: LobbyView = {
  queueId: 420,
  queueName: "Dereceli Tek/Çift",
  members: [
    {
      name: "örnek oyuncu#TR1",
      level: 99,
      iconId: 6923,
      tier: "Altın",
      division: "II",
      lp: 91,
      wins: 41,
      losses: 33,
      winrate: 55.41,
      position: "JUNGLE",
      isSelf: true,
    },
    {
      name: "orda kimse yok#TR1",
      level: 214,
      iconId: 4568,
      tier: "Platin",
      division: "IV",
      lp: 12,
      wins: 128,
      losses: 121,
      winrate: 51.41,
      position: "MIDDLE",
      isSelf: false,
    },
    {
      name: "destek main#TR2",
      level: 67,
      iconId: 5212,
      tier: "Gümüş",
      division: "I",
      lp: 74,
      wins: 62,
      losses: 58,
      winrate: 51.67,
      position: "UTILITY",
      isSelf: false,
    },
  ],
};

function card(
  name: string,
  championId: number,
  championName: string,
  position: string,
  tier: string,
  division: string,
  lp: number,
  wins: number,
  losses: number,
  isAlly: boolean,
): PlayerCard {
  return {
    puuid: null,
    summonerId: null,
    displayName: name,
    championId,
    championName,
    assignedPosition: position,
    rank: `${tier} ${division} · ${lp} LP`,
    tier,
    division,
    lp,
    wins,
    losses,
    recentWinrate:
      wins + losses > 0 ? Math.round((wins / (wins + losses)) * 100) : null,
    recentGames: wins + losses,
    isAlly,
    hidden: !isAlly,
  };
}

export const previewChampSelect: ChampSelectView = {
  phase: "ChampSelect",
  allies: [
    card("örnek oyuncu#TR1", 141, "Kayn", "JUNGLE", "Altın", "II", 91, 41, 33, true),
    card("orda kimse yok#TR1", 103, "Ahri", "MIDDLE", "Platin", "IV", 12, 128, 121, true),
    card("destek main#TR2", 412, "Thresh", "UTILITY", "Gümüş", "I", 74, 62, 58, true),
    card("üst taraf#TR1", 86, "Garen", "TOP", "Altın", "III", 44, 77, 74, true),
    card("alt koridor#TR3", 222, "Jinx", "BOTTOM", "Altın", "I", 7, 55, 49, true),
  ],
  enemies: [
    card("Gizli oyuncu", 64, "Lee Sin", "JUNGLE", "", "", 0, 0, 0, false),
    card("Gizli oyuncu", 157, "Yasuo", "MIDDLE", "", "", 0, 0, 0, false),
    card("Gizli oyuncu", 89, "Leona", "UTILITY", "", "", 0, 0, 0, false),
    card("Gizli oyuncu", 122, "Darius", "TOP", "", "", 0, 0, 0, false),
    card("Gizli oyuncu", 202, "Jhin", "BOTTOM", "", "", 0, 0, 0, false),
  ],
  myChampionId: 141,
  timer: 18,
};

const PREVIEW_MATCHES: Array<[string, number, string, boolean, number, number, number, number, number, number]> = [
  ["TR1_1", 141, "Kayn", true, 12, 4, 9, 182, 1_942, 420],
  ["TR1_2", 141, "Kayn", false, 6, 8, 5, 154, 2_108, 420],
  ["TR1_3", 64, "Lee Sin", true, 8, 3, 14, 141, 1_764, 420],
  ["TR1_4", 412, "Thresh", true, 2, 6, 24, 38, 2_320, 420],
  ["TR1_5", 103, "Ahri", false, 9, 7, 6, 201, 2_451, 420],
  ["TR1_6", 222, "Jinx", true, 14, 2, 8, 263, 1_820, 420],
  ["TR1_7", 141, "Kayn", true, 11, 5, 7, 176, 2_015, 440],
  ["TR1_8", 157, "Yasuo", false, 4, 9, 3, 210, 1_688, 420],
  ["TR1_9", 141, "Kayn", true, 15, 3, 11, 190, 2_240, 420],
  ["TR1_10", 412, "Thresh", false, 1, 8, 19, 44, 2_512, 420],
];

export const previewMatches: MatchSummary[] = PREVIEW_MATCHES.map(
  ([matchId, championId, championName, win, kills, deaths, assists, cs, durationSecs, queueId], index) => ({
    matchId,
    championName,
    championId,
    win,
    kills,
    deaths,
    assists,
    cs,
    durationSecs,
    queueId,
    queueName: "Dereceli Tek/Çift",
    gameCreation: now - (index + 1) * 3_600_000,
    role: null,
  }),
);
