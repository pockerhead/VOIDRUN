shader_type spatial;
render_mode blend_add, cull_back, depth_draw_opaque, unshaded;

// Uniforms (устанавливаются из Rust via ShaderMaterial)
uniform vec3 shield_color : source_color = vec3(0.3, 0.6, 1.0);  // Синий цвет щита
uniform float energy_percent : hint_range(0.0, 1.0) = 1.0;       // Энергия щита (0.0-1.0)
uniform vec3 last_hit_pos = vec3(0.0, 0.0, 0.0);                // Позиция последнего попадания
uniform float last_hit_time = -999.0;                            // Время последнего попадания

void fragment() {
    // ========================================
    // 1. Fresnel effect (края ярче центра)
    // ========================================
    vec3 view_dir = normalize(VIEW);
    vec3 normal = normalize(NORMAL);
    float fresnel = pow(1.0 - abs(dot(normal, view_dir)), 3.0);

    // ========================================
    // 2. Ripple effect от last_hit_pos
    // ========================================
    vec3 world_pos = (INV_VIEW_MATRIX * vec4(VERTEX, 1.0)).xyz;
    float dist_to_hit = distance(world_pos, last_hit_pos);
    float time_since_hit = TIME - last_hit_time;

    float ripple = 0.0;
    if (time_since_hit < 0.5) {  // 0.5s после попадания
        float wave = sin(dist_to_hit * 10.0 - time_since_hit * 20.0);
        float attenuation = exp(-dist_to_hit * 2.0) * (1.0 - time_since_hit * 2.0);
        ripple = wave * attenuation;
    }

    // ========================================
    // 3. Alpha fade based on energy
    // ========================================
    // При полной энергии (1.0) → base_alpha = 0.3 (слабо виден)
    // При нулевой энергии (0.0) → base_alpha = 0.0 (невидим)
    float base_alpha = fresnel * energy_percent * 0.3;
    float final_alpha = base_alpha + ripple * 0.5;

    // ========================================
    // 4. Color & Emission
    // ========================================
    ALBEDO = shield_color;
    ALPHA = final_alpha;
    EMISSION = shield_color * fresnel * 2.0;  // Яркое свечение на краях
}
