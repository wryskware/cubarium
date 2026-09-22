# Cubarium ComfyUI workflows

API-format graphs for the Stage 2 art pass, validated and run on the local ComfyUI
(`127.0.0.1:8188`, ComfyUI 0.37.0, RTX 5090). **Qwen-Image 2.1 only** — Wrysk,
2026-09-21: no Flux, Z-Image, HiDream or other checkpoint is worth using here.

Run one with the helper rather than editing a file per render:

    python3 art/gen/tools/comfy_run.py art/gen/workflows/<wf>.json --out DIR/0001.png \
        --set 4.inputs.prompt=@prompt.txt --set 6.inputs.seed=201

`--set <node>.inputs.<key>=<value>`; `=@path` reads the value from a file.

Sampling follows Wrysk's 2026-09-21 notes: **cfg 1, euler, 40 steps** on the official
path. The negative prompt is unused at cfg 1 — raise cfg only if you actually want one
(the 2026-09-21 S6 probe baselines predate the note and ran at 28 steps / cfg 4).
Output size comes from the latent node, `EmptySD3LatentImage` here, and wants multiples
of 32.

| file | model files | defaults |
| --- | --- | --- |
| `qwen21-t2i-1024.json` | UNET `qwen_image_2.1_int8_convrot`, CLIP `qwen3vl_8b_int8_convrot` (type `qwen_image`), VAE `qwen_image_2.1_vae_bf16` | 1024×1024, 40 steps, cfg 1, euler/simple. Node 4 is `TextEncodeQwenImage21` (positive + negative + resolution 1024 in one node); node 5 sets the size; node 6 the seed. |
| `qwen21-t2i-wide.json` | same | 1792×768 (56×24 blocks of 32) — the widest plate size the model took cleanly at ~1.4 MP. Otherwise identical. |
| `qwen-edit-2509-ref.json` | UNET `qwen_image_edit_2509_fp8_e4m3fn` + LoRA `Qwen-Image-Edit-2509-Lightning-4steps-V1.0-bf16` (strength 1.0), CLIP `qwen_2.5_vl_7b_fp8_scaled`, VAE `qwen_image_vae` | 4 steps, cfg 1, euler/simple. Node 5 is `LoadImage`: upload the reference first (`curl -F image=@ref.png -F overwrite=true http://127.0.0.1:8188/upload/image`) and pass its name with `--set 5.inputs.image=<name>`. Node 6 is the edit instruction, node 7 the negative. |

## Transparent sprites

Wrysk's wrapper — `"This is an RGBA format image with transparency. <subject>. The image
has an alpha channel and a transparent background."` — makes `qwen21-t2i-1024.json`
return a genuine **RGBA** PNG, but on this graph the alpha came back effectively opaque
(min 150, no transparent pixels). `EmptyQwenImageLayeredLatentImage` in place of
`EmptySD3LatentImage` fails in the KSampler (`too many values to unpack (expected 4)`),
and this install has no layered decode node. Unresolved; sprites currently need the
black-background + `compose_in_scene.py` alpha cut instead.

No LoRA was downloaded for these graphs.
