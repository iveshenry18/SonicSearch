import { listen, TauriEvent, UnlistenFn } from "@tauri-apps/api/event";
import { onMount, onCleanup, createSignal } from "solid-js";
import { AiOutlineClose, AiOutlineDelete } from "solid-icons/ai";
import { currentlyIndexedPaths, setCurrentlyIndexedPaths } from "../App";
import { commands } from "../lib/specta-bindings";
import { IndexingButton } from "./IndexingButton";

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

  const [fileDropHovering, setFileDropHovering] = createSignal<null | string>(
    null
  );

  async function addPathsToIndex(paths: string[]) {
    try {
      const currentPathsRes = await commands.addPathsToIndex(paths);
      if (currentPathsRes.status === "error") {
        console.error(currentPathsRes.error);
      } else {
        setCurrentlyIndexedPaths(currentPathsRes.data);
      }
    } catch (e) {
      console.error(e);
    }
  }

  async function deletePathFromIndex(path: string) {
    try {
      const deleteRes = await commands.deletePathFromIndex(path);
      if (deleteRes.status === "error") {
        console.error(deleteRes.error);
      } else {
        setCurrentlyIndexedPaths(deleteRes.data);
      }
    } catch (e) {
      console.error(e);
    }
  }

  onMount(() => {
    async function registerFileDragListeners() {
      console.debug("Registering file drag listeners");
      const fileDropUnlisten = await listen(
        TauriEvent.WINDOW_FILE_DROP,
        (event) => {
          console.log(event);
          if (mouseInDropZone()) {
            if (Array.isArray(event.payload)) {
              addPathsToIndex(event.payload);
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
    }

    registerFileDragListeners();
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
        <IndexingButton />
      </div>
    </div>
  );
}
