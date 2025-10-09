# Architecture Backlog — Открытые вопросы для решения

**Версия:** 1.0
**Обновлено:** 2025-01-07

Этот документ содержит все архитектурные вопросы, которые требуют решения перед или во время соответствующих фаз разработки.

---

## 🔴 Критические (решить перед Фазой 1)

### 1. ECS Component Design

**Вопрос:** Структура базовых компонентов для combat системы?

**Варианты:**
- **A:** Отдельные компоненты (Position, Velocity, Health, Stamina, AttackPower)
- **B:** Сгруппированные (PhysicsBundle, CombatBundle)
- **C:** Использовать Required Components (Bevy 0.16)

**Trade-offs:**
| Вариант | Плюсы | Минусы |
|---------|-------|--------|
| A | Гибкость, простота query | Больше boilerplate при spawn |
| B | Удобно спавнить, меньше забывчивости | Негибко, сложнее query с частью компонентов |
| C | Best of both worlds | Bevy 0.16 feature, может быть unstable |

**Рекомендация:** Вариант C (Required Components) — это best practice Bevy 0.16, уменьшает boilerplate.

**Зависимости:** Нужно решить перед началом Фазы 1 (неделя 1).

---

### 2. Hitbox Hierarchy Architecture

**Вопрос:** Как организовать hitbox'ы для multi-part атак (голова/туловище/конечности)?

**Варианты:**
- **A:** Flat structure — все hitbox'ы как отдельные entities с родительской ссылкой
- **B:** Bevy Hierarchy — Parent/Children компоненты
- **C:** ECS Relationships (Bevy 0.16) — более типобезопасно

**Trade-offs:**
| Вариант | Плюсы | Минусы |
|---------|-------|--------|
| A | Простота, явный контроль | Ручное управление sync позиций |
| B | Автоматический transform propagation | Overhead для простых случаев |
| C | Типобезопасность, современный подход | Новая фича, меньше примеров |

**Рекомендация:** Вариант B для MVP (Фаза 1), миграция на C в Фазе 2 если нужна типобезопасность.

**Зависимости:** Решить на неделе 3 Фазы 1 (когда делаем hitbox систему).

---

### 3. Fixed-Point Arithmetic Strategy

**Вопрос:** Использовать `fixed` crate, `fxp`, или custom implementation для критичной логики?

**Контекст:**
- Rollback netcode требует абсолютного детерминизма
- f32/f64 могут давать разные результаты на разных CPU (даже с FMA disabled)
- Критичные части: позиции, скорости, hitbox расчеты

**Варианты:**
- **A:** `fixed` crate (I24F8 для позиций, I16F16 для скоростей)
- **B:** `fxp` crate (более новый, меньше boilerplate)
- **C:** Custom implementation (полный контроль)
- **D:** Оставить f32/f64 + rigorous тестирование

**Trade-offs:**
| Вариант | Плюсы | Минусы |
|---------|-------|--------|--------|
| A | Зрелый крейт, проверенный | Verbose API, нужно явно конвертить |
| B | Удобный API, макросы | Менее зрелый |
| C | Полный контроль, оптимизации | Много работы, риск багов |
| D | Простота разработки | Риск недетерминизма |

**Рекомендация:** Начать с D (f32/f64) в Фазе 1, мигрировать на A (`fixed` crate) в Фазе 2 при интеграции GGRS.

**Критерий миграции:** Если cross-CPU тесты покажут расхождения > 0.01 units за 1000 тиков.

**Зависимости:** Решение нужно к началу Фазы 2 (rollback netcode).

---

## 🟡 Важные (решить во время Фазы 1-2)

### 4. Event Bus Implementation

**Вопрос:** Использовать Bevy Events или custom pub/sub систему?

**Контекст:**
- События нужны для междоменной связи (Combat → UI, Physics → Audio)
- Replay система требует сериализуемых событий
- Netcode может требовать события в deterministic порядке

**Варианты:**
- **A:** Bevy Events (EventReader/EventWriter)
- **B:** Custom event bus с сериализацией
- **C:** Гибрид: Bevy Events + manual serialization wrapper

**Trade-offs:**
| Вариант | Плюсы | Минусы |
|---------|-------|--------|
| A | Нативная интеграция, zero overhead | Не сериализуемы из коробки |
| B | Полный контроль, replay-ready | Дублирование функциональности Bevy |
| C | Best of both | Boilerplate для каждого события |

**Рекомендация:** Вариант A для Фазы 1, мигрировать на C в Фазе 2 когда понадобится replay.

**Зависимости:** Решить на неделе 2-3 Фазы 1 (когда появятся первые междоменные связи).

---

### 5. AI Decision-Making Architecture

**Вопрос:** FSM, Behavior Trees, Utility AI, или GOAP?

**Контекст:**
- Фаза 1: простой aggro AI (Idle → Approach → Attack)
- Фаза 4: faction AI с долгосрочными решениями (война/мир/торговля)
- Нужна расширяемость без переписывания

**Варианты:**
- **A:** Simple FSM (enum + match)
- **B:** Behavior Trees (`big-brain` crate)
- **C:** Utility AI (scoring функции)
- **D:** GOAP (Goal-Oriented Action Planning)

**Trade-offs:**
| Вариант | Плюсы | Минусы |
|---------|-------|--------|
| A | Простота, понятность | Не масштабируется |
| B | Визуальный дизайн, композиция | Overkill для простого AI |
| C | Emergent behavior, гибкость | Сложнее отлаживать |
| D | Долгосрочное планирование | Overhead, нужен для Фазы 4+ |

**Рекомендация:** Вариант A для Фазы 1, миграция на C (Utility AI) в Фазе 4 для faction AI.

**Зависимости:** Решить на неделе 3 Фазы 1 (simple aggro AI), пересмотреть в Фазе 4.

---

### 6. Snapshot/Restore Strategy

**Вопрос:** `bevy_save`, custom serialization, или clone World?

**Контекст:**
- Rollback netcode требует snapshot каждый тик (64Hz)
- Нужна скорость: snapshot за <1ms для 1000 entities
- Должно быть детерминистично (порядок сериализации важен)

**Варианты:**
- **A:** `bevy_save` crate (автоматическая сериализация World)
- **B:** Custom: собираем только rollback-critical компоненты
- **C:** `World::clone()` (если Bevy поддерживает)

**Trade-offs:**
| Вариант | Плюсы | Минусы |
|---------|-------|--------|
| A | Автоматизация, меньше кода | Может быть медленно, сериализует все |
| B | Максимальная скорость | Ручное обслуживание списка компонентов |
| C | Простейший код | Может не быть в Bevy, медленно |

**Рекомендация:** Вариант B (custom) — контроль над performance критично для 64Hz.

**Зависимости:** Решить к началу Фазы 2 (rollback интеграция).

---

## 🟢 Желательные (решить в Фазе 3+)

### 7. Content Pipeline Architecture

**Вопрос:** YAML, RON, или JSON для data-driven контента?

**Контекст:**
- Items, NPC archetypes, ships, weapons — сотни определений
- Нужна валидация (schema)
- Hot-reload для итераций
- Моддинг в будущем

**Варианты:**
- **A:** YAML (человекочитаемый, комментарии)
- **B:** RON (Rust Object Notation, нативная типизация)
- **C:** JSON (стандарт, много тулинга)
- **D:** Гибрид (JSON Schema + YAML/RON файлы)

**Trade-offs:**
| Вариант | Плюсы | Минусы |
|---------|-------|--------|
| A | Читаемость, комментарии | Медленный парсинг |
| B | Rust-native, типы из коробки | Менее популярен |
| C | Стандарт, валидация (JSON Schema) | Нет комментариев |
| D | Best of both | Сложность тулинга |

**Рекомендация:** Вариант B (RON) — нативная интеграция с Rust, типобезопасность.

**Зависимости:** Решить в начале Фазы 3 (economy, items).

---

### 7. Living World Systems (SR2-inspired)

**Вопрос:** Архитектура фоновой симуляции (фракции, репутация, NPC progression, trade routes)?

**Ключевые системы:**
- **Reputation:** faction-level + personal NPC relationships
- **NPC Progression:** traders → guild masters, rangers → pirate leaders
- **Trade Routes:** dynamic, avoid danger, supply/demand driven
- **Background Sim:** мир живет без игрока (1Hz AISchedule)

**Рекомендация:**
- Фаза 3: Economy + trade routes
- Фаза 4: Faction AI + reputation
- Фаза 6: NPC progression + emergent stories

**Компоненты:**
```rust
Reputation = Component (faction_rep, personal_bonds)
NPC progression = FSM events (Promoted, Betrayed, etc)
Trade routes = A* pathfinding по sector graph
Background = AISchedule (1Hz, always runs)
```

**Риски:** Performance (1000+ NPC), save/load сложность, balance "player impact"

**Зависимости:** Детали в Фазе 3-4.

---

### 8. Crate Modularity Strategy

**Вопрос:** Монолит `voidrun_simulation` или разбить на под-крейты?

**Контекст:**
- Сейчас: один крейт `voidrun_simulation`
- Будущее: physics, combat, economy, ai, quests, netcode — отдельные домены

**Варианты:**
- **A:** Монолит до 10k LOC, потом разбить
- **B:** Сразу разбить на `voidrun_physics`, `voidrun_combat`, etc.
- **C:** Разбить когда домен стабилизируется

**Trade-offs:**
| Вариант | Плюсы | Минусы |
|---------|-------|--------|
| A | Простота, меньше overhead | Сложнее рефакторить позже |
| B | Чистая архитектура, явные границы | Overhead при быстрых итерациях |
| C | Баланс | Нужна дисциплина "когда разбивать" |

**Рекомендация:** Вариант A (монолит) до конца Фазы 2, разбить в Фазе 3 когда combat стабилен.

**Критерий разбивки:** >5k LOC в одном модуле, или явные boundary между доменами.

**Зависимости:** Пересмотреть после Фазы 2.

---

### 9. Netcode Strategy (PvP)

**Вопрос:** Rollback (GGRS, P2P) или Snapshot-interpolation (client-server)?

**Контекст:**
- Фаза 2: rollback для 1v1 PvP
- Будущее: возможно 4v4 arena, или co-op PvE

**Варианты:**
- **A:** Rollback (GGRS) — P2P, no dedicated server
- **B:** Snapshot-interpolation — client-server, масштабируется
- **C:** Гибрид: rollback для малых игр, snapshot для больших

**Trade-offs:**
| Вариант | Плюсы | Минусы |
|---------|-------|--------|
| A | Нет dedicated server, instant | Не масштабируется >4 игроков |
| B | Масштабируется, authoritative server | Нужен сервер, латентность |
| C | Гибкость | Двойная сложность |

**Рекомендация:** Вариант A (GGRS rollback) для Фазы 2 (1v1 PvP), пересмотреть в будущем если нужен >4v4.

**Зависимости:** Решение принято для Фазы 2, пересмотреть если scope изменится.

---

### 10. Modding API Strategy

**Вопрос:** Scripting layer (Lua/WASM) или чистый Rust API?

**Контекст:**
- Моддинг — часть vision документа
- Нужно баланс: безопасность vs гибкость

**Варианты:**
- **A:** Lua через `mlua` (классика, безопасно)
- **B:** WASM через `wasmtime` (производительность)
- **C:** Rust API + hot-reload (максимум гибкости, нужен компилятор)
- **D:** Data-only моддинг (YAML/RON, новые items/quests)

**Trade-offs:**
| Вариант | Плюсы | Минусы |
|---------|-------|--------|
| A | Простота для модеров, безопасность | Медленнее |
| B | Производительность, изоляция | Сложнее для модеров |
| C | Максимум гибкости | Барьер входа (нужен Rust) |
| D | Простейший, безопасный | Ограниченность |

**Рекомендация:** Начать с D (data-only) в Фазе 3, добавить A (Lua) в Фазе 6+ если community запросит.

**Зависимости:** Не критично до Фазы 6+.

---

### 11. Godot Client Architecture

**Вопрос:** Thin client (только render+input) или thick client (UI логика)?

**Контекст:**
- Rust симуляция vs Godot presentation
- Где должна быть UI логика (inventory, menus)?

**Варианты:**
- **A:** Thin client — вся логика в Rust, Godot только рисует
- **B:** Thick client — UI логика в GDScript, Rust только simulation
- **C:** Гибрид — критичная логика в Rust, UI polish в Godot

**Trade-offs:**
| Вариант | Плюсы | Минусы |
|---------|-------|--------|
| A | Детерминизм, легче тестировать | Сложнее UI итерации |
| B | Быстрые UI итерации, знакомый стек | Риск логики в двух местах |
| C | Best of both | Нужна дисциплина границ |

**Рекомендация:** Вариант C — критичная логика (inventory capacity check) в Rust, UI transitions в Godot.

**Зависимости:** Решить в Фазе 7 (full Godot integration).

---

## 🔵 Низкий приоритет (Фаза 8+)

### 12. LOD Strategy для Hitboxes

**Вопрос:** На какой дистанции упрощать hitbox'ы? 50m? 100m?

**Контекст:**
- Близко: multi-part hitbox (голова/туловище/ноги)
- Далеко: single capsule
- Performance vs precision

**Рекомендация:** Отложить до performance профайлинга в Фазе 5+ (когда будет 100+ NPC на сцене).

---

### 13. Procedural Generation Scope

**Вопрос:** Генерировать галактику, планеты, или только контент (quests/items)?

**Контекст:**
- Vision: emergent gameplay из систем
- Но handcrafted контент может быть лучше для narrative

**Рекомендация:** Отложить до Фазы 8 (content expansion). Начать с handcrafted для vertical slice.

---

### 14. Guild/Faction Leadership Mechanics

**Вопрос:** Что происходит когда игрок становится лидером гильдии/фракции? Управление или автономность?

**Контекст:**
- Классическая проблема RPG: "Стал королем, но всё еще побегушкин"
- Игроки хотят чувствовать власть, но НЕ хотят Excel-менеджмент
- Примеры провала: Skyrim (ничего не изменилось), M&B2 (скучные меню), X4 (spreadsheet hell)

**Варианты:**

**A) Autonomous Leadership (рекомендую):**
```
Игрок = strategic commander, guild = автономен в execution

Механики:
- Отдаешь стратегические приказы: "Attack sector X", "Trade with faction Y"
- Guild выполняет АВТОНОМНО (AI системы, как NPC traders)
- Passive income: +% от guild операций
- Emergent consequences: bad decision → members leave → guild weakens

Player experience:
- Чувствуешь власть (приказы работают, видишь рост guild)
- Не тратишь 30 минут в меню
- Фокус на exploration + combat (core gameplay)

Dev time: 1-2 недели
Inspiration: Kenshi (autonomous workers), Dragon Age Inquisition (War Table)
```

**B) Delegated Management (если community хочет больше контроля):**
```
Один экран управления, "set and forget":

Guild Management UI:
├─ Assign 5 Lieutenants (Trade, Military, Diplomacy, Research, Operations)
├─ Set Strategic Goal (Expand / Profit / Reputation)
└─ View Reports (income graph, territory map, event log)

= 5 минут раз в час игры, опционально

Dev time: 3-4 недели
```

**C) Full RTS Mode (НЕ рекомендую):**
```
Strategic map → микроконтроль флотов → RTS battle

Проблемы:
- Scope explosion (+6 месяцев)
- Split audience (strategy vs action players)
- Off-brand (VOIDRUN = FPS/melee, не Civilization)
- 70%+ игроков не используют (waste dev time)
```

**Trade-offs:**

| Вариант | Dev Time | Scope Risk | Player Engagement | Fits Vision | Player Usage |
|---------|----------|------------|-------------------|-------------|--------------|
| A: Autonomous | 1-2 недели | Низкий | Высокая (не отвлекает) | ✅ Да | ~90% |
| B: Delegated | 3-4 недели | Средний | Средняя (опциональна) | 🤔 Может быть | ~40% |
| C: Full RTS | 6+ месяцев | **Критический** | Низкая (split audience) | ❌ Нет | ~20% |

**Рекомендация:**
1. **Фаза 1-4:** Не думать про это (фокус на combat + living world)
2. **Фаза 6:** Добавить Вариант A (autonomous, 1-2 недели)
3. **Фаза 8:** Оценить feedback → расширить до Вариант B если нужно
4. **Никогда:** НЕ делать RTS mode (scope explosion, off-brand)

**Ключевой принцип:**
> Игрок отдает **стратегические приказы**, guild выполняет **автономно** через существующие AI системы. Власть через **consequences**, не через **micromanagement**.

**Примеры из систем:**
```rust
// Player promoted to GuildMaster
PlayerPromoted { guild_id, rank: GuildMaster }

// Что изменилось:
1. Guild AI теперь слушается приказов игрока
2. Passive income: доля от guild операций
3. NPC просят совета (не приказов): "Pirates attack, what do?"
   → Player chooses: Fight / Negotiate / Ignore
   → Guild executes АВТОНОМНО
4. Emergent: bad decisions → loyalty drops → members leave
```

**Зависимости:** Отложить до Фазы 6+ (после Living World доказан в Фазе 4).

---

## 📊 Приоритеты по фазам

| Фаза | Критичные вопросы | Срок решения |
|------|-------------------|--------------|
| Фаза 1 | #1, #2, #5 | Неделя 1-3 |
| Фаза 2 | #3, #4, #6 | До начала Фазы 2 |
| Фаза 3 | #7, #8 | Начало Фазы 3 |
| Фаза 4+ | #9, #10, #11 | По мере необходимости |
| Фаза 6+ | #14 | После Living World |
| Фаза 8+ | #12, #13 | Низкий приоритет |

---

## 🔄 Процесс принятия решений

**Для каждого вопроса:**
1. **Спайк:** быстрый prototype (1-2 часа) если неясно
2. **Обсуждение:** trade-offs анализ (этот документ)
3. **Решение:** выбрать вариант, записать обоснование
4. **ADR:** создать Architecture Decision Record (если критично)
5. **Ревью:** пересмотреть решение если контекст изменился

**Правило:** не парализовать разработку — лучше "достаточно хорошее" решение сейчас, чем "идеальное" через неделю.

---

**Следующий шаг:** Решить вопросы #1, #2, #5 перед началом Фазы 1 (неделя 1).
