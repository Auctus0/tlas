export interface Settings {
  riotApiKey: string;
  platform: string;
  autoUpdate: boolean;
}

export interface LcuStatus {
  connected: boolean;
  phase: string | null;
  summonerName: string | null;
  platform: string | null;
  error: string | null;
}

export interface PlayerCard {
  puuid: string | null;
  summonerId: number | null;
  displayName: string;
  championId: number;
  championName: string | null;
  assignedPosition: string | null;
  rank: string | null;
  tier: string | null;
  division: string | null;
  lp: number | null;
  wins: number | null;
  losses: number | null;
  recentWinrate: number | null;
  recentGames: number | null;
  isAlly: boolean;
  hidden: boolean;
}

export interface ChampSelectView {
  phase: string;
  allies: PlayerCard[];
  enemies: PlayerCard[];
  myChampionId: number | null;
  timer: number | null;
}

export interface MatchSummary {
  matchId: string;
  championName: string;
  championId: number;
  win: boolean;
  kills: number;
  deaths: number;
  assists: number;
  cs: number;
  durationSecs: number;
  queueId: number;
  gameCreation: number;
  role: string | null;
}

export interface ChampionInfo {
  id: string;
  name: string;
}
