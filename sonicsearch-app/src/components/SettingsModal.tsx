import { invoke } from "@tauri-apps/api";
import { listen, TauriEvent, UnlistenFn } from "@tauri-apps/api/event";
import { onMount, onCleanup, createSignal } from "solid-js";
import { AiOutlineClose, AiOutlineDelete } from "solid-icons/ai";
import { isIndexing, setIsIndexing, updateAudioIndex } from "../App";

function getLastPortionOfPath(path: string) {
  const splitPath = path.split("/");
  return splitPath[splitPath.length - 1];
}

export function SettingsModal({ onClose }: { onClose: () => void }) {
  const [fileDropListen, setFileDropListen] = createSignal<UnlistenFn | null>(
    null
  );
  const [fileDropHoverListen, setFileDropHoverListen] =
    createSignal<UnlistenFn | null>(null);
  const [fileDropHoverCancelledListen, setFileDropHoverCancelledListen] =
    createSignal<UnlistenFn | null>(null);
  const [mouseInDropZone, setMouseInDropZone] = createSignal(false);
  const [currentlyIndexedPaths, setCurrentlyIndexedPaths] = createSignal<
    string[]
  >([]);

  const [fileDropHovering, setFileDropHovering] = createSignal<null | string>(
    null
  );

  async function updateCurrentlyIndexedPaths() {
    try {
      const paths = await invoke("get_paths_from_index");
      console.debug(paths);
      setCurrentlyIndexedPaths(paths as string[]);
    } catch (e) {
      console.error(e);
    }
  }

  async function addPathOrPathsToIndex(pathOrPaths: string | string[]) {
    setIsIndexing(true);
    let currentPaths;
    try {
      if (Array.isArray(pathOrPaths)) {
        currentPaths = await invoke<string[]>("add_paths_to_index", {
          paths: pathOrPaths,
        });
      } else {
        currentPaths = await invoke<string[]>("add_path_to_index", {
          path: pathOrPaths,
        });
      }
      setCurrentlyIndexedPaths(currentPaths);
    } catch (e) {
      console.error(e);
    } finally {
      setIsIndexing(false);
    }
  }

  async function deletePathFromIndex(path: string) {
    try {
      const currentPaths = await invoke<string[]>("delete_path_from_index", {
        path,
      });
      setCurrentlyIndexedPaths(currentPaths);
    } catch (e) {
      console.error(e);
    }
  }

  onMount(() => {
    async function registerListeners() {
      console.debug("Registering listeners");
      const fileDropUnlisten = await listen(
        TauriEvent.WINDOW_FILE_DROP,
        (event) => {
          console.log(event);
          if (mouseInDropZone()) {
            if (
              Array.isArray(event.payload) ||
              typeof event.payload === "string"
            ) {
              addPathOrPathsToIndex(event.payload);
            } else {
              console.error("Unexpected payload type", event.payload);
            }
          }
          setFileDropHovering(null);
        }
      );

      const fileDropHoverCancelledUnlisten = await listen(
        TauriEvent.WINDOW_FILE_DROP_CANCELLED,
        (event) => {
          console.log(event);
          setFileDropHovering(null);
        }
      );

      const fileDropHoverUnlisten = await listen(
        TauriEvent.WINDOW_FILE_DROP_HOVER,
        (event) => {
          console.debug(event);
          setFileDropHovering(event.payload as string);
        }
      );

      setFileDropListen(() => fileDropUnlisten);
      setFileDropHoverCancelledListen(() => fileDropHoverCancelledUnlisten);
      setFileDropHoverListen(() => fileDropHoverUnlisten);
      updateCurrentlyIndexedPaths();
    }

    registerListeners();
  });

  onCleanup(() => {
    console.debug("Cleaning up listeners");
    fileDropListen()?.();
    fileDropHoverCancelledListen()?.();
    fileDropHoverListen()?.();
  });
  return (
    <div class="settings-modal">
      <div class="settings-header">
        <button class="close" onClick={onClose}>
          <AiOutlineClose />
        </button>
        <h3>Settings</h3>
      </div>
      <div class="settings-body">
        <div
          class={`file-drop-zone ${
            fileDropHovering() && mouseInDropZone() ? " file-hovering" : ""
          }`}
        >
          <div
            class="file-drop-receiver"
            onDragEnter={() => {
              console.debug("in");
              setMouseInDropZone(true);
            }}
            onDragExit={() => {
              console.debug("out");
              setMouseInDropZone(false);
            }}
            onDragLeave={() => {
              console.debug("leave");
              setMouseInDropZone(false);
            }}
            onDrop={() => {
              console.debug("drop");
              setMouseInDropZone(false);
            }}
          />
          <p>Drop files or folders here to index them</p>
        </div>
        <div class="indexed-paths">
          <ul>
            {currentlyIndexedPaths().map((path) => (
              <li>
                <div class="indexed-path" aria-describedby="path-tooltip">
                  <div role="tooltip" id="path-tooltip">
                    {path}
                  </div>
                  <p>{getLastPortionOfPath(path)}</p>
                  <AiOutlineDelete onClick={() => deletePathFromIndex(path)} />
                </div>
              </li>
            ))}
          </ul>
        </div>
        <button
          onClick={updateAudioIndex}
          disabled={isIndexing()}
          class={isIndexing() ? "disabled refresh-button" : "refresh-button"}
        >
          <p>{isIndexing() ? "Refreshing..." : "Refresh Index"}</p>
        </button>
      </div>
    </div>
  );
}
