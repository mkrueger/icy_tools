precision highp float;

uniform sampler2D u_render_texture;
uniform sampler2D u_render_data_texture;

uniform vec2      u_render_coordinates;
uniform vec2      u_resolution;
uniform float     u_effect;
uniform vec4      u_buffer_rect;
uniform float     u_time;
uniform vec2      u_scroll_position; // in screen px.

uniform vec3      u_border_color;

uniform vec2      u_raster;
uniform float     u_raster_alpha;
uniform vec3      u_raster_color;

uniform vec2      u_guide;
uniform float     u_guide_alpha;
uniform vec3      u_guide_color;

uniform float     u_show_selection_rectangle;

uniform vec4        u_layer_rectangle;
uniform vec3        u_layer_rectangle_color;
uniform vec4        u_preview_layer_rectangle;
uniform vec3        u_preview_layer_rectangle_color;
uniform vec4        u_selection_rectangle;
uniform vec3        u_selection_fill_color;

uniform float gamma;
uniform float contrast;
uniform float saturation;
uniform float brightness;
uniform float curvature;
uniform float light;
uniform float blur;
uniform float u_scanlines;
uniform float u_use_monochrome;
uniform vec3  u_monchrome_mask;

out vec4 color;

// Shader used: 
// https://www.shadertoy.com/view/XdyGzR
 
vec4 postEffects(in vec3 rgb, in vec2 xy) {
    rgb = pow(rgb, vec3(gamma));
    rgb = mix(vec3(.5), mix(vec3(dot(vec3(.2125, .7154, .0721), rgb * brightness)), rgb * brightness, saturation), contrast);
    return vec4(rgb, 1.0);
}

// Sigma 1. Size 3
vec3 gaussian(in vec2 uv) {
    float b = blur / (u_resolution.x / u_resolution.y);

    uv+= .5;

    vec3 col = texture(u_render_texture, vec2(uv.x - b/u_resolution.x, uv.y - b/u_resolution.y) ).rgb * 0.077847;
    col += texture(u_render_texture, vec2(uv.x - b/u_resolution.x, uv.y) ).rgb * 0.123317;
    col += texture(u_render_texture, vec2(uv.x - b/u_resolution.x, uv.y + b/u_resolution.y) ).rgb * 0.077847;

    col += texture(u_render_texture, vec2(uv.x, uv.y - b/u_resolution.y) ).rgb * 0.123317;
    col += texture(u_render_texture, vec2(uv.x, uv.y) ).rgb * 0.195346;
    col += texture(u_render_texture, vec2(uv.x, uv.y + b/u_resolution.y) ).rgb * 0.123317;

    col += texture(u_render_texture, vec2(uv.x + b/u_resolution.x, uv.y - b/u_resolution.y) ).rgb * 0.077847;
    col += texture(u_render_texture, vec2(uv.x + b/u_resolution.x, uv.y) ).rgb * 0.123317;
    col += texture(u_render_texture, vec2(uv.x + b/u_resolution.x, uv.y + b/u_resolution.y) ).rgb * 0.077847;

    return col;
}

void scanlines2(vec2 coord)
{
	vec2 st = coord - vec2(.5);
    // Curvature/light
    float d = length(st *.5 * st *.5 * curvature);
    vec2 uv = st * d + st;

    // Fudge aspect ratio
#ifdef ASPECT_RATIO
    uv.x *= u_resolution.x/u_resolution.y*.75;
#endif
    
    // CRT color blur
    vec3 col = gaussian(uv);

    // Light
	if (light > 0.0) {
    	float l = 1. - min(1., d * light);
    	col *= l;
	}

    // Scanlines
    float y = uv.y;

    float showScanlines = 1.;
    if (u_resolution.y < 360.) {
		showScanlines = 0.;
	}
    
	float s = 1. - smoothstep(320., 1440., u_resolution.y) + 1.;
	float j = cos(y*u_resolution.y*s)*u_scanlines; // values between .01 to .25 are ok.
	col = abs(showScanlines - 1.)*col + showScanlines * (col - col*j);
	col *= 1. - ( .01 + ceil(mod( (st.x+.5)*u_resolution.x, 3.) ) * (.995-1.01) )*showScanlines;

    // Border mask
	if (curvature > 0.0) {
		float m = max(0.0, 1. - 2. * max(abs(uv.x), abs(uv.y) ));
		m = min(m * 200., 1.);
		col *= m;
	}

    color = postEffects(col, st);
}

void draw_checkers_background() {
	float checker_size = 8.0;
    vec2 p = floor((gl_FragCoord.xy + u_render_coordinates) / checker_size);
    float PatternMask = mod(p.x + mod(p.y, 2.0), 2.0);
	if (PatternMask < 1.0) {
		color = vec4(0.4, 0.4, 0.4, 1.0);
	} else {
		color = vec4(0.6, 0.6, 0.6, 1.0);
	}
}

void draw_dash() {
	float checker_size = 2.0;
    vec2 p = floor((gl_FragCoord.xy + u_render_coordinates) / checker_size);
    float PatternMask = mod(p.x + mod(p.y, 4.0) + u_time, 4.0);
	if (PatternMask < 2.0) {
		color = vec4(1.0);
	} else {
		color = vec4(0.0);
	} 

}

vec4 draw_grid_raster(vec4 c) {
    float PatternMask = mod(gl_FragCoord.x + gl_FragCoord.y, 2.0);
	if (PatternMask == 0.0) {
		return u_raster_alpha * vec4(u_raster_color, 1.0) + (1.0 - u_raster_alpha) * c;
	} else {
		return c;
	} 
}

vec4 draw_guide_raster(vec4 c) {
	float checker_size = 1.0;
    vec2 p = floor((gl_FragCoord.xy + u_render_coordinates) / checker_size);
    float PatternMask = mod(p.x + mod(p.y, 2.0), 2.0);
	if (PatternMask == 0.0) {
		return c;
	} else {
		return u_guide_alpha * vec4(u_guide_color, 1.0) + (1.0 - u_guide_alpha) * c;
	} 
}

void draw_background() {
	color = vec4(u_border_color, 1.0);
}

void selection_border() {
	color = vec4(0.6 * color.rgb, 1.0);
}

void draw_selection_rect(vec2 upper_left, vec2 bottom_right, bool in_buffer_rect) {
	vec2 uv   = gl_FragCoord.xy + vec2(0.5);
	vec2 from = u_buffer_rect.xy;
	vec2 to   = u_buffer_rect.zw ;

	float v = 1.0;

	if (in_buffer_rect) {
	    vec2 uv2   = gl_FragCoord.xy;
		vec2 coord = (uv2 - from) / (to - from);
		vec4 sel = texture(u_render_data_texture, coord);
		if (sel.r == 1.0) {
			vec2 div = (to - from);
			float f = 1.0;

			// check if selection rect is not the edge
			vec4 up = texture(u_render_data_texture, coord - vec2(0.0, f) / div);
			vec4 down = texture(u_render_data_texture, coord + vec2(0.0, f) / div);
			vec4 left = texture(u_render_data_texture, coord - vec2(f, 0.0) / div);
			vec4 right = texture(u_render_data_texture, coord + vec2(f, 0.0) / div);
			if (up.r == 1.0 && down.r == 1.0 && left.r == 1.0 && right.r == 1.0) {
				in_buffer_rect = false;
			}
		}
	}

    if (upper_left.y <= uv.y && uv.y <= bottom_right.y)  {
		// left
		if (upper_left.x == uv.x) {
			if (in_buffer_rect) {
				draw_dash();
			} else {
				color = vec4(1.0);
			}
		} else {
			// inner left border
			if (upper_left.x == uv.x - 1.0 && upper_left.y < uv.y && uv.y < bottom_right.y) {
				selection_border();
			}
		}
		
		// right
		if (bottom_right.x == uv.x) {
			if (in_buffer_rect) {
				draw_dash();
			} else {
				color = vec4(1.0);
			}
		} else {
			// inner left border
			if (bottom_right.x == uv.x + 1.0 && upper_left.y < uv.y && uv.y < bottom_right.y) {
				selection_border();
			}
		}
    }

	if (upper_left.y - 1.0 <= uv.y && uv.y <= bottom_right.y + 1.0)  {
		// outer left border
		if (upper_left.x == uv.x + 1.0) {
			selection_border();
		}	
		// outer left border
		if (bottom_right.x == uv.x - 1.0) {
			selection_border();
		}
	}
	
    if (upper_left.x <= uv.x && uv.x <= bottom_right.x)  {
		// bottom
		if (upper_left.y == uv.y) {
			if (in_buffer_rect) {
				draw_dash();
			} else {
				color = vec4(1.0);
			}
		} else {
			// inner
			if (upper_left.y == uv.y - 1.0 && upper_left.x + 1.0 < uv.x && uv.x < bottom_right.x - 1.0) {
				selection_border();
			}
			// outer
			if (upper_left.y == uv.y + 1.0) {
				selection_border();
			}
		}

		// top
		if (bottom_right.y == uv.y) {
			if (in_buffer_rect) {
				draw_dash();
			} else {
				color = vec4(1.0);
			}
		}  else {
			// inner
			if (bottom_right.y == uv.y + 1.0 && upper_left.x + 1.0 < uv.x && uv.x < bottom_right.x - 1.0) {
				selection_border();
			}
			// outer
			if (bottom_right.y == uv.y - 1.0) {
				selection_border();
			}
		}
    }
}

bool is_inside_selection() {
	vec2 uv   = gl_FragCoord.xy + vec2(0.5);
	vec2 upper_left = u_selection_rectangle.xy;
	vec2 bottom_right = u_selection_rectangle.zw;

	if (upper_left.y <= uv.y && uv.y <= bottom_right.y && 
	    upper_left.x <= uv.x && uv.x <= bottom_right.x)  {
		return true;
	}

	return false;
}

void draw_color_dash(vec3 rect_color) {
	if (is_inside_selection())  {
		draw_dash();
		return;
	}

	float checker_size = 2.0;
    vec2 p = floor((gl_FragCoord.xy + u_render_coordinates) / checker_size);
    float PatternMask = mod(p.x + mod(p.y, 4.0), 4.0);
	if (PatternMask < 2.0) {
		color = vec4(rect_color, 1.0);
	} else {
		color = vec4(0.0);
	} 
}

void draw_layer_rect(vec2 upper_left, vec2 bottom_right, vec3 rect_color) {
	vec2 uv   = gl_FragCoord.xy + vec2(0.5);
	vec2 from = u_buffer_rect.xy;
	vec2 to   = u_buffer_rect.zw ;

	float v = 1.0;
    if (upper_left.y <= uv.y && uv.y <= bottom_right.y)  {
		// left
		if (upper_left.x == uv.x) {
			draw_color_dash(rect_color);
		} 
		
		// right
		if (bottom_right.x == uv.x) {
			draw_color_dash(rect_color);
		}
    }
	
    if (upper_left.x <= uv.x && uv.x <= bottom_right.x)  {
		// bottom
		if (upper_left.y == uv.y) {
			draw_color_dash(rect_color);
		} 

		// top
		if (bottom_right.y == uv.y) {
			draw_color_dash(rect_color);
		}
    }
}

void draw_preview_rect(vec2 upper_left, vec2 bottom_right, vec3 rect_color) {
	vec2 uv   = gl_FragCoord.xy + vec2(0.5);
	vec2 from = u_buffer_rect.xy;
	vec2 to   = u_buffer_rect.zw ;
  
  	vec2 bfrom = u_buffer_rect.xy;
	vec2 bto   = u_buffer_rect.zw ;

	float v = 1.0;
    if (upper_left.y <= uv.y && uv.y <= bottom_right.y)  {
		// left
		if (upper_left.x == uv.x) {
			color = vec4(rect_color, 1.0);
		} else {
			// inner left border
			if (upper_left.x == uv.x - 1.0 && upper_left.y < uv.y && uv.y < bottom_right.y) {
				selection_border();
			}
		}
		
		// right
		if (bottom_right.x == uv.x) {
			color = vec4(rect_color, 1.0);
		} else {
			// inner left border
			if (bottom_right.x == uv.x + 1.0 && upper_left.y < uv.y && uv.y < bottom_right.y) {
				selection_border();
			}
		}
    }

	if (upper_left.y - 1.0 <= uv.y && uv.y <= bottom_right.y + 1.0)  {
		// outer left border
		if (upper_left.x == uv.x + 1.0) {
			selection_border();
		}	
		// outer left border
		if (bottom_right.x == uv.x - 1.0) {
			selection_border();
		}
	}
	
    if (upper_left.x <= uv.x && uv.x <= bottom_right.x)  {
		// bottom
		if (upper_left.y == uv.y) {
			color = vec4(rect_color, 1.0);
		} else {
			// inner
			if (upper_left.y == uv.y - 1.0 && upper_left.x + 1.0 < uv.x && uv.x < bottom_right.x - 1.0) {
				selection_border();
			}
			// outer
			if (upper_left.y == uv.y + 1.0) {
				selection_border();
			}
		}

		// top
		if (bottom_right.y == uv.y) {
			color = vec4(rect_color, 1.0);
		}  else {
			// inner
			if (bottom_right.y == uv.y + 1.0 && upper_left.x + 1.0 < uv.x && uv.x < bottom_right.x - 1.0) {
				selection_border();
			}
			// outer
			if (bottom_right.y == uv.y - 1.0) {
				selection_border();
			}
		}
    }
}

void draw_layer_rectangle(bool in_buffer_rect) {
	if (u_layer_rectangle_color == vec3(0.0)) {
		return;
	}

	draw_layer_rect(u_layer_rectangle.xy, u_layer_rectangle.zw, u_layer_rectangle_color);
	draw_preview_rect(u_preview_layer_rectangle.xy, u_preview_layer_rectangle.zw, u_preview_layer_rectangle_color);
	draw_selection_rect(u_selection_rectangle.xy, u_selection_rectangle.zw, in_buffer_rect);
}

void main() {
	vec2 uv   = gl_FragCoord.xy;
	vec2 from = u_buffer_rect.xy;
	vec2 to   = u_buffer_rect.zw;

	vec2 coord = (uv - from) / (to - from);

	if (from.x <= uv.x && uv.x < to.x && 
		from.y <= uv.y && uv.y < to.y) {
		vec2 buffer_px = floor(vec2(from.x, to.y) - uv + u_scroll_position);

		if (u_effect > 0.9 && u_effect < 1.1) { 
			scanlines2(coord);
		} else { 
			vec4 c = texture(u_render_texture, coord);

			if (c.w < 1.0) {
				draw_checkers_background();
				draw_layer_rectangle(true);
				return;
			}

			if (u_raster.x != 0.0 && mod(buffer_px.x, u_raster.x) == 0.0 ||
			    u_raster.y != 0.0 && mod(buffer_px.y, u_raster.y) == 0.0)  {
				c = draw_grid_raster(c);
			}

			if (u_guide.x > 0.0 && u_guide.y != 0.0) {
				if (-buffer_px.x == u_guide.x && buffer_px.y < u_guide.y ||
					 buffer_px.y == u_guide.y && -buffer_px.x < u_guide.x)  {
					c = draw_guide_raster(c);
				}
			}
			if (is_inside_selection()) {
				color = vec4(0.9 * c.xyz + 0.05 * u_selection_fill_color, 1.0);
			} else {
				color = vec4(c.xyz, 1.0);
			}
		}

		vec4 sel = texture(u_render_data_texture, coord);
		float f = 1.0;
		vec2 div = (to - from);
		vec4 up = texture(u_render_data_texture, coord - vec2(0.0, f) / div);
		vec4 down = texture(u_render_data_texture, coord + vec2(0.0, f) / div);
		vec4 left = texture(u_render_data_texture, coord - vec2(f, 0.0) / div);
		vec4 right = texture(u_render_data_texture, coord + vec2(f, 0.0) / div);

		vec4 left_up = texture(u_render_data_texture, coord + vec2(-f, -f) / div);
		vec4 left_down = texture(u_render_data_texture, coord + vec2(-f, f) / div);
		vec4 right_up = texture(u_render_data_texture, coord + vec2(f, -f) / div);
		vec4 right_down = texture(u_render_data_texture, coord + vec2(f, f) / div);

		if (u_show_selection_rectangle > 0.0) {

			// test outpupt - view selected area
			// color = 0.8 * color + 0.2 * sel.rgb;
			if (sel.r == 1.0) {
				if (up.r == 0.0 || down.r == 0.0 || left.r == 0.0 || right.r == 0.0
				|| left_up.r == 0.0 || left_down.r == 0.0 || right_up.r == 0.0 || right_down.r == 0.0) {
					draw_dash();
				} else {
					color = vec4(0.9 * color.xyz, 1.0);
				}
			} else { 
				// draw outer selection border
				if (up.r == 1.0 || down.r == 1.0 || left.r == 1.0 || right.r == 1.0 
					|| left_up.r == 1.0 || left_down.r == 1.0 || right_up.r == 1.0 || right_down.r == 1.0) {
					selection_border();
				} 
			}

			if (sel.g == 1.0) {
				if (up.g == 0.0 || down.g == 0.0 || left.g == 0.0 || right.g == 0.0
				|| left_up.g == 0.0 || left_down.g == 0.0 || right_up.g == 0.0 || right_down.g == 0.0) {
					color = vec4(vec3(1.0, 1.0, 1.0) - color.xyz, 1.0);
				}
			}
			draw_layer_rectangle(true);

		}

		if (u_use_monochrome > 0.0) {
			float mono = 0.2126 * color.r + 0.7152 * color.g + 0.0722 * color.b;
			color = vec4(mono, mono, mono, 1.0);
			color *= vec4(u_monchrome_mask, 1.0);
		}
	} else {
		draw_background();
		if (u_show_selection_rectangle > 0.0) {
			// correct left & bottom margin for selection
			// It's a hack, but it works.
			from -= vec2(1.0)/ u_resolution;
			draw_layer_rectangle(
				from.x <= uv.x && uv.x < to.x && 
				from.y <= uv.y && uv.y < to.y
			);
		}
	}
}
