# Neon UI theme

Wrysk's picked look (design/ui/design-language.md, round 3's defaults) as Godot resources, edited in the editor. Fonts come from `scripts/fetch-ui-fonts.sh` (see game/fonts/README.md).

Where to edit what:

| Piece | Resource | Inspector |
|---|---|---|
| Type roles, axes, letter spacing | `fonts/*.tres` (FontVariation) | Variation > OpenType (wght, ELSH; Gridlite's own BACK and RECT stay at 1), Spacing > Glyph, Baseline Offset; Metadata dot_back, dot_rect (the role's BACK and RECT) |
| Gridlite's unlit dots, drawn on whole pixels | `neon_dot_field.gd` (NeonDotField), added to every Gridlite label by the `NeonDots` autoload (`neon_dots.gd`); geometry in `gridlite.gd` | `neon_theme.tres` Label constants dot_light (percent of the font's light, 100), dot_min_gap (px between dots, 0), dot_fill_spacing (fill letter spacing, 0), dot_scaled_keep_pitch (below UI scale 1 everything draws solid unless the role's on-screen pitch is at least this many tenths of a px, 45: the title stays dotted down to about 0.64) |
| Sizes and colours per role | `neon_theme.tres` | Theme editor: Label variations (TitleLabel, BigNumber, Kicker, FactLabel, ChromeLabel, BodyText, ListText, SmallText, Keycap...) |
| Main panel: ring, glow, fill | `styles/main_panel.tres` (StyleBoxNeonFrame) | Ring, Glow, Fill Style |
| Dithered bands | `styles/panel_bands.tres` (StyleBoxNeonBands) | Colors, Edges, Cell, Blend |
| Inner boxes, tab bar, search, second button | `styles/inner_box.tres`, `tab_bar.tres`, `search_box.tres`, `secondary_button.tres` | Ring, Fill Color, content margins |
| Selected row, chips | `styles/row_selected.tres`, `chip.tres`, `chip_selected.tres` (StyleBoxNeonDots) | Dot Color, Dot Size, Pitch, Left bar |
| Focus glow (every focusable) | `styles/glow_focus.tres` (StyleBoxNeonFocus) | Ring, Outset, Glow |
| Rules | `styles/fact_rule.tres`, `hot_rule.tres` (StyleBoxNeonDashes) | Color, Dash, Gap, Thickness, sides |
| Scrim | `neon_scrim.tres` (ShaderMaterial on `neon_scrim.gdshader`) | Shader parameters |
| Heights, press drop, disabled opacity | `neon_theme.tres`, Button variations (MainButton, SecondaryButton, Tab, ListRow, Chip) | Constants min_height, press_drop, disabled_opacity |
| Notices and the paused badge over the world, warning text, the guide's group toggles | `neon_theme.tres`: NoticeBox (the tab bar's box), WarningText (text in warm), GroupToggle (no box, glow focus) | Theme editor |

Scenes: `gallery.tscn` shows every component in every state; `field_guide_mock.tscn` is round 3's Lanternjaw page with static content. The app's screens built from these are `ui/tabbed_page/tabbed_page.tscn` (the tab strip, clock, main panel and key hints; its tabs' contents are `ui/field_guide/field_guide.tscn` and `ui/settings.tscn`, instanced into the panel), `ui/pause_menu.tscn`, `ui/dialog.tscn`, `ui/save_slots.tscn` (rows: `ui/save_slot_row.tscn`), `ui/inspect_panel.tscn`, `ui/notices.tscn` (each: `ui/notice.tscn`), `ui/paused_badge.tscn`, `ui/busy_panel.tscn`, `ui/new_world.tscn` (cards: `ui/terrain_card.tscn`); each shows sample content when opened with no world. Buttons that hold several parts are `NeonButton`s; the labels inside that change when the button is pressed or selected are `NeonLabel`s (look / pressed look).

Hover isn't designed: every hover style is the normal style plus the focus glow (a StyleBoxNeonStack).

Pictures without a window: `scripts/hidden.sh godot --path game --resolution 1920x1080 --script res://tests/neon_shots.gd -- --scene res://ui/neon/gallery.tscn --out /tmp/gallery.png`.

## Settings controls (round 4)

Wrysk's picks (design/ui/design-language.md, "Settings controls (round 4)"): arrows, dot slider, checkbox, dot-row progress. Each is a `@tool` control script in this folder; put the script on a node and every value is in its Inspector (none of them reads the theme). The gallery's second panel shows each in every state; it is 1656 px tall, so take its picture at `--resolution 1920x1656`.

| Control | Script, node type | Signal and API | Inspector |
|---|---|---|---|
| Arrows: choice from a list | `neon_arrows.gd` (NeonArrows), Control | `changed(index)`; `options`, `index` (quiet), `step(±1)` and `choose(i)` (as a viewer, emit) | Options, Index, Wraps (on: right at the last goes to the first), Disabled; Box (style, focus style, height, disabled opacity); Value (font, size, colour, bottom pad); Arrows (colours, pixel, inset, click width); Pips (size, gap, colours, bottom) |
| Dot slider: steps | `neon_dot_slider.gd` (NeonDotSlider), Control | `changed(index)`; `stops`, `index` (quiet), `step`, `choose`; a click or drag snaps to the nearest stop | Stops, Index, Disabled, focus style; Track (top pad, height, dot size, pitch, lit and unlit colours); Handle (size, dot colour, dot, pitch, ring colour and width); Stop names (font, size, colours, uppercase, gap) |
| Checkbox: on/off | `neon_checkbox.gd` (NeonCheckbox), BaseButton in toggle mode | `toggled(on)`; `button_pressed`, `disabled`; A, Enter or a click toggles | Text, focus style, disabled opacity; Layout (height, pads, gap); Box (size, ring, off and on colours, dot plate, tick colour and pixel); Text (font, size, colour) |
| Dot-row progress | `neon_dot_progress.gd` (NeonDotProgress), Control | `value` 0–1; `indeterminate` sweeps a run of dots when there is no end yet | Value, Show Percent, Small, Indeterminate; Dots (large and small size and pitch, done, edge and unlit colours); Percent (font, size, colour, column width, gap); Sweep (dots, speed) |

Disabled is 40 % everywhere. The row fills the control's width, so set the progress's width with custom_minimum_size (the founding row is 516: 420 of dots, 18 gap, 78 for the percentage).
