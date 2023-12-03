import { createEffect, createSignal } from "solid-js";
import { invoke } from "@tauri-apps/api/tauri";
import { Command } from "@tauri-apps/api/shell";
import { basename } from "@tauri-apps/api/path";
import "./App.css";
import { z } from "zod";

type ProcessedSearchResult = {
  fullPath: string;
  basename: string;
  startingTimestamp: number;
};

const searchResult = z
  .object({
    file_path: z.string(),
    starting_timestamp: z.number(),
    distance: z.number(),
  })
  .transform((obj) => {
    return {
      fullPath: obj.file_path,
      startingTimestamp: obj.starting_timestamp,
      distance: obj.distance,
    };
  });
const SearchIndexResult = z.array(searchResult);

function secondsToString(seconds: number) {
  const SECONDS_IN_HOUR = 3600;
  const SECONDS_IN_10_MINUTES = 600;
  const isoString = new Date(seconds * 1000).toISOString();
  return seconds > SECONDS_IN_HOUR
    ? isoString.slice(11, 19)
    : seconds > SECONDS_IN_10_MINUTES
    ? isoString.slice(14, 19)
    : isoString.slice(15, 19);
}

function App() {
  const [searchResults, setSearchResults] = createSignal<
    ProcessedSearchResult[]
  >([]);
  const [isSearching, setIsSearching] = createSignal(false);
  const [searchString, setSearchString] = createSignal("");
  const [isIndexing, setIsIndexing] = createSignal(false);
  const [refreshCount, setRefreshCount] = createSignal(0);
  const [resetCount, _setResetCount] = createSignal(0);

  async function updateAudioIndex() {
    setIsIndexing(true);
    await invoke("update_audio_index");
    setIsIndexing(false);
  }

  createEffect(() => {
    // This is a hack to force the app to refresh the index
    refreshCount();
    updateAudioIndex();
  });

  createEffect(() => {
    async function resetAudioIndex() {
      const res = await invoke("reset_audio_index");
      console.debug(res);
    }

    async function resetAndRefreshAudioIndex() {
      await resetAudioIndex();
      updateAudioIndex();
    }

    if (resetCount() > 0) {
      resetAndRefreshAudioIndex();
    }
  });

  async function search() {
    setIsSearching(true);
    const currentSearchString = searchString();
    console.log(`Searching for ${currentSearchString}`);
    const res = await invoke("search_index", {
      searchString: currentSearchString,
    });
    setIsSearching(false);

    console.log(res);
    const parseRes = SearchIndexResult.safeParse(res);
    if (!parseRes.success) {
      console.error(parseRes.error);
      return;
    }
    const parsedRes = parseRes.data;

    const processedRes = await Promise.all(
      parsedRes.map(async (res) => {
        return {
          fullPath: res.fullPath,
          basename: await basename(res.fullPath),
          startingTimestamp: res.startingTimestamp,
        } satisfies ProcessedSearchResult;
      })
    );
    setSearchResults(processedRes);
  }

  return (
    <div class="container">
      <div class="title">
        <h1>SonicSearch</h1>
        <h2>a search engine for your sounds</h2>
      </div>

      <div class="search-zone">
        <form
          class="row"
          onSubmit={(e) => {
            e.preventDefault();
            search();
          }}
        >
          <input
            id="greet-input"
            onChange={(e) => setSearchString(e.currentTarget.value)}
            placeholder="Enter a sound..."
          />
          <button
            type="submit"
            disabled={isSearching()}
            class={isSearching() ? "disabled" : ""}
          >
            {isSearching() ? "Searching..." : "Search"}
          </button>
        </form>

        {searchResults().length > 0 && (
          <ul class="search-results">
            {searchResults().map((searchResult) => (
              <li class="search-result">
                <a
                  class="search-result"
                  onClick={(e) => {
                    e.preventDefault();
                    new Command("openInFinder", [
                      "-R",
                      searchResult.fullPath,
                    ]).execute();
                  }}
                >
                  <p class="search-result">
                    <span class="search-result-basename">
                      {searchResult.basename}
                    </span>
                    <span class="search-result-starting-timestamp">
                      {" (" +
                        secondsToString(searchResult.startingTimestamp) +
                        ")"}
                    </span>
                  </p>
                </a>
              </li>
            ))}
          </ul>
        )}
      </div>
      <button
        onClick={() => setRefreshCount(refreshCount() + 1)}
        disabled={isIndexing()}
        class={isIndexing() ? "disabled refresh-button" : "refresh-button"}
      >
        <p>{isIndexing() ? "Indexing your files..." : "Refresh Index"}</p>
      </button>
    </div>
  );
}

export default App;
