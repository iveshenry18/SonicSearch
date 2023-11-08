# coding=utf-8
# Copyright 2023 The HuggingFace Inc. team. All rights reserved.
#
# Licensed under the Apache License, Version 2.0 (the "License");
# you may not use this file except in compliance with the License.
# You may obtain a copy of the License at
#
#     http://www.apache.org/licenses/LICENSE-2.0
#
# Unless required by applicable law or agreed to in writing, software
# distributed under the License is distributed on an "AS IS" BASIS,
# WITHOUT WARRANTIES OR CONDITIONS OF ANY KIND, either express or implied.
# See the License for the specific language governing permissions and
# limitations under the License.
"""Feature extractor class for CLAP."""


import copy
from typing import Any, Dict, List, Optional, Union

import numpy as np
import torch

from ...audio_utils import mel_filter_bank, spectrogram, window_function
from ...feature_extraction_sequence_utils import SequenceFeatureExtractor
from ...feature_extraction_utils import BatchFeature
from ...utils import TensorType, logging


logger = logging.get_logger(__name__)


class ClapFeatureExtractor(SequenceFeatureExtractor):
    r"""
    Constructs a CLAP feature extractor.

    This feature extractor inherits from [`~feature_extraction_sequence_utils.SequenceFeatureExtractor`] which contains
    most of the main methods. Users should refer to this superclass for more information regarding those methods.

    This class extracts mel-filter bank features from raw speech using a custom numpy implementation of the *Short Time
    Fourier Transform* (STFT) which should match pytorch's `torch.stft` equivalent.

    Args:
        feature_size (`int`, *optional*, defaults to 64):
            The feature dimension of the extracted Mel spectrograms. This corresponds to the number of mel filters
            (`n_mels`).
        sampling_rate (`int`, *optional*, defaults to 48000):
            The sampling rate at which the audio files should be digitalized expressed in hertz (Hz). This only serves
            to warn users if the audio fed to the feature extractor does not have the same sampling rate.
        hop_length (`int`,*optional*, defaults to 480):
            Length of the overlaping windows for the STFT used to obtain the Mel Spectrogram. The audio will be split
            in smaller `frames` with a step of `hop_length` between each frame.
        max_length_s (`int`, *optional*, defaults to 10):
            The maximum input length of the model in seconds. This is used to pad the audio.
        fft_window_size (`int`, *optional*, defaults to 1024):
            Size of the window (in samples) on which the Fourier transform is applied. This controls the frequency
            resolution of the spectrogram. 400 means that the fourrier transform is computed on windows of 400 samples.
        padding_value (`float`, *optional*, defaults to 0.0):
            Padding value used to pad the audio. Should correspond to silences.
        return_attention_mask (`bool`, *optional*, defaults to `False`):
            Whether or not the model should return the attention masks coresponding to the input.
        frequency_min (`float`, *optional*, defaults to 0):
            The lowest frequency of interest. The STFT will not be computed for values below this.
        frequency_max (`float`, *optional*, defaults to 14000):
            The highest frequency of interest. The STFT will not be computed for values above this.
        top_db (`float`, *optional*):
            The highest decibel value used to convert the mel spectrogram to the log scale. For more details see the
            `audio_utils.power_to_db` function
        truncation (`str`, *optional*, defaults to `"fusion"`):
            Truncation pattern for long audio inputs. Two patterns are available:
                - `fusion` will use `_random_mel_fusion`, which stacks 3 random crops from the mel spectrogram and a
                  downsampled version of the entire mel spectrogram.
            If `config.fusion` is set to True, shorter audios also need to to return 4 mels, which will just be a copy
            of the original mel obtained from the padded audio.
                - `rand_trunc` will select a random crop of the mel spectrogram.
        padding (`str`, *optional*, defaults to `"repeatpad"`):
               Padding pattern for shorter audio inputs. Three patterns were originally implemented:
                - `repeatpad`: the audio is repeated, and then padded to fit the `max_length`.
                - `repeat`: the audio is repeated and then cut to fit the `max_length`
                - `pad`: the audio is padded.
    """

    model_input_names = ["input_features", "is_longer"]

    def __init__(
        self,
        feature_size=64,
        sampling_rate=48_000,
        hop_length=480,
        max_length_s=10,
        fft_window_size=1024,
        padding_value=0.0,
        return_attention_mask=False,  # pad inputs to max length with silence token (zero) and no attention mask
        frequency_min: float = 0,
        frequency_max: float = 14_000,
        top_db: int = None,
        truncation: str = "fusion",
        padding: str = "repeatpad",
        **kwargs,
    ):
        super().__init__(
            feature_size=feature_size,
            sampling_rate=sampling_rate,
            padding_value=padding_value,
            return_attention_mask=return_attention_mask,
            **kwargs,
        )
        self.top_db = top_db
        self.truncation = truncation
        self.padding = padding
        self.fft_window_size = fft_window_size
        self.nb_frequency_bins = (fft_window_size >> 1) + 1
        self.hop_length = hop_length
        self.max_length_s = max_length_s
        self.nb_max_samples = max_length_s * sampling_rate
        self.sampling_rate = sampling_rate
        self.frequency_min = frequency_min
        self.frequency_max = frequency_max
        self.mel_filters = mel_filter_bank(
            num_frequency_bins=self.nb_frequency_bins,
            num_mel_filters=feature_size,
            min_frequency=frequency_min,
            max_frequency=frequency_max,
            sampling_rate=sampling_rate,
            norm=None,
            mel_scale="htk",
        )
        self.mel_filters_slaney = mel_filter_bank(
            num_frequency_bins=self.nb_frequency_bins,
            num_mel_filters=feature_size,
            min_frequency=frequency_min,
            max_frequency=frequency_max,
            sampling_rate=sampling_rate,
            norm="slaney",
            mel_scale="slaney",
        )

    def _np_extract_fbank_features(self, waveform: np.array, mel_filters: Optional[np.array] = None) -> np.ndarray:
        """
        Compute the log-mel spectrogram of the provided `waveform` using the Hann window. In CLAP, two different filter
        banks are used depending on the truncation pattern:
            - `self.mel_filters`: they correspond to the default parameters of `torchaudio` which can be obtained from
              calling `torchaudio.transforms.MelSpectrogram().mel_scale.fb`. These filters are used when `truncation`
              is set to `"fusion"`.
            - `self.mel_filteres_slaney` : they correspond to the default parameters of `librosa` which used
              `librosa.filters.mel` when computing the mel spectrogram. These filters were only used in the original
              implementation when the truncation mode is not `"fusion"`.
        """
        log_mel_spectrogram = spectrogram(
            waveform,
            window_function(self.fft_window_size, "hann"),
            frame_length=self.fft_window_size,
            hop_length=self.hop_length,
            power=2.0,
            mel_filters=mel_filters,
            log_mel="dB",
        )
        return log_mel_spectrogram.T

    def _get_input_mel(self, waveform: np.array, max_length) -> np.array:
        """
        Extracts the mel spectrogram and prepares it for the mode based on the `truncation` and `padding` arguments.
        Four different path are possible:
            - `truncation="fusion"` and the length of the waveform is greater than the max length: the mel spectrogram
              will be computed on the entire audio. 3 random crops and a dowsampled version of the full mel spectrogram
              are then stacked together. They will later be used for `feature_fusion`.
            - `truncation="rand_trunc"` and the length of the waveform is smaller than the max length: the audio is
              padded based on `padding`.
            - `truncation="fusion"` and the length of the waveform is smaller than the max length: the audio is padded
              based on `padding`, and is repeated `4` times.
            - `truncation="rand_trunc"` and the length of the waveform is greater than the max length: the mel
              spectrogram will be computed on a random crop of the waveform.

        """
        

        # only use repeat as a new possible value for padding. you repeat the audio before applying the usual max_length padding
        if waveform.shape[0] < max_length:
            n_repeat = int(max_length / len(waveform))
            waveform = np.stack(np.tile(waveform, n_repeat))
            waveform = np.pad(waveform, (0, max_length - waveform.shape[0]), mode="constant", constant_values=0)

        input_mel = self._np_extract_fbank_features(waveform, self.mel_filters_slaney)[None, :]

        return input_mel

    def __call__(
        self,
        raw_speech: List[np.ndarray],
        sampling_rate: Optional[int] = 48000,
    ) -> BatchFeature:
        """
        Main method to featurize and prepare for the model one or several sequence(s).

        Args:
            raw_speech (`np.ndarray`, `List[float]`, `List[np.ndarray]`, `List[List[float]]`):
                The sequence or batch of sequences to be padded. Each sequence can be a numpy array, a list of float
                values, a list of numpy arrays or a list of list of float values. Must be mono channel audio, not
                stereo, i.e. single float per timestep.
            sampling_rate (`int`, *optional*):
                The sampling rate at which the `raw_speech` input was sampled. It is strongly recommended to pass
                `sampling_rate` at the forward call to prevent silent errors and allow automatic speech recognition
                pipeline.
        """
        truncation = self.truncation # rand_trunc
        padding = self.padding # repeatpad
        max_length = self.max_length_s * self.sampling_rate # 48000
        assert sampling_rate == self.sampling_rate # 48000
        # Assert that raw_speech is a list of waveforms (= np.ndarray of float64)
        assert isinstance(raw_speech, list)
        assert all(isinstance(waveform, np.ndarray) for waveform in raw_speech)
        assert all(waveform.length >= max_length for waveform in raw_speech)

        # convert to mel spectrogram, truncate and pad if needed.
        padded_inputs = [
            self._get_input_mel(waveform, max_length)
            for waveform in raw_speech
        ]

        input_mel = []
        is_longer = []
        for mel in padded_inputs:
            input_mel.append(mel)
            is_longer.append(False)

        if isinstance(input_mel[0], List):
            input_mel = [np.asarray(feature, dtype=np.float64) for feature in input_mel]

        # is_longer is a list of bool
        is_longer = [[longer] for longer in is_longer]

        input_features = {"input_features": input_mel, "is_longer": is_longer}
        input_features = BatchFeature(input_features)

        return input_features
