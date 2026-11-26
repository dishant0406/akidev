// The shapes the API returns.
//
// These mirror apps/server/API.md, which is the contract; nothing here is invented locally.

export type Kind = "anime" | "series" | "movie";

/** One title, flat. Everything a tile, row or banner draws from. */
export interface Media {
  /** The title key, e.g. "anime-16498". */
  id: string;
  kind: Kind;
  title: string;
  poster: string | null;
  banner: string | null;
  description: string | null;
  format: string | null;
  year: number | null;
  episodes: number | null;
  runtime_minutes: number | null;
  /** A provider's own word (FINISHED, RELEASING, Ended, ...), so display text. */
  status: string | null;
  /** 0 to 100. */
  score: number | null;
  popularity: number;
  genres: string[];
  is_adult: boolean;
}

export interface Page {
  results: Media[];
  has_next: boolean;
}

export interface Episode {
  /** Self-describing, e.g. "series-tt0903747-s1e5". */
  id: string;
  season: number;
  number: number;
  name: string | null;
  airdate: string | null;
}

export interface Person {
  name: string;
  role: string | null;
  image: string | null;
}

export interface Trailer {
  site: string;
  id: string;
}

/** A title in full. `episodes` is empty for a film and for an anime, which counts 1..media.episodes. */
export interface Detail {
  media: Media;
  studios: string[];
  trailer: Trailer | null;
  episodes: Episode[];
  cast: Person[];
  related: Media[];
  recommended: Media[];
}

/** One release of an episode or film, from `GET /v1/releases`. Any field the name does not say is null. */
export interface Release {
  /** A hash, or <hash>:<fileIdx> for a selected addon file; sent to /v1/play as `release`. */
  id: string;
  title: string;
  size_bytes: number | null;
/** Null means the provider did not report a valid count; zero is a known empty swarm. */
  seeders: number | null;
  source: string;
  /** All contributing indexers; absent on older servers. */
  sources?: string[];
  /** 480, 720, 1080 or 2160. */
  resolution: number | null;
  codec: "h264" | "hevc" | "av1" | null;
  /** "lossless" is FLAC, DTS, AC-3; "compressed" is AAC, MP3, Opus. */
  audio: "lossless" | "compressed" | null;
  /** A whole season in one torrent. */
  pack: boolean;
  /** A codec that may not decode where it is shown (HEVC, AV1). */
  uncommon: boolean;
  /** Whether the debrid service already holds it; null when there is none or it could not be asked. */
  cached: boolean | null;
}

/** One subtitle file for an episode or film, from `GET /v1/subtitles`. */
export interface SubtitleOption {
  id: string;
  provider: string;
  /** Two letters, or "pt-BR" / "zh-TW" where the region matters. */
  lang: string;
  /** The language's name in English. */
  language: string;
  label: string;
  hearing_impaired: boolean;
  /** The WebVTT file, on the API's origin. */
  url: string;
}

export type PlayResult =
  | {
      available: true;
      url: string;
      /** "hls" when the container needed converting, "file" when the browser plays it directly. */
      stream_type: "file" | "hls";
      release: string | null;
      /** Bare hash or file-scoped selection id: what a watch party names, and what `release` replays. */
      release_id?: string | null;
      source: string | null;
      /** Seconds, when known. */
      duration: number | null;
    }
  | { available: false; reason: string };

export interface HomeRow {
  title: string;
  kind: Kind;
  /** Empty when the row's provider failed; the page leaves such a row out. */
  items: Media[];
}

/** What the apps draw, from GET /v1/screens: the tabs, sorts, genres and home rows. */
export interface Screens {
  tabs: { kind: Kind; slug: string; label: string; tag: string }[];
  sorts: { sort: string; label: string }[];
  /** By kind name, and `all` for the page that shows every catalogue. */
  genres: Record<string, string[]>;
  home: { title: string; kind: Kind; sort: string; genre?: string }[];
  /** The credits: the footer line and the names the privacy page lists. */
  about: { attribution: string; catalogues: string[]; subtitles: string[] };
}

export interface HomeResponse {
  featured: Media[];
  rows: HomeRow[];
}
