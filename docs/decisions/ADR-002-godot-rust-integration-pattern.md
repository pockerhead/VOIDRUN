# ADR-002: Godot-Rust Integration Pattern (Hybrid SimulationBridge)

**Дата:** 2025-01-10
**Статус:** ✅ Принято
**Контекст:** Выбор между абстракцией presentation layer vs прямая Godot интеграция

---

## Контекст

После завершения Фазы 1 (Combat Core) встал вопрос: делать ли абстрактный `PresentationClient` trait для изоляции от Godot, или продолжить с прямой интеграцией через `SimulationBridge`?

**Исходные варианты:**
1. **Presentation Layer Abstraction** — trait между simulation и любым движком
2. **Godot-centric** — прямая интеграция, SimulationBridge без абстракций
3. **Hybrid** — минимальная абстракция только где критично

**Ограничения:**
- Single-player priority (не MMORPG пока)
- 5-8 дней до следующего milestone (player control + combat mechanics)
- Godot работает отлично, нет проблем с текущей интеграцией
- Детерминизм уже реализован на уровне Bevy ECS (headless тесты работают)

---

## Решение

**Выбрано:** Вариант 2 (Godot-centric) — продолжить с `SimulationBridge`, **отложить** presentation layer abstraction.

**Архитектура (current):**
```
Bevy ECS Simulation (voidrun_simulation)
  ↓ (direct access)
Godot SimulationBridge (voidrun_godot)
  ↓ (GDExtension)
Godot Engine (визуализация)
```

**Ключевые решения:**
- ✅ `SimulationBridge` остаётся как единственный способ визуализации
- ✅ Прямой доступ к Bevy World/Query из Godot кода
- ✅ Headless тесты работают через direct ECS access (не через trait)
- ⏸️ `PresentationClient` trait отложен до Vertical Slice

---

## Обоснование

### Почему НЕ делать abstraction сейчас:

**1. YAGNI (You Aren't Gonna Need It):**
- Presentation layer trait решает проблему "смены движка" — но мы **не меняем** движок
- Риск смены <5% до 2026 года (Godot 4.3 стабилен, GDExtension API не ломается)
- Моддинг API = проблема 2026+, не сейчас

**2. Фокус на геймплей:**
- 5-8 дней на abstraction = 0 gameplay features
- Player control + combat mechanics критичнее архитектурной чистоты
- "Делай игру, не архитектуру" — core principle

**3. Текущее решение работает:**
- SimulationBridge = правильный pattern (Simulation vs Presentation из godot-rust best practices)
- Rust ECS изолирована (headless тесты без Godot)
- Godot = только presentation (не лезет в game logic)

**4. Research показал:**
- godot-rust рекомендует **именно такой** подход для больших игр
- Примеры (drhaynes/godot-gdext-rust-example) используют прямую интеграцию
- Performance: GDExtension overhead negligible для systems RPG

### Trade-offs (осознанные риски):

**За текущее решение:**
- ✅ Быстрая разработка (нет boilerplate trait impl)
- ✅ Прямой доступ к Godot features (NavigationAgent3D, AnimationTree)
- ✅ Headless тесты всё равно работают (через Bevy App напрямую)
- ✅ 5-8 дней тратим на **игру**

**Против (риски):**
- ⚠️ Tight coupling с Godot — если движок сломается, рефакторинг болезненный
- ⚠️ Моддинг API сложнее (community не может подключить custom рендер)
- ⚠️ Web/mobile render = нужен будет рефакторинг

**Mitigation (как снижаем риски):**
- Держим **всю логику** в Rust simulation (SimulationBridge = только визуал)
- Godot не лезет в game state (читает Bevy components read-only)
- Event-driven архитектура (можно добавить event queue между Bevy и Godot позже)

---

## Влияния

**Архитектура:**
- SimulationBridge остаётся единственным способом визуализации
- voidrun_godot напрямую зависит от voidrun_simulation (no trait)
- Headless тесты продолжают работать через direct ECS access

**Тесты:**
- `cargo test -p voidrun_simulation` работает без Godot (уже так)
- Не нужен `HeadlessPresentationClient` (тесты используют Bevy App)

**Контракты:**
- Нет формального protocol (trait) между simulation и presentation
- API = прямой доступ к Bevy World/Query

**Инструменты:**
- Godot editor продолжает использоваться для ассетов/сцен
- Rust = вся логика + визуализация код (SimulationBridge)

---

## План

**Immediate (сейчас):**
1. ✅ Отметить presentation-layer-abstraction.md как POSTPONED
2. ✅ Обновить roadmap.md — убрать Фазу 1.5 (Presentation Layer)
3. ✅ Начать Фазу 1.5 (Combat Mechanics + Player Control)

**Next milestone (после Vertical Slice):**
- Переоценить решение когда будет playable prototype
- Если появится нужда в моддинг API → вернуться к PresentationClient trait
- Если web/mobile render станет priority → добавить abstraction

**Критерии пересмотра (когда вернуться к вопросу):**
- Community просит custom рендеры (моддинг)
- Web/mobile версия стала приоритетом
- Godot показал критические проблемы (нестабильность, performance)
- После Vertical Slice (когда есть что показать)

---

## Откат

**Если решение не зайдёт:**

План B (добавить abstraction позже):
1. Создать `PresentationEvent` enum (все действия симуляции → визуал)
2. SimulationBridge → GodotPresentationClient (impl trait)
3. Добавить event queue между Bevy и Godot
4. Refactor занимает ~3-5 дней (не критично)

**Сигналы что нужен откат:**
- Godot тормозит разработку (bugs, crashes, API breaking changes)
- Community активно просит моддинг API
- Web/mobile версия стала must-have

**Стоимость отката:** 3-5 дней работы + риск breaking changes в integration коде

---

## Связанные решения

- [ADR-001: Godot vs Bevy для визуализации](ADR-001-godot-vs-bevy.md) — почему выбрали Godot
- [physics-architecture.md](../architecture/physics-architecture.md) — детерминизм на уровне ECS
- [godot-rust-integration.md](../architecture/godot-rust-integration.md) — текущий integration pattern

---

## Research Links

**godot-rust best practices:**
- [Game Architecture — The godot-rust Book](https://godot-rust.github.io/book/gdnative/overview/architecture.html)
  - Pattern 2: Simulation vs Presentation — именно наш случай
  - Рекомендация: "Most of the game is written in Rust, using the engine primarily for input/output handling"

**Примеры в индустрии:**
- [drhaynes/godot-gdext-rust-example](https://github.com/drhaynes/godot-gdext-rust-example) — custom Node в Rust
- godot-rust v0.3 (2025) — hot reload, lifecycle methods (ready/process/physics_process)

**Performance:**
- GDExtension overhead: <1ms для typical game loop
- Критично для real-time симуляций (MMO сервер) — не критично для single-player RPG

---

**Итого:** Продолжаем с Godot + SimulationBridge. Делаем **игру**, не архитектуру. Абстракция подождёт.
