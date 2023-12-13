import { createMemo } from "solid-js";
import { indexingStatus, isIndexing, updateAudioIndex } from "../App";

const getProgressPercentage = () => {
  const status = indexingStatus();
  if (status === "Started") return 0;
  if (status === "Idle") return 100;
  if (status.InProgress.indexing === null)
    return (
      (status.InProgress.preindexing.preindexed / status.InProgress.total) * 100
    );
  else if (status.InProgress.indexing !== null) {
    return (
      ((status.InProgress.total -
        status.InProgress.indexing.total_to_index +
        status.InProgress.indexing.newly_indexed) /
        status.InProgress.total) *
      100
    );
  }
  return 0;
};

const getProgressColor = () => {
  const status = indexingStatus();
  if (status === "Started") return "rgba(128, 128, 128, 0.5)";
  if (status === "Idle") return "rgba(128, 128, 128, 0.5)";
  if (status.InProgress.indexing === null) return "rgba(128, 128, 128, 0.4)";
  return "rgba(128, 128, 128, 0.5)";
};

const getButtonText = () => {
  const status = indexingStatus();
  if (status === "Idle") return "Refresh Index";
  if (status === "Started" || status.InProgress.indexing === null)
    return "Preparing...";
  if (status.InProgress.indexing !== null) return "Indexing...";
  return "Refresh Index";
};

function trimLeadingZero(str: string) {
  return str[0] === "0" ? str.slice(1) : str;
}

function secondsToString(seconds: number): string | null {
  const SECONDS_IN_10_HOURS = 36000;
  const SECONDS_IN_HOUR = 3600;
  const SECONDS_IN_MINUTE = 60;
  const datestring = new Date(seconds * 1000).toISOString();
  return seconds > SECONDS_IN_10_HOURS
    ? null
    : seconds > SECONDS_IN_HOUR
    ? trimLeadingZero(datestring.slice(11, 13)) + " hours"
    : seconds > SECONDS_IN_MINUTE
    ? trimLeadingZero(datestring.slice(14, 16)) + " minutes"
    : trimLeadingZero(datestring.slice(17, 19)) + " seconds";
}

function getSubtitleText(): string | null {
  const status = indexingStatus();
  if (status === "Idle" || status === "Started") {
    return null;
  } else if (status.InProgress.indexing === null) {
    return "This shouldn't take long";
  } else if (status.InProgress.indexing !== null) {
    const defaultText = "Calculating time remaining...";
    // Avoid overpromising:
    // wait until at least some of the index has been processed to estimate
    if (
      status.InProgress.indexing.newly_indexed <
      status.InProgress.indexing.total_to_index * 0.12
    )
      return defaultText;
    console.debug("Computing estimated time remaining for:", status);
    const { indexing } = status.InProgress;
    const secondsElapsed =
      (Date.now() - Date.parse(indexing.started_indexing)) / 1000;
    if (secondsElapsed === 0) {
      return defaultText;
    }
    const indexedPerSecond = indexing.newly_indexed / secondsElapsed;
    if (indexedPerSecond === 0) {
      return defaultText;
    }
    const estimatedSecondsRemaining =
      (indexing.total_to_index - indexing.newly_indexed) / indexedPerSecond;
    const timeText = secondsToString(estimatedSecondsRemaining);
    return timeText ? `About ${timeText} remaining` : defaultText;
  } else {
    return null;
  }
}

export const IndexingButton = () => {
  const progress = createMemo(getProgressPercentage);
  const buttonText = createMemo(getButtonText);
  const progressColor = createMemo(getProgressColor);

  const subtitleText = createMemo(getSubtitleText);

  return (
    <button
      onClick={updateAudioIndex}
      disabled={isIndexing()}
      class={isIndexing() ? "disabled refresh-button" : "refresh-button"}
      style={{
        "--progress-percentage": `${progress()}%`,
        "--progress-bg-color": progressColor(),
      }}
    >
      <p>{buttonText()}</p>
      {subtitleText() && <p class="small">{subtitleText()}</p>}
    </button>
  );
};
