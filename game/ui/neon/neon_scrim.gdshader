// The neon UI's scrim (design/ui/design-language.md, "Panels"): the world shows through a
// 6 px checker of rgba(7,4,26,.88) and .62 in 3 px squares, counted from the node's corner.
// Put it on a ColorRect (or any Control) over the world; its material is neon_scrim.tres.
shader_type canvas_item;

uniform vec3 tint : source_color = vec3(0.0274510, 0.0156863, 0.1019608); // #07041A
// The squares where the column and row differ in parity (round 3's top-right and bottom-left).
uniform float dark_alpha : hint_range(0.0, 1.0) = 0.88;
uniform float light_alpha : hint_range(0.0, 1.0) = 0.62;
uniform float square_px = 3.0;

varying vec2 local_px;

void vertex() {
	local_px = VERTEX;
}

void fragment() {
	vec2 cell = floor(local_px / max(square_px, 1.0));
	COLOR = vec4(tint, mix(light_alpha, dark_alpha, mod(cell.x + cell.y, 2.0)));
}
