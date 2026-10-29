export interface Settings {
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
  queueName: string | null;
  gameCreation: number;
  role: string | null;
}

export interface ChampionInfo {
  id: string;
  name: string;
}

export interface RankView {
  queue: string;
  label: string;
  tier: string | null;
  division: string | null;
  lp: number | null;
  wins: number;
  losses: number;
  winrate: number | null;
  provisional: boolean;
}

export interface LpPoint {
  ts: number;
  tier: string;
  division: string | null;
  lp: number;
  delta: number | null;
}

export interface ChampionStat {
  championId: number;
  games: number;
  wins: number;
  winrate: number;
  kda: number;
  points: number | null;
}

export interface AccountOverview {
  summoner: {
    name: string;
    level: number | null;
    iconId: number | null;
  };
  ranks: RankView[];
  lpHistory: LpPoint[];
  champions: ChampionStat[];
  recentGames: number;
  recentWins: number;
  recentWinrate: number | null;
  recentForm: boolean[];
  streak: number;
}

export interface LobbyMember {
  name: string;
  level: number | null;
  iconId: number | null;
  tier: string | null;
  division: string | null;
  lp: number | null;
  wins: number | null;
  losses: number | null;
  winrate: number | null;
  position: string | null;
  isSelf: boolean;
}

export interface LobbyView {
  queueId: number;
  queueName: string | null;
  members: LobbyMember[];
}
