# %% [markdown]
# ### Globals

# %%
import datetime

tauri_onnx_models_directory = "../SonicSearch/src-tauri/onnx_models/"
file_timestamp = datetime.datetime.now().strftime("%Y%m%d-%H%M%S")
model_name = "laion/clap-htsat-unfused"

# %% [markdown]
# ### Utilities

# %%
# Utilities
import time

def make_filename_or_dirname(filename, extension=None):
    extension = "" if extension is None else "." + extension.strip('.')
    filename = filename.strip('.').lstrip('/')
    return f'{tauri_onnx_models_directory}{filename}-{file_timestamp}{extension}'

# Inspect inputs and outputs
def get_shapes_in_nests(node, count=0):
    try:
        return str(node.shape)
    except:
        count += 1
        try:
            return ('\n' + '\t'*count).join([f'{key}: {get_shapes_in_nests(value)}' for key, value in node.items()])
        except:
            if isinstance(node, list):
                return ('\n' + '\t'*count).join([get_shapes_in_nests(n) for n in node])
            else:
                return str(node)
        
class QuickTimer():
    """hahaha"""
    _start = 0
    
    def start():
        QuickTimer._start = time.time()
    
    def stop():
        return time.time() - QuickTimer._start

# %% [markdown]
# ## Load and Process Dummy Data

# %%
from transformers import AutoProcessor
from datasets import load_dataset

dataset = load_dataset("ashraq/esc50")
# NOTE: this dataset has a sampling_rate of 44100, whereas the model expects 48000. It works  dummy input, but don't use it for real data.
audio_samples = [data["array"] for data in dataset["train"]["audio"][0:32]]

input_texts = ["The sound of a moderate-length input string", "The sound of a slightly longer input string", "ok", 
              "Now this one is like super super super super suuuuuuuuuuuuuuuuuuuuuuuuuuuuuuuper long!!! and has :) all these characters (f$#%)!"]

# %%

processor = AutoProcessor.from_pretrained(model_name, torchscript=True)
processed_inputs = processor(text=input_texts, audios=audio_samples, return_tensors="pt", padding=True)

# %%
# Inspect Data
import librosa

print("Pre-processed audio: ", get_shapes_in_nests(audio_samples))
librosa.display.waveshow(audio_samples[0], color='b')
# 32 (batch) * 220500 (samples)

print("Pre-processed text: ", get_shapes_in_nests(input_texts))
# 4 (batch) * Variable-length string

print("Processed Model Inputs: ", get_shapes_in_nests(processed_inputs))

print("Processed text: ", get_shapes_in_nests((processed_inputs["input_ids"], processed_inputs["attention_mask"])))
print(processor.tokenizer)
# RobertaTokenizerFast
# input_ids: 4 (batch) * 42 (tokens), "1" is padding.
# attention_mask: 4 (batch) * 42 (tokens), "1" is for real tokens, "0" is for padding.

print("Processed Audio: ", get_shapes_in_nests(processed_inputs["input_features"]))
print(processor.feature_extractor)
# ClapFeatureExtractor - Mel Spectrogram + truncation + padding
# input_features: 32 (batch) * 1 (channel) * 1001 (?) * 64 (?)

# %% [markdown]
# ### Save processor configs

# %%
processor.tokenizer.save_pretrained(make_filename_or_dirname("tokenizer"))
processor.feature_extractor.save_pretrained(make_filename_or_dirname("feature_extractor"))

# %% [markdown]
# ## Full CLAP Model

# %% [markdown]
# ### Load from pretrained

# %%
# Transformers Export
# https://huggingface.co/docs/transformers/v4.27.2/en/model_doc/clap

from transformers import ClapModel
model = ClapModel.from_pretrained(model_name)
model.eval()

# %% [markdown]
# ### Run Full Model

# %%
print("Running model")
print("Inputs: ", get_shapes_in_nests(processed_inputs))
QuickTimer.start()
outputs = model(**processed_inputs)
print(f"Model finished in  {QuickTimer.stop()} seconds")
print("Outputs: ", get_shapes_in_nests(outputs))

# %% [markdown]
# ## Embedders-only (broken)

# # %% [markdown]
# # ## Export Full Model

# # %%
# # Onnx Export - Full Model

# from torch import onnx
# import time

# onnx_inputs = (processed_inputs["input_ids"], processed_inputs["input_features"], False, processed_inputs["attention_mask"])
# onnx_input_names = ["input_ids", "input_features", "is_longer", "attention_mask"]

# onnx_output_names = []

# print("Exporting model to ONNX...")
# start = time.time()
# onnx.export(
#     model,
#     onnx_inputs,
#     make_filename_or_dirname("laion_clap_htsat_unfused", "onnx"),
#     export_params=True,
#     input_names=onnx_input_names,
#     output_names=model(**processed_inputs, return_dict=True).keys(),
    
# )
# print("Exporting model to ONNX took: ", time.time() - start)


