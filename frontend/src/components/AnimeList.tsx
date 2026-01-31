import "./AnimeList.css";
import { useQuery } from "@tanstack/react-query";
import { useState } from "react";
import type { Anime, Override } from "../types";
import { useMutation, useQueryClient } from "@tanstack/react-query";

function OverrideForm({
  id,
  title,
  episodeOffset,
  setShowOverride,
}: {
  id: number;
  title: string | null;
  episodeOffset: number | null;
  setShowOverride: React.Dispatch<React.SetStateAction<boolean>>;
}) {
  const queryClient = useQueryClient();

  const { mutate, error, isPending } = useMutation({
    mutationFn: async (override: Override) => {
      const response = await fetch(`/api/anime/${id}/edit`, {
        method: "POST",
        headers: {
          "Content-Type": "application/json",
        },
        body: JSON.stringify(override),
      });
      if (!response.ok) {
        const data = await response.json();
        throw Error(data.error);
      }
      return null;
    },
    onSuccess: () => {
      queryClient.invalidateQueries({ queryKey: ["animelist"] });
      setShowOverride(false);
    },
  });

  function override(formData: FormData) {
    const title = formData.get("title");
    if (typeof title !== "string") {
      throw new Error("Invalid title");
    }
    const episodeOffset = formData.get("episode_offset");
    mutate({
      title: title,
      episode_offset: episodeOffset ? Number(episodeOffset) : null,
    });
  }

  return (
    <form className="overrides" action={override}>
      <input
        name="title"
        type="text"
        placeholder="Title"
        defaultValue={title ?? ""}
      />
      <input
        name="episode_offset"
        type="number"
        placeholder="Episode offset"
        defaultValue={episodeOffset ?? ""}
      />
      <button disabled={isPending}>Save</button>
      {error && <div className="error-text">{error.message}</div>}
    </form>
  );
}

function TitleMatching({ value }: { value: string | null }) {
  if (value)
    return (
      <>
        Matching against <b>{value}</b> in Plex.
      </>
    );
  return <>Using automatic title matching.</>;
}

function EpisodeOffset({ value }: { value: number | null }) {
  if (value)
    return (
      <>
        Episode numbering offset by <b>{value}</b>.
      </>
    );
  return null;
}

function Anime({ anime }: { anime: Anime }) {
  const url = `https://anilist.co/anime/${anime.media_id}/`;

  const [showOverride, setShowOverride] = useState(false);

  return (
    <div className="anime-item">
      <div className="details">
        <div>
          <a href={url} target="_blank">
            {anime.title}
          </a>
          <p className="matching-info">
            <TitleMatching value={anime.title_override} />{" "}
            <EpisodeOffset value={anime.episode_offset} />
          </p>
        </div>
        <button
          className={showOverride ? "secondary" : ""}
          onClick={() => setShowOverride(!showOverride)}
        >
          Set overrides
        </button>
      </div>

      {showOverride && (
        <OverrideForm
          id={anime.id}
          title={anime.title_override}
          episodeOffset={anime.episode_offset}
          setShowOverride={setShowOverride}
        />
      )}
    </div>
  );
}

function AnimeList() {
  const animeQuery = useQuery({
    queryKey: ["animelist"],
    queryFn: async () => {
      const response = await fetch("/api/anime");
      return await response.json();
    },
    staleTime: 15 * 60 * 1000,
  });

  return (
    <div>
      <h2>Matching overrides</h2>
      <p>Set matching overrides for your Anilist watching items.</p>
      <p>
        <b>Title</b>: Set the Plex library title. Fuzzy matching will no longer
        be used for anime with a set title override.
      </p>
      <p>
        <b>Episode offset</b>: Define how much Plex episode numbers should be
        offset to match Anilist. For example, if you wanted to match Plex
        episode 13 to Anilist episode 1, you'd set an offset of -12.
      </p>
      {animeQuery.data?.map((anime: Anime) => (
        <Anime key={anime.id} anime={anime} />
      ))}
    </div>
  );
}

export default AnimeList;
