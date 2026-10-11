shader_type canvas_item;
uniform vec2 view_size = vec2(1920.0, 1080.0);
uniform float zoom = 1.0;
uniform bool enabled = true;

float bayer(ivec2 p) {
	int values[16] = int[16](0, 8, 2, 10, 12, 4, 14, 6, 3, 11, 1, 9, 15, 7, 13, 5);
	return (float(values[(p.y % 4) * 4 + p.x % 4]) + 0.5) / 16.0;
}
float segment(vec2 p, vec2 a, vec2 b) {
	vec2 d = b - a;
	return length(p - a - d * clamp(dot(p - a, d) / max(dot(d, d), 0.0001), 0.0, 1.0));
}
void fragment() {
	float amount = enabled ? smoothstep(1.0, 2.25, zoom) : 0.0;
	// Scale the 3px dither cells down with small windows; keep the central ellipse clear.
	float cell = max(1.0, floor(min(view_size.x / 1920.0, view_size.y / 1080.0) * 3.0));
	vec2 pixel = floor(UV * view_size / cell) * cell + 0.5 * cell;
	vec2 radial = (pixel / view_size - 0.5) * 2.0;
	float shade = 0.61 * amount * smoothstep(0.55, 1.23, length(radial * sqrt(0.77)));
	float levels = shade * 5.0;
	float alpha = (floor(levels) + step(bayer(ivec2(floor(pixel / cell))), fract(levels))) / 5.0;
	vec3 tint = vec3(13.0, 5.0, 38.0) / 255.0;
	// Mirror a single six-segment notched corner into all four corners.
	vec2 p = min(pixel, view_size - pixel);
	float shorter = min(view_size.x, view_size.y);
	float inset = round(shorter * 0.055 / cell) * cell;
	float arm = round(shorter * 0.068 / cell) * cell;
	p -= vec2(inset);
	float d = segment(p, vec2(arm, 0.0), vec2(3.0 * cell, 0.0));
	d = min(d, segment(p, vec2(3.0 * cell, 0.0), vec2(3.0 * cell, 2.0 * cell)));
	d = min(d, segment(p, vec2(3.0 * cell, 2.0 * cell), vec2(2.0 * cell, 2.0 * cell)));
	d = min(d, segment(p, vec2(2.0 * cell, 2.0 * cell), vec2(2.0 * cell, 3.0 * cell)));
	d = min(d, segment(p, vec2(2.0 * cell, 3.0 * cell), vec2(0.0, 3.0 * cell)));
	d = min(d, segment(p, vec2(0.0, 3.0 * cell), vec2(0.0, arm)));
	float line = (1.0 - smoothstep(0.5 * cell, 0.5 * cell + 0.75, d)) * 0.78 * amount;
	float glow = exp(-d / (2.0 * cell)) * 0.12 * amount;
	float hot = max(line, glow);
	float combined = hot + alpha * (1.0 - hot);
	vec3 colour = (vec3(242.0, 91.0, 219.0) / 255.0 * hot + tint * alpha * (1.0 - hot)) / max(combined, 0.0001);
	COLOR = vec4(colour, combined);
}
