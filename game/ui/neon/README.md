# Neon UI theme

Wrysk's picked look (design/ui/design-language.md, round 3's defaults) as Godot resources, edited in the editor. Fonts come from `scripts/fetch-ui-fonts.sh` (see game/fonts/README.md).

Where to edit what:

| Piece | Resource | Inspector |
|---|---|---|
| Type roles, axes, letter spacing | `fonts/*.tres` (FontVariation) | Variation > OpenType (wght, BACK, RECT, ELSH), Spacing > Glyph, Baseline Offset |
| Sizes and colours per role | `neon_theme.tres` | Theme editor: Label variations (TitleLabel, BigNumber, Kicker, FactLabel, ChromeLabel, BodyText, ListText, SmallText, Keycap...) |
| Main panel: ring, glow, fill | `styles/main_panel.tres` (StyleBoxNeonFrame) | Ring, Glow, Fill Style |
| Dithered bands | `styles/panel_bands.tres` (StyleBoxNeonBands) | Colors, Edges, Cell, Blend |
| Inner boxes, tab bar, search, second button | `styles/inner_box.tres`, `tab_bar.tres`, `search_box.tres`, `secondary_button.tres` | Ring, Fill Color, content margins |
| Selected row, chips | `styles/row_selected.tres`, `chip.tres`, `chip_selected.tres` (StyleBoxNeonDots) | Dot Color, Dot Size, Pitch, Left bar |
| Focus glow (every focusable) | `styles/glow_focus.tres` (StyleBoxNeonFocus) | Ring, Outset, Glow |
| Rules | `styles/fact_rule.tres`, `hot_rule.tres` (StyleBoxNeonDashes) | Color, Dash, Gap, Thickness, sides |
| Scrim | `neon_scrim.tres` (ShaderMaterial on `neon_scrim.gdshader`) | Shader parameters |
| Heights, press drop, disabled opacity | `neon_theme.tres`, Button variations (MainButton, SecondaryButton, Tab, ListRow, Chip) | Constants min_height, press_drop, disabled_opacity |

Scenes: `gallery.tscn` shows every component in every state; `field_guide_mock.tscn` is round 3's Lanternjaw page with static content. Buttons that hold several parts are `NeonButton`s; the labels inside that change when the button is pressed or selected are `NeonLabel`s (look / pressed look).

Hover isn't designed: every hover style is the normal style plus the focus glow (a StyleBoxNeonStack).

Pictures without a window: `scripts/hidden.sh godot --path game --resolution 1920x1080 --script res://tests/neon_shots.gd -- --scene res://ui/neon/gallery.tscn --out /tmp/gallery.png`.
