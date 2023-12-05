import { invoke } from "@tauri-apps/api";
import { basename } from "@tauri-apps/api/path";
import { Command } from "@tauri-apps/api/shell";
import { createSignal } from "solid-js";
import { z } from "zod";
import { AudioPlayer } from "./AudioPlayer";
import { AiFillFolderOpen } from "solid-icons/ai";

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

export function SearchZone() {
  const [searchResults, setSearchResults] = createSignal<
    ProcessedSearchResult[]
  >([]);
  const [isSearching, setIsSearching] = createSignal(false);
  const [searchString, setSearchString] = createSignal("");
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
              <div class="search-result">
                <div class="search-result-left">
                  <div>
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
                  </div>
                  <div>
                    <AudioPlayer
                      src={searchResult.fullPath}
                      startingTimestamp={searchResult.startingTimestamp}
                    />
                  </div>
                </div>
                <div class="search-result-right">
                  <a
                    class="search-result-folder"
                    onClick={(e) => {
                      e.preventDefault();
                      new Command("openInFinder", [
                        "-R",
                        searchResult.fullPath,
                      ]).execute();
                    }}
                  >
                    <AiFillFolderOpen />
                  </a>
                </div>
              </div>
            </li>
          ))}
        </ul>
      )}
    </div>
  );
}
