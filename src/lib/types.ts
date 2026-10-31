export interface Settings {
  autoUpdate: boolean;
  experimentalMeta: boolean;
  overlayEnabled: boolean;
  autoAccept: boolean;
  autoPickChampionId: number | null;
  autoBanChampionId: number | null;
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
  myPosition: string | null;
  timer: number | null;
}

export interface DraftInsight {
  championId: number;
  position: string | null;
  games: number;
  wins: number;
  winrate: number;
  kda: number;
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
  tags?: string[];
  info?: {
    attack: number;
    magic: number;
    defense: number;
    difficulty: number;
  };
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
  session: SessionSummary;
}

export interface SessionSummary {
  games: number;
  wins: number;
  losses: number;
  winrate: number | null;
  avgKda: number | null;
  lpDelta: number | null;
  streak: number;
  since: number | null;
}

export interface MatchPlayer {
  puuid: string | null;
  displayName: string;
  championId: number;
  teamId: number;
  position: string | null;
  kills: number;
  deaths: number;
  assists: number;
  cs: number;
  gold: number;
  damage: number;
  vision: number;
  items: number[];
  isMe: boolean;
}

export interface MatchTeam {
  teamId: number;
  win: boolean;
  kills: number;
  players: MatchPlayer[];
}

export interface MatchDetail {
  matchId: string;
  queueId: number;
  queueName: string | null;
  durationSecs: number;
  gameCreation: number;
  teams: MatchTeam[];
}

export interface BuildItem {
  itemId: number;
  count: number;
  winrate: number;
}

export interface PersonalBuild {
  championId: number;
  games: number;
  wins: number;
  winrate: number;
  kda: number;
  avgCs: number;
  position: string | null;
  coreItems: BuildItem[];
  boots: BuildItem[];
}

export interface LivePlayer {
  displayName: string;
  championId: number;
  position: string | null;
  isAlly: boolean;
  isMe: boolean;
}

export interface LiveGameView {
  gameId: number;
  queueId: number;
  queueName: string | null;
  allies: LivePlayer[];
  enemies: LivePlayer[];
}

export interface MetaReport {
  champion: string;
  position: string;
  tier: number | null;
  winrate: number | null;
  pickrate: number | null;
  banrate: number | null;
  coreItems: string[];
  rune: string | null;
  cached: boolean;
  fetchedAt: number;
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
