import { createSignal, onMount } from "solid-js";
import { VsSettingsGear } from "solid-icons/vs";
import "./App.css";
import { SettingsModal } from "./components/SettingsModal";
import { Portal } from "solid-js/web";
import { SearchZone } from "./components/SearchZone";
import { Status, commands, events } from "./lib/specta-bindings";
import { appWindow } from "@tauri-apps/api/window";

export const [isInitialized, setIsInitialized] = createSignal(false);
export async function initializeBackend() {
  const res = await commands.initializeBackend();
  if (res.status === "error") {
    console.error(res.error);
  } else {
    setIsInitialized(true);
  }
}

export async function updateAudioIndex() {
  console.debug("Updating audio index");
  events.updateAudioIndex(appWindow).emit();
}

export const [indexingStatus, setIndexingStatus] = createSignal<Status>("Idle");

function registerIndexingStatusListener() {
  events.indexingStatusChanged(appWindow).listen((e) => {
    console.debug("Indexing status changed", e);
    setIndexingStatus(e.payload);
  });
}
export function isIndexing() {
  return indexingStatus() !== "Idle";
}

export const [currentlyIndexedPaths, setCurrentlyIndexedPaths] = createSignal<
  string[]
>([]);
export async function syncCurrentlyIndexedPaths() {
  try {
    const pathsRes = await commands.getPathsFromIndex();
    console.debug(pathsRes);
    if (pathsRes.status === "error") {
      console.error(pathsRes.error);
    } else {
      setCurrentlyIndexedPaths(pathsRes.data);
    }
  } catch (e) {
    console.error(e);
  }
}

function App() {
  const [settingsModalOpen, setSettingsModalOpen] = createSignal(false);

  onMount(() => {
    syncCurrentlyIndexedPaths();
    registerIndexingStatusListener();
    initializeBackend();
  });

  return (
    <div class="container">
      <div class="title">
        <h1>SonicSearch</h1>
        <h2>a search engine for your sounds</h2>
      </div>
      {!isInitialized() ? (
        <div class="splash">
          <h3>Initializing...</h3>
          <p class="small"> (please hold)</p>
        </div>
      ) : (
        <>
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
        </>
      )}
    </div>
  );
}

export default App;
