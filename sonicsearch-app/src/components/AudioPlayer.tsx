import { convertFileSrc } from "@tauri-apps/api/tauri";
import { onMount } from "solid-js";

/**
 * An Audio Player that plays a file from a given starting timestamp
 */
export function AudioPlayer({
  src,
  startingTimestamp,
}: {
  src: string;
  startingTimestamp: number;
}) {
  let audio: HTMLAudioElement | ((el: HTMLAudioElement) => void) | undefined;
  onMount(() => {
    if (audio instanceof HTMLAudioElement) {
      audio.currentTime = startingTimestamp;
    }
  });

  return (
    <audio controls ref={audio} src={convertFileSrc(src)} preload="metadata" />
  );
}