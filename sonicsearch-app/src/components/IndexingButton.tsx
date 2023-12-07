import { createMemo } from "solid-js";
import { indexingStatus, isIndexing, updateAudioIndex } from "../App";

const getProgress = () => {
  const status = indexingStatus();
  if (status === "Started") return 0;
  if (status === "Idle") return 100;
  if (status.InProgress.indexing === null)
    return (
      (status.InProgress.preindexing.preindexed / status.InProgress.total) * 100
    );
  else if (status.InProgress.indexing !== null) {
    return (
      (status.InProgress.indexing.newly_indexed +
        (status.InProgress.total - status.InProgress.indexing.total_to_index) /
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
  return "rgba(128, 128, 128, 0.7)";
};

const getButtonText = () => {
  const status = indexingStatus();
  if (status === "Idle") return "Refresh Index";
  if (status === "Started" || status.InProgress.indexing === null)
    return "Preparing...";
  if (status.InProgress.indexing !== null) return "Indexing...";
  return "Refresh Index";
};

export const IndexingButton = () => {
  const progress = createMemo(getProgress);
  const buttonText = createMemo(getButtonText);
  const progressColor = createMemo(getProgressColor);

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
    </button>
  );
};
