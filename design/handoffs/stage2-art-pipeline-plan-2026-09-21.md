---
design_status: accepted-by-owner-in-conversation
last_reviewed: 2026-09-21
decision_refs: []
---

# Stage 2 art pass: generation → selection → rigging infrastructure (plan)

Proposal for Wrysk, 2026-09-21. Cites `design/art-direction/Cubarium_Art_Direction_v0.1.md`
(the art direction) and `WORKING_POLICY.md` §Art direction: assets are AI-generated and
**selected by Wrysk**; agents generate candidates and make no taste calls. This plan lays
infrastructure only. It does not decide sprite scale, the 2D/3D balance, or the renderer's
sprite layer; those stay open per the art direction.

## What exists today

| Piece | State |
| --- | --- |
| Art direction v0.1 + eight references | Decided (`design/art-direction/`). |
| Local ComfyUI 0.37 on 127.0.0.1:8188 | Running. RTX 5090 (32 GB) + RTX 5080 (16 GB). Models: Flux 1 dev / Krea / Fill / Kontext, Flux 2 dev, Flux 2 Klein 9B, Qwen-Image 2.1, Qwen-Image-Edit 2509 (+ 4-step Lightning LoRA), Z-Image turbo, HiDream. **No pixel-art LoRA, no ControlNet, no upscaler.** |
| ComfyUI MCP (`npx comfyui-mcp`, compact mode) | Registered globally, **disabled for this project**: `claude mcp list` says "re-enable via /mcp". Nothing in this session (or any subagent) can call it until that is flipped. `runpod` MCP is in the same state. |
| Reference skill | `~/.claude/skills/generate-lexomancy-assets/` is the pattern: discover models/workflows, one baseline with recorded seed, retrieve with `list_assets`/`view_image`, one variable per iteration, contact sheets, staged exports, permission gates. |
| Godot atelier (`art/`) | Cutout rigs (`Sprite2D` under named pivots, `AnimationPlayer` clips rest/move/feed/bud, plants stage1/stage2/fruit/grow01/grow12), CPU bake `bake.gd` → `assets/atelier/*.png + pack.json` (v5). 16×16 tiles, one scene unit = one cube pixel. Godot 4.7.2 at `/usr/bin/godot`, `./scripts/art-bake.sh`. |
| Voxel presenter organism art | `crates/cubarium/src/voxel/appearance.rs`: a shared **face-texel glyph atlas** (pigment id + tone per texel, 8 glyphs per part class) consumed by the CPU presenter and the GPU shader. This is the Stage 1 dev look. It is *not* an RGBA sprite path; Stage 2 sprites will need a compositing layer (separate renderer package, after the artbook exists). |
| Provider keys | None in the environment (no OpenAI, Gemini, Replicate or fal keys). Midjourney has no API. So today: ComfyUI = agent lane; Midjourney / GPT-image / Gemini = Wrysk's manual lane. |
| Species to draw | Flora: bloomcrown, umbrellafrond, springturf, stonecushion, velvetpad, glowcap (fungus). Fauna: frondgrazer. Voxel = 0.125 m; panel candidates 4 or 8 px per voxel. |

## Proposed layout

```
art/gen/                         AI generation workshop (git: text + accepted images only)
  PROMPT-KIT.md                  style block (versioned) + provider dialects + subject cards  ← first deliverable
  inbox/                         Wrysk's manual drops (MJ / GPT / Gemini); agent ingests + logs; gitignored
  runs/<date>-<subject>-<nn>/    candidates + run.json (provider, model, seed, prompt, params, verdict, failure)  gitignored
  artbook/                       THE golden reference: accepted images + the style-block version that made them
  workflows/*.json               saved ComfyUI API-format workflows per asset family
  tools/                         compose_in_scene.py (paste candidate onto a voxel capture at N px), pixel_snap.py, palette_quantise.py
design/art-direction/            unchanged; the artbook is referenced from README there once it exists
```

## Phases

**0. Enable and probe** (agent-scoped MCP; sprite-forge, first message).
Confirm the MCP tool list in compact mode, list models/workflows, one baseline render per
Qwen-Image 2.1 mode with the v0 style block (Qwen 2.1 is the only local model; Wrysk,
2026-09-21), save the API workflows, and run one GPT-image call through the Codex lane. Build `compose_in_scene.py` from a fresh `--sink png` capture at 4 and 8 px/voxel
so every candidate is judged in context from the first day (art direction §07).

**1. Style loop, Wrysk in the loop** (the stage you named; repeats per provider).
Input: `PROMPT-KIT.md` v0. Two lanes in parallel: Wrysk explores manually on Midjourney,
GPT-image and Gemini and drops keepers in `inbox/`; the ComfyUI agent explores the same
subject cards locally. Two experiment arms on every provider, because they need different
prompts and cleanup:
- **A. pixel-look direct**: ask for pixel art at 1024², then `pixel_snap` to a true grid.
- **B. clean graphic at 1024², downsample + palette-quantise** to sprite scale.
Each round: one subject, ≤ 8 candidates per provider, a labelled contact sheet, each shown
in-scene at two scales. Wrysk marks keep / iterate / reject and names the largest visible
failure. The kit's style block and each provider's dialect get revised from that, versioned.
Exit: **artbook v0** of roughly 10 accepted images: one habitat plate, two plants at two
growth states, glowcap on a log, frondgrazer in two poses, one terrain/water material swatch,
plus the palette card. From then on the artbook, not the eight references, is the image
reference fed to generators (MJ `--sref`, GPT/Gemini image input, Qwen-Edit / Kontext locally).

**2. The sprite-candidate agent** (persistent worker, Opus, medium; high while authoring the skill).
A project skill `.claude/skills/generate-cubarium-assets/` modelled on the Lexomancy one but
with the Cubarium style block, subject cards, artbook conditioning and the in-scene check
baked in. The worker takes a subject card by message, runs the generate → view → critique
(against the style block and artbook) → iterate loop, and returns a contact sheet plus
`run.json` per candidate. It never promotes anything to `artbook/`; Wrysk does.
Later, with keys, the same worker drives GPT-image and Gemini through their APIs; the
subject cards and log format are provider-neutral now so nothing is rewritten then.

**3. Sprite prep** (scripts in `art/gen/tools/`, Opus low).
Background removal → alpha; palette quantise to the Cubarium family (§05); downsample to
candidate sprite sizes; part separation for rigging, either by asking the edit model
(Qwen-Image-Edit: "the same creature, only the head, same palette, black background") or by
cutting from one accepted image. Output: transparent PNG parts sized in voxel units.

**4. Rigging and sheets** (Godot atelier, reuse).
Animals: cutout rigs exactly as `art/creatures/*.tscn`, new parts, the same clip names.
Plants: whole-sprite growth stages + sway, as `art/plants/`. Bake with `bake.gd` to an RGBA
sheet + `pack.json`. Two things need deciding before the first bake, not now: the tile size
(16 px was the cube; the panel wants larger) and whether the voxel presenter reads the
existing pack format or a new one. Proposal when we get there: keep the pack contract, lift
the tile size to a per-pack field.

**5. Artbook as the golden reference** (ongoing).
`artbook/README.md` lists every accepted image with subject, provider, prompt, seed and the
style-block version. New candidates are judged against it, and it gets pruned when the look
moves, per art direction §07 "preserve a useful reference set".

## Decisions (Wrysk, 2026-09-21)

1. **MCP scoped to the agent, not the project.** `.claude/agents/sprite-forge.md` declares
   the `comfyui` server inline in its `mcpServers` frontmatter (documented: inline servers
   connect only for that subagent; the parent session never sees the tools). The
   project-level server stays disabled. **LoRA downloads for Flux/Qwen are pre-authorised.**
2. **Arms:** unknown winner. Wrysk's read: pixel-look generations look good but their scale
   is off and inconsistent across assets; downsampling is probably necessary for final
   assets. So A explores, and a scale-normalise / downsample step is on the path to every
   final sprite.
3. **Cutout parts, plus angles.** The world is 3D, so each animal needs a mix of angle bases
   (side mirrored, three-quarter toward, three-quarter away) produced from the accepted
   design via the edit lane. Wrysk may supply **full sprite overrides** made from reference
   photos; those bypass generation and go straight to prep.
4. First subject: habitat plate, then glowcap on a log, then frondgrazer. Agreed.
5. Evaluate at 8 and 4 px per voxel, decide scale later. Agreed.

**Providers (Wrysk's cursory exploration):** locally, **Qwen-Image 2.1** is excellent and
has both modes we need (text-to-image, reference-to-image/edit); it is the local primary.
Upstream, **GPT-image is by far the winner**, and it needs no API key: Codex agents carry
an `image_generation` tool (confirmed: `codex features list` shows it stable), so the lane
is `art/gen/tools/gpt_image.sh` running `codex exec -m gpt-5.6-terra` at reasoning high
(a mid-range model unless a prompt needs real intelligence). Wrysk is also setting up an
**Adobe Firefly MCP** with GPT-image 2.5 access; it slots in as a second upstream lane when
it exists, same cards and log format.

## Out of scope for this pass

The renderer's RGBA sprite compositing layer (CPU presenter + GPU shader, one spec, two
implementations), the LED-cube adaptation, terrain re-skinning, and any change to
`design/art-direction/`. These start after artbook v0 exists.
