export type Anime = {
  id: number;
  media_id: number;
  title: string;
  episode_offset: number | null;
  title_override: string | null;
};

export type Authentication = {
  token: string;
};

export type Override = {
  title: string | null;
  episode_offset: number | null;
};

export type User = {
  id: number;
  name: string;
  expiry: number;
};
