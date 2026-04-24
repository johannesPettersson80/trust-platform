# One Project Live Loop GIF Shot List

Purpose: produce a short top-of-funnel media asset for the README and public
homepage that proves the "One project. Every surface. All live." story without
adding another long marketing paragraph.

Target duration: 25-35 seconds.

Target output:

- `docs/public/assets/images/one-project-live-loop.gif`
- optional poster still:
  `docs/public/assets/images/one-project-live-loop.png`

Interim shipped asset:

- `docs/public/assets/images/one-project-surface-tour.gif` is an animated tour
  generated from existing real product captures. It is useful for the README
  and homepage now, but it is not a replacement for the final live-loop capture
  below.
- Regenerate it with
  `./scripts/captures/generate-one-project-surface-tour.sh`.

## Story

Show one shipped project moving through the integrated loop:

1. VS Code opens the project with ST and runtime panel visible.
2. AI/tooling reads diagnostics or project context.
3. A small ST or HMI descriptor edit is applied.
4. Diagnostics/build state turns clean.
5. Runtime reload or debug action runs.
6. Runtime panel shows live I/O/state.
7. Browser HMI reflects the same project.

## Capture Sequence

| Time | Scene | Evidence |
| --- | --- | --- |
| 0-4s | VS Code project open | ST file, runtime panel, and project tree visible |
| 4-8s | AI/tooling context | diagnostics/tool result or command output showing project-aware context |
| 8-13s | Apply small edit | source or `hmi/` descriptor changes in-place |
| 13-18s | Validate/reload | build/diagnostics status changes to clean, reload/debug action requested |
| 18-25s | Live runtime | runtime panel shows live value/state |
| 25-32s | Browser HMI | `/hmi` shows the same project state from the operator surface |

## Preferred Capture Path

Use existing automated capture infrastructure where possible:

```bash
npm --prefix scripts/captures ci
npm --prefix editors/vscode ci
npm --prefix editors/vscode run compile
./scripts/captures/run-playwright-captures.sh vscode
./scripts/captures/run-playwright-captures.sh browser
```

If the code-server path cannot show the AI/tooling interaction cleanly, use
native desktop VS Code and record the prepared viewport with `ffmpeg`.

## Native Desktop Fallback

Prerequisites observed locally:

- `ffmpeg`
- ImageMagick `magick`
- `code`
- `ydotool`
- `grim`

Example recording shape:

```bash
ffmpeg -f x11grab -framerate 12 -video_size 1280x720 -i "$DISPLAY+0,0" \
  -t 32 -vf "fps=12,scale=1280:-1:flags=lanczos" \
  /tmp/one-project-live-loop.mp4

ffmpeg -i /tmp/one-project-live-loop.mp4 \
  -vf "fps=12,scale=960:-1:flags=lanczos,split[s0][s1];[s0]palettegen[p];[s1][p]paletteuse" \
  docs/public/assets/images/one-project-live-loop.gif
```

Adjust capture geometry for the actual desktop layout before recording.

## Acceptance Checklist

- The clip shows one project, not unrelated windows.
- ST/runtime/HMI surfaces are visibly connected.
- AI/tooling appears as typed/tool-backed assistance, not free-form magic.
- No secrets, tokens, private file paths, or personal browser tabs are visible.
- Text remains readable at README scale.
- The public asset is generated from an internal repeatable capture note or
  script, not a hand-dropped mystery file.
