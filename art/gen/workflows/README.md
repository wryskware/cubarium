# Cubarium ComfyUI workflows

API-format graphs for the Stage 2 art pass, validated and run on the local ComfyUI
(`127.0.0.1:8188`, ComfyUI 0.37.0, RTX 5090). **Qwen-Image 2.1 only** — Wrysk,
2026-09-21: no Flux, Z-Image, HiDream or other checkpoint is worth using here.

Run one with the helper rather than editing a file per render:

    python3 art/gen/tools/comfy_run.py art/gen/workflows/<wf>.json --out DIR/0001.png \
        --set 470.inputs.prompt=@prompt.txt --set 475.inputs.seed=201

`--set <node>.inputs.<key>=<value>`; `=@path` reads the value from a file.

Sampling follows Wrysk's 2026-09-21 notes: **cfg 1, euler** (the negative prompt is
unused at cfg 1 — raise cfg only if you actually want one), size in multiples of 32.

| file | role | model files | defaults |
| --- | --- | --- | --- |
| `qwen21-official-t2i.json` | **sprite default** | UNET `qwen_image_2.1_int8_convrot`, CLIP `qwen3vl_8b_int8_convrot` (type `qwen_image`), VAE `qwen_image_2.1_vae_bf16` | The official ComfyUI template `image_qwen_image_2_1_t2i`, exported unchanged except that the `ResolutionSelector` (node 13) is set to `1:1 (Square)` / 1 MP / multiple 32 = 1024×1024. 25 steps, cfg 1, euler/simple. Node 470 is `TextEncodeQwenImage21` (prompt + negative_prompt + resolution), node 475 the seed, node 461 `SaveImageAdvanced` (png, 8-bit). Change size with `--set 13.inputs.aspect_ratio=…` and `--set 13.inputs.megapixels=…`. |
| `qwen21-t2i-wide.json` | plates | same | Hand-built graph at 1792×768 (56×24 blocks of 32, ~1.4 MP), 40 steps, cfg 1, euler/simple. Node 4 is the text encode, node 5 `EmptySD3LatentImage` (size), node 6 the seed. |
| `qwen-edit-2509-ref.json` | edits, poses, angles, parts | UNET `qwen_image_edit_2509_fp8_e4m3fn` + LoRA `Qwen-Image-Edit-2509-Lightning-4steps-V1.0-bf16` (strength 1.0), CLIP `qwen_2.5_vl_7b_fp8_scaled`, VAE `qwen_image_vae` | 4 steps, cfg 1, euler/simple. Node 5 is `LoadImage`: upload the reference first (`curl -F image=@ref.png -F overwrite=true http://127.0.0.1:8188/upload/image`) and pass its name with `--set 5.inputs.image=<name>`. Node 6 is the edit instruction, node 7 the negative. There is **no** official Qwen-Image 2.1 edit/reference template in the saved library (the only edit template there is `image_flux2_klein_image_edit_9b_distilled.json`, a Flux graph), so this hand-built graph stays the edit path. |

## Transparent sprites

Wrap the subject exactly as Wrysk specified:

> This is an RGBA format image with transparency. `<subject>`. The image has an alpha
> channel and a transparent background.

On `qwen21-official-t2i.json` this yields a genuinely transparent RGBA PNG — the S6
glowcap check at seed 101 came back **747 104 / 1 048 576 pixels with alpha < 10** (71.2 %
cut clean around the log).

**The prompt must not also ask for a background.** The same wrapper with the old S6
sentence "isolated and centred on a plain solid black background" returned RGBA with a
fully opaque alpha (0 pixels under 10, min 134) on both this template and the hand-built
graph. Sprite cards keep "isolated and centred, side view" and let the wrapper own the
background.

No LoRA was downloaded for these graphs.
