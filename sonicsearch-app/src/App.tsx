import { createEffect, createSignal } from "solid-js";
import { invoke } from "@tauri-apps/api/tauri";
import { Command } from "@tauri-apps/api/shell";
import { basename } from "@tauri-apps/api/path";
import "./App.css";

type ProcessedSearchResult = {
  fullPath: string;
  basename: string;
};

function App() {
  const [searchResults, setSearchResults] = createSignal<
    ProcessedSearchResult[]
  >([]);
  const [searchString, setSearchString] = createSignal("");
  const [indexIsReady, setIndexIsReady] = createSignal(false);
  const [refreshCount, setRefreshCount] = createSignal(0);
  const [resetCount, setResetCount] = createSignal(0);

  async function updateAudioIndex() {
    const res = await invoke("update_audio_index");
    console.debug(res);
    setIndexIsReady(true);
  }

  createEffect(() => {
    // This is a hack to force the app to refresh the index
    refreshCount();
    if (!indexIsReady()) {
      updateAudioIndex();
    }
  });

  createEffect(() => {
    async function resetAudioIndex() {
      const res = await invoke("reset_audio_index");
      console.debug(res);
      setIndexIsReady(false);
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
    // Learn more about Tauri commands at https://tauri.app/v1/guides/features/command
    const res = await invoke<string[]>("search", {
      searchString: searchString(),
    });
    const processedRes = await Promise.all(
      res.map(
        async (fullPath) =>
          ({
            fullPath,
            basename: await basename(fullPath),
          } satisfies ProcessedSearchResult)
      )
    );
    setSearchResults(processedRes);
  }

  return (
    <div class="container">
      <div class="title">
        <h1>SonicSearch</h1>
        <h2>a search engine for your sounds</h2>
        <button onClick={() => setRefreshCount(refreshCount() + 1)}>
          <p>Refresh Index</p>
        </button>
      </div>

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
        <button type="submit">Search</button>
      </form>

      {searchResults().length > 0 && (
        <div>
          <ul>
            {searchResults().map((searchResult) => (
              <li>
                <a
                  onClick={(e) => {
                    e.preventDefault();
                    new Command("openInFinder", [
                      "-R",
                      searchResult.fullPath,
                    ]).execute();
                  }}
                >
                  {searchResult.basename}
                </a>
              </li>
            ))}
          </ul>
        </div>
      )}
    </div>
  );
}

export default App;
