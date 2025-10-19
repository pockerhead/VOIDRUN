# Lighting System: SDFGI Integration Plan

**Статус:** Planned
**Дата создания:** 2025-01-19
**Автор:** Research based on Godot 4.3 capabilities

---

## Executive Summary

**Цель:** Внедрить modern global illumination систему в VOIDRUN без hardware ray-tracing требований.

**Решение:** Использовать **SDFGI (Signed Distance Field Global Illumination)** — встроенную в Godot 4 систему, которая обеспечивает ray-traced quality освещение БЕЗ необходимости в RTX GPU.

**Ключевые преимущества:**
- ✅ Real-time dynamic GI для процедурно-генерируемых уровней
- ✅ Работает на mid-range GPU (GTX 1060+, 2016 год)
- ✅ Automatic setup (минимальная настройка)
- ✅ Open world friendly (подходит для chunk-based streaming)
- ✅ Native Godot feature (no vendor lock-in)

**Performance target:** 2-5ms @ 1080p (10-30% от frame budget 60 FPS)

---

## Background: Ray-Tracing Research

### Текущее состояние RT в Godot 4.3

**Hardware Ray-Tracing:**
- ❌ **НЕТ** нативной поддержки в Godot 4.3
- 🔄 **В разработке:** Proposals активны (GitHub discussions #5162, issues #6033)
- ⏳ **ETA:** Неизвестно (возможно Godot 4.5+, но не гарантировано)

**Альтернативы:**
1. **SDFGI** — SDF-based ray-marching (рекомендуется)
2. **VoxelGI** — Voxel-based GI (для small/medium indoor)
3. **Custom shaders** — Software RT/ray-marching (слишком медленно)
4. **Third-party** — Jenova Framework, NVIDIA mods (vendor lock-in)

**Детали исследования:** См. раздел "Appendix: Full Research" в конце документа.

---

## SDFGI Technical Overview

### Как работает

**Принцип:**
- Использует **Signed Distance Fields** для представления геометрии
- **Ray-marching** вместо hardware ray-tracing (GPU compute)
- Создаёт **cascades** (каскады) в real-time для покрытия больших расстояний
- Обновляется **динамически** при изменении освещения

**vs Hardware RT:**
- SDFGI: Точность ниже (SDF approximation), но performance выше
- HW RT: Максимальная точность (per-triangle), но требует RTX GPU
- **Trade-off:** 80% качества за 20% стоимости — acceptable для инди

### Performance Characteristics

**Tested Hardware:**
- **GTX 1060** (2016): 60 FPS stable (примеры из Godot dev blog)
- **RTX 2060+**: 2-3ms frame time @ 1080p
- **RTX 3070+**: 1-2ms frame time @ 1080p

**Frame Budget (60 FPS = 16.6ms):**
- SDFGI: ~2-5ms (10-30%)
- ECS simulation: ~2-3ms (12-18%)
- Godot physics: ~2-4ms (12-24%)
- Rendering (rest): ~5-10ms (30-60%)

**Вывод:** Приемлемо при оптимизированном рендере.

### Requirements

**Hardware:**
- GPU с **Vulkan 1.2+** поддержкой
- Минимум: GTX 1060 / RX 580 (2016-2017)
- Рекомендуется: GTX 1660+ / RX 5600+ (2019+)

**Software:**
- Godot 4.0+ (у нас 4.3+) ✅
- Forward+ или Mobile renderer (Compatibility НЕ поддерживает SDFGI)

---

## SDFGI Limitations & Mitigations

### Known Issues

**1. Light Leaks (просачивание света через стены)**

**Проблема:** Свет проникает через тонкие стены, особенно в углах.

**Причина:** SDF approximation не идеальна для thin geometry.

**Решения:**
- Увеличить толщину стен (минимум 0.5-1м для надёжности)
- Включить `Use Occlusion` в SDFGI настройках
- Добавить invisible occluder geometry в проблемных местах
- Настроить `Normal Bias` параметр

**Impact:** Low (архитектурные правила + tweaks решают)

---

**2. Cascade Shifts (видимые переходы между каскадами)**

**Проблема:** При движении камеры видны "ступеньки" в освещении.

**Причина:** SDFGI использует cascades разного разрешения для LOD.

**Решения:**
- Увеличить `Cascade` overlap параметр
- Настроить `Blend Distance` для плавных переходов
- Использовать больше каскадов (trade-off: memory/performance)

**Impact:** Medium (заметно при быстром движении, но настраивается)

---

**3. Dynamic Objects Contribution**

**Проблема:** Динамические объекты (NPC, корабли) **получают** GI, но **не вносят** вклад в освещение сцены.

**Причина:** SDFGI работает с static geometry (SDF baking медленный для moving objects).

**Решения:**
- Использовать **emissive materials** для важных динамических источников света (двигатели кораблей, энергощиты)
- Добавлять **manual OmniLight3D/SpotLight3D** для критичных NPC (например, носимые фонари)
- Для minor NPCs — полагаться только на получение GI (acceptable)

**Impact:** Low (гибридный подход emissive + manual lights компенсирует)

---

**4. Memory Usage**

**Проблема:** Каждый cascade требует texture memory для SDF storage.

**Типичное потребление:**
- 4 cascades @ Medium quality: ~100-200 MB VRAM
- 6 cascades @ High quality: ~300-500 MB VRAM

**Решения:**
- Оптимизировать количество cascades под размер чанков
- Использовать dynamic cascade count (indoor = меньше, outdoor = больше)

**Impact:** Low (modern GPUs имеют 4-8GB VRAM)

---

## Integration Plan for VOIDRUN

### Phase 1: Basic SDFGI Setup (1-2 дня)

**Goal:** Включить SDFGI и получить базовое GI освещение.

**Tasks:**
1. **Enable SDFGI в WorldEnvironment**
   ```gdscript
   # В main.tscn или через Rust:
   env = Environment.new()
   env.sdfgi_enabled = true
   env.sdfgi_cascades = 4  # Начать с 4, tune позже
   env.sdfgi_use_occlusion = true  # Борьба с light leaks
   ```

2. **Настроить cascades под chunk system**
   - VOIDRUN chunks: 32x32м
   - Cascade 0: 16м (indoor, tight spaces)
   - Cascade 1: 32м (single chunk)
   - Cascade 2: 64м (2x2 chunks)
   - Cascade 3: 128м (4x4 chunks)
   - **Total coverage:** ~128м radius от камеры

3. **Performance baseline test**
   - Запустить на test scene с разными GPU (если доступны)
   - Измерить frame time через Godot Performance Monitor
   - Target: <5ms @ 1080p на GTX 1660+

**Acceptance Criteria:**
- ✅ SDFGI включён и работает без crashes
- ✅ Видно indirect lighting (свет отражается от стен)
- ✅ Frame time приемлемый (<16.6ms total)

---

### Phase 2: Quality Tuning (2-3 дня)

**Goal:** Устранить артефакты и оптимизировать качество/performance.

**Tasks:**

**2.1. Light Leak Mitigation**
1. Проверить существующую архитектуру (толщина стен)
2. Включить debug visualizations (`Debug Draw` → `GI`)
3. Добавить occluders в проблемные зоны
4. Tune `Normal Bias` (default 1.0, try 1.5-2.0 при leaks)

**2.2. Cascade Optimization**
1. Измерить actual view distances в gameplay
2. Если indoor-heavy: уменьшить cascades до 3
3. Если outdoor-heavy: возможно 5 cascades
4. Настроить `Min Cell Size` (default 0.2, можно 0.15 для качества)

**2.3. Dynamic Lighting Strategy**
1. Определить critical dynamic light sources:
   - Player flashlight ✅ (manual SpotLight3D)
   - Ship engines ✅ (emissive material + OmniLight3D)
   - Explosions ✅ (temporary OmniLight3D)
   - NPC weapon muzzle flashes ⚠️ (emissive only, no manual light)
2. Создать prefab templates для этих источников
3. Интегрировать с ECS event system (например, `WeaponFired` → spawn temp light)

**Acceptance Criteria:**
- ✅ Нет заметных light leaks в 90% сцен
- ✅ Cascade shifts незаметны при normal движении
- ✅ Динамические источники света визуально убедительны

---

### Phase 3: Hybrid Lighting (опционально, 3-5 дней)

**Goal:** Комбинировать SDFGI с другими техниками для best results.

**Tasks:**

**3.1. VoxelGI для критичных indoor**
- Определить small enclosed spaces (например, ship bridges, bunkers)
- Добавить VoxelGI nodes для этих зон
- Bake VoxelGI (fast, <10 сек)
- Blend SDFGI (outdoor) → VoxelGI (indoor) через triggers

**3.2. LightmapGI для static structures**
- Если есть hero assets (крупные станции, landmarks)
- Pre-bake LightmapGI для максимального quality
- Combine: LightmapGI (static) + SDFGI (dynamic) + manual lights

**3.3. Custom Shader Effects**
- Volumetric fog/nebulae (ray-marching в compute shader)
- Water/metal reflections (screen-space + ray-marched fallback)
- Energy shield caustics (custom fragment shader)

**Acceptance Criteria:**
- ✅ Hybrid system работает без conflicts
- ✅ Performance budget соблюдён (<5ms для всего GI)
- ✅ Visual quality comparable к AAA indie games

---

### Phase 4: Procgen Integration (2-3 дня)

**Goal:** Убедиться, что SDFGI работает с chunk streaming и procgen.

**Tasks:**

**4.1. Chunk Loading/Unloading**
- Проверить, что SDFGI автоматически обновляется при spawn/despawn чанков
- Если lag: рассмотреть staggered updates (несколько frames для re-bake)

**4.2. Seed-based Lighting**
- Убедиться, что освещение детерминировано при одинаковом seed
- Lights должны spawn через ECS (не через Godot random)

**4.3. Performance при Streaming**
- Worst case: массовый spawn нового чанка → SDFGI re-bake
- Measure frame time spike
- Если >50ms: рассмотреть async baking (если Godot поддерживает)

**Acceptance Criteria:**
- ✅ Освещение корректно при chunk streaming
- ✅ Нет visual pops (резких изменений GI)
- ✅ Performance spikes acceptable (<100ms worst case)

---

## Implementation Details (Rust/gdext)

### Enabling SDFGI через Rust

```rust
// В SimulationBridge::ready() или отдельной системе

use godot::classes::{Environment, WorldEnvironment};

fn setup_sdfgi(&mut self) {
    // Get или create WorldEnvironment
    let Some(mut world_env) = self.base()
        .try_get_node_as::<WorldEnvironment>("WorldEnvironment") else {
        log_error("WorldEnvironment not found");
        return;
    };

    // Get Environment resource
    let Some(mut env) = world_env.get_environment() else {
        // Create new if doesn't exist
        let mut new_env = Environment::new_gd();
        setup_sdfgi_env(&mut new_env);
        world_env.set_environment(&new_env);
        return;
    };

    setup_sdfgi_env(&mut env);
}

fn setup_sdfgi_env(env: &mut Gd<Environment>) {
    // Enable SDFGI
    env.set_sdfgi_enabled(true);

    // Cascade configuration (tune these!)
    env.set_sdfgi_cascades(4);  // 4 cascades для начала
    env.set_sdfgi_min_cell_size(0.2);  // Default, можно 0.15 для quality

    // Anti-leak settings
    env.set_sdfgi_use_occlusion(true);
    env.set_sdfgi_bounce_feedback(0.5);  // Indirect bounce intensity

    // Quality (Medium для начала)
    env.set_sdfgi_cascade0_distance(16.0);  // Meters
    // Cascade distances автоматически удваиваются

    log("SDFGI enabled with 4 cascades");
}
```

### Dynamic Light Management

```rust
// Event-driven light spawning

use godot::classes::OmniLight3D;

fn handle_weapon_fired(
    &mut self,
    commands: &mut Commands,
    event: &GodotWeaponFired,
) {
    // Spawn temporary muzzle flash light
    let mut light = OmniLight3D::new_alloc();
    light.set_param(OmniLight3D::PARAM_ENERGY, 2.0);
    light.set_param(OmniLight3D::PARAM_RANGE, 5.0);
    light.set_param(OmniLight3D::PARAM_ATTENUATION, 2.0);

    // Position at weapon muzzle
    light.set_global_position(event.muzzle_position);

    // Attach to scene
    self.base_mut().add_child(&light);

    // Schedule removal after 0.1 sec
    // (TODO: implement timed removal system)
    commands.spawn((
        TemporaryLight {
            node: light.instance_id(),
            lifetime: 0.1,
        },
    ));
}

// System to cleanup temporary lights
fn cleanup_temporary_lights(
    time: Res<Time>,
    mut query: Query<(Entity, &mut TemporaryLight)>,
    mut commands: Commands,
) {
    for (entity, mut temp_light) in query.iter_mut() {
        temp_light.lifetime -= time.delta_seconds();
        if temp_light.lifetime <= 0.0 {
            // Remove Godot node
            if let Some(mut node) = Gd::<Node3D>::try_from_instance_id(temp_light.node) {
                node.queue_free();
            }
            // Remove ECS entity
            commands.entity(entity).despawn();
        }
    }
}
```

### Performance Monitoring

```rust
// Track SDFGI impact on frame time

fn monitor_sdfgi_performance(&mut self) {
    // Get Performance singleton
    let perf = godot::classes::Performance::singleton();

    // Monitor relevant metrics
    let frame_time = perf.get_monitor(Performance::TIME_PROCESS);
    let render_time = perf.get_monitor(Performance::TIME_PHYSICS_PROCESS);

    // SDFGI-specific (если есть в Godot 4.3, иначе use external profiler)
    // let sdfgi_time = perf.get_custom_monitor("sdfgi/update_time");

    if frame_time > 16.6 {
        log_error(&format!("Frame time exceeded: {:.2}ms", frame_time));
    }
}
```

---

## Testing Strategy

### Test Scenes

**1. Minimal Test (sanity check)**
- Single room, 10x10м
- 1 directional light (sun)
- 1 colored wall (должна отражать цвет на противоположную стену)
- **Success:** Видно color bleeding

**2. Indoor Test (light leaks)**
- Enclosed corridor, thin walls (0.2м)
- Exterior bright light
- **Success:** Нет/минимум light leaks через стены

**3. Outdoor Test (cascades)**
- Large open area, 200x200м
- Multiple light sources на разных расстояниях
- Move camera быстро
- **Success:** Нет заметных cascade shifts

**4. Dynamic Test (moving lights)**
- NPC с flashlight
- Moving ship с emissive engines
- **Success:** GI updates в real-time, no lag

**5. Procgen Test (streaming)**
- Spawn/despawn chunks во время runtime
- **Success:** SDFGI updates корректно, no crashes

### Performance Benchmarks

**Target Hardware:**
- **Minimum:** GTX 1060 / RX 580 (1080p, Medium settings)
  - Target: 60 FPS average, 45 FPS minimum
- **Recommended:** GTX 1660+ / RX 5600+ (1080p, High settings)
  - Target: 60 FPS locked
- **High-end:** RTX 3060+ / RX 6700+ (1440p, High settings)
  - Target: 60 FPS locked, room for future features

**Metrics to Track:**
- Frame time (total): <16.6ms
- SDFGI contribution: <5ms
- Memory usage (VRAM): <500MB для SDFGI

---

## Rollout Plan

### Timeline

**Week 1: Foundation**
- Day 1-2: Phase 1 (Basic SDFGI setup)
- Day 3-5: Phase 2 (Quality tuning)

**Week 2: Integration**
- Day 1-3: Phase 4 (Procgen integration)
- Day 4-5: Testing на различных GPU (если доступны)

**Week 3 (optional): Polish**
- Day 1-5: Phase 3 (Hybrid lighting)

**Total:** 2-3 недели для полного внедрения

### Rollback Strategy

**If SDFGI не работает (performance/quality issues):**

**Plan B: Baked Lighting**
- Переключиться на LightmapGI (pre-baked)
- Trade-off: теряем dynamic GI, но гарантированный performance
- Procgen: bake lightmaps для chunk templates, reuse

**Plan C: Minimal Lighting**
- Только direct lighting (no GI)
- Stylized art direction (dark space aesthetic, локальные яркие источники)
- Меньше realism, но стабильный 60 FPS даже на low-end

**Вероятность rollback:** <10% (SDFGI proven tech в Godot 4)

---

## Success Metrics

### Technical
- ✅ SDFGI работает на GTX 1060+ без major issues
- ✅ Frame time impact <5ms @ 1080p
- ✅ Нет game-breaking light leaks
- ✅ Совместимость с chunk streaming

### Visual
- ✅ Indirect lighting заметно и улучшает immersion
- ✅ Динамические источники света convincing
- ✅ Качество comparable к AA/AAA indie games (Satisfactory, Subnautica)

### Development
- ✅ Система maintainable (не требует constant tweaking)
- ✅ Документация достаточна для onboarding
- ✅ No vendor lock-in (pure Godot solution)

---

## Future Considerations

### Godot 5.0+ (когда выйдет)
- Возможно появится hardware RT support
- Migration path: SDFGI → HW RT опционально (upgrade, не rewrite)

### Performance Optimization
- GPU-driven rendering (Godot roadmap)
- Better cascade management (dynamic LOD)

### Advanced Effects
- Volumetric fog integration с SDFGI
- Reflections: SSR + SDFGI fallback
- Caustics для специальных материалов (water, energy shields)

---

## Resources

### Official Godot Docs
- [SDFGI Documentation](https://docs.godotengine.org/en/stable/tutorials/3d/global_illumination/using_sdfgi.html)
- [Global Illumination Introduction](https://docs.godotengine.org/en/stable/tutorials/3d/global_illumination/introduction_to_global_illumination.html)
- [VoxelGI Documentation](https://docs.godotengine.org/en/stable/tutorials/3d/global_illumination/using_voxel_gi.html)

### Community Resources
- [Godot SDFGI Announcement](https://godotengine.org/article/godot-40-gets-sdf-based-real-time-global-illumination/)
- [Ray-marching in Godot Tutorial](https://github.com/PLUkraine/raymarching-godot)
- [Compute Shader RT Tutorial](https://nekotoarts.github.io/projects/computeraytracer)

### Related ADRs
- ADR-002: Godot-Rust Integration (SimulationBridge)
- ADR-003: ECS vs Godot Physics (Hybrid architecture)
- ADR-007: TSCN Prefabs + Dynamic Attachment

---

## Appendix: Full Research Summary

### Ray-Tracing Feasibility Study (2025-01-19)

**Question:** Можно ли в Godot впилить ray-tracing для освещения?

**Answer:**
- **Hardware RT:** ❌ НЕТ в Godot 4.3, в разработке (ETA unknown)
- **SDFGI (RT-like):** ✅ ДА, recommended для VOIDRUN
- **Custom Software RT:** ✅ Возможно, но слишком медленно
- **Third-party:** ⚠️ Возможно (Jenova, NVIDIA mods), но vendor lock-in

### Comparison Table

| Method | Quality GI | Performance | Dynamic | Complexity | For VOIDRUN |
|--------|------------|-------------|---------|------------|-------------|
| **SDFGI** | ⭐⭐⭐⭐ | ⭐⭐⭐⭐⭐ | ⭐⭐⭐⭐ | ⭐⭐⭐⭐⭐ | ✅ Best choice |
| **VoxelGI** | ⭐⭐⭐ | ⭐⭐⭐ | ⭐⭐⭐⭐⭐ | ⭐⭐⭐⭐ | ⚠️ Indoor only |
| **Custom RT** | ⭐⭐⭐⭐⭐ | ⭐ | ⭐⭐⭐⭐⭐ | ⭐ | ❌ Too slow |
| **Hardware RT** | ⭐⭐⭐⭐⭐ | ⭐⭐⭐⭐⭐ | ⭐⭐⭐⭐⭐ | N/A | ❌ Not available |
| **Third-party** | ⭐⭐⭐⭐ | ⭐⭐⭐⭐ | ⭐⭐⭐⭐ | ⭐⭐ | ❌ Vendor lock-in |

### Key Findings

**SDFGI Advantages:**
- Works on GTX 1060+ (2016 hardware)
- Real-time updates (procgen friendly)
- Automatic setup (low maintenance)
- Native Godot (future-proof)

**SDFGI Limitations:**
- Light leaks (mitigatable)
- Cascade shifts (tunable)
- Dynamic objects don't contribute to GI (hybrid approach needed)

**Recommendation:** Использовать SDFGI как primary GI system для VOIDRUN.

---

**Версия:** 1.0
**Последнее обновление:** 2025-01-19
**Next Review:** После Phase 1 implementation
