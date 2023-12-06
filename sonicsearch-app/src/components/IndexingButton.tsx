import { createMemo } from 'solid-js';
import { indexingStatus, isIndexing, updateAudioIndex } from '../App';

const getProgress = () => {
  const status = indexingStatus();
  if (status === "Started") return 0;
  if (status === "Idle") return 100;
  if ("PreIndexing" in status) return (status.PreIndexing.preindexed / status.PreIndexing.total) * 100;
  if ("Indexing" in status) return (status.Indexing.indexed / status.Indexing.total) * 100;
  return 0;
};

const getButtonText = () => {
  const status = indexingStatus();
  if (status === "Idle") return "Refresh Index";
  if (status === "Started" || "PreIndexing" in status) return "Preparing...";
  if ("Indexing" in status) return "Indexing...";
  return "Refresh Index";
};

export const IndexingButton = () => {
  const progress = createMemo(getProgress);
  const buttonText = createMemo(getButtonText);

  return (
    <button
      onClick={updateAudioIndex}
      disabled={isIndexing()}
      class={isIndexing() ? "disabled refresh-button" : "refresh-button"}
      style={{ 'background-image': `linear-gradient(to right, rgba(128, 128, 128, 0.5) ${progress()}%, transparent ${progress()}%)` }}
    >
      <p>{buttonText()}</p>
    </button>
  );
};
