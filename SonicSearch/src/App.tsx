import { createSignal } from "solid-js";
import { invoke } from "@tauri-apps/api/tauri";
import "./App.css";

function App() {
  const [searchResult, setSearchResult] = createSignal("");
  const [searchString, setSearchString] = createSignal("");

  async function search() {
    // Learn more about Tauri commands at https://tauri.app/v1/guides/features/command
    setSearchResult(await invoke("search", { name: searchString() }));
  }

  return (
    <div class="container">
      <div class="title">
        <h1>SonicSearch</h1>
        <h2>a search engine for your sounds</h2>
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

      <p>{searchResult()}</p>
    </div>
  );
}

export default App;
