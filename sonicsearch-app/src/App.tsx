import { createSignal, onMount } from "solid-js";
import { invoke } from "@tauri-apps/api/tauri";
import { VsSettingsGear } from "solid-icons/vs";
import "./App.css";
import { SettingsModal } from "./components/SettingsModal";
import { Portal } from "solid-js/web";
import { SearchZone } from "./components/SearchZone";

export const [isIndexing, setIsIndexing] = createSignal(false);
export async function updateAudioIndex() {
  setIsIndexing(true);
  await invoke("update_audio_index");
  setIsIndexing(false);
}

function App() {
  const [settingsModalOpen, setSettingsModalOpen] = createSignal(false);

  onMount(() => {
    // updateAudioIndex();
  });

  return (
    <div class="container">
      <div class="title">
        <h1>SonicSearch</h1>
        <h2>a search engine for your sounds</h2>
      </div>
      <button
        class="settings"
        onClick={() => setSettingsModalOpen((prev) => !prev)}
      >
        <VsSettingsGear class="gear-icon" />
      </button>
      <SearchZone />
      {settingsModalOpen() && (
        <Portal>
          <div
            class="settings-modal-base"
            onClick={() => setSettingsModalOpen(false)}
          >
            <div
              onClick={(e) => {
                e.stopPropagation();
              }}
            >
              <SettingsModal
                onClose={() => {
                  setSettingsModalOpen(false);
                }}
              />
            </div>
          </div>
        </Portal>
      )}
    </div>
  );
}

export default App;
