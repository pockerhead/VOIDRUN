# Campaign & Sandbox System Design

**Version:** 1.0
**Created:** 2025-01-23
**Status:** Design Phase

---

## Обзор

VOIDRUN предлагает два режима игры:

1. **Story Mode** — кураторские кампании с предопределёнными сидами
2. **Sandbox Mode** — полная кастомизация мира и кампании

**Философия:** Sandbox-first development. Curated campaigns = пресеты поверх гибкой системы конфигурации.

---

## Два режима

### Story Mode (Curated Experience)

**Структура:**
```
Campaign Selection → Background Selection → START
         ↓
  (seed предопределён)
```

**Features:**
- 3 предопределённых мира (seeds)
- Balanced difficulty (протестировано)
- Ключевые NPCs на своих местах
- Consistent experience (игроки обсуждают одинаковый контент)

### Sandbox Mode (Player-Configured)

**Структура:**
```
World Seed Input → Campaign Type → Campaign Config → World Config → Background → START
```

**Features:**
- Custom/random seeds
- Deep configuration (26+ параметров)
- Freeplay option (no campaign)
- Seed sharing (community feature)

---

## Три кампании

### 1. The Last Hope (Galactic Threat)

**Эмоциональный тон:** Героическая надежда, эпичность, ответственность
**Длительность:** 30-40 часов
**Predefined seed:** "Invasion" (seed: 42)

**Структура:**
- **Act 1:** Discovery — доказательства угрозы
- **Act 2:** Unity — объединение фракций
- **Act 3:** War — финальная битва

**Seed features:**
- Alien threat активна на границе
- Wartime economy (оружие дорогое)
- Military factions готовы к войне

**Endgame:** Post-war galaxy
- United Galaxy (diplomatic victory) → rebuilding, politics
- Fractured Galaxy (pyrrhic victory) → cold war, chaos

**Уникальные механики:**
- Faction unity system (дипломатия)
- Threat timer (опционально)
- Mass battles

---

### 2. Blood Debt (Revenge Arc)

**Эмоциональный тон:** Мрачная ярость, катарсис
**Длительность:** 25-35 часов
**Predefined seed:** "Betrayal" (seed: 1337)

**Структура:**
- **Act 1:** First Blood — Target #1 убит, зацепка на #2
- **Act 2:** The Chain — Targets #2-4 (escalation)
- **Act 3:** Regicide — Faction Leader убит

**Seed features:**
- MegaCorp контролирует 40% территорий
- Underworld процветает (black market)
- Political corruption

**Target chain (процедурная генерация):**
```
Target #1 (Field Agent) → #2 (Lieutenant) → #3 (Commander) → #4 (Inner Circle) → #5 (Leader)
```

**Critical choice после убийства лидера:**
- **Take Control** → Стать главой фракции (management sandbox)
- **Walk Away** → Фракция в civil war (explorer sandbox)
- **Disband Faction** → Активно разрушить (hardcore chaos mode)

**Endgame (Take Control):**
- Faction management (territory, economy, military)
- Internal rebellions (loyalists vs new regime)
- Expansion wars

**Endgame (Walk Away):**
- True freedom (no obligations)
- Legendary status (NPCs recognize you)
- Mercenary work, exploration

**Уникальные механики:**
- Procedural target generation
- Investigation system (intel gathering)
- Faction takeover mechanics

---

### 3. Final Dawn (Death Timer)

**Эмоциональный тон:** Трагический фатализм, legacy building
**Длительность:** 10-15 часов (FIXED by timer)
**Predefined seed:** "Twilight" (seed: 2077)

**Концепт:** Ты смертельно ранен. 60-90 дней до смерти. Выбери как хочешь чтобы тебя запомнили.

**Seed features:**
- Post-crisis galaxy (фракции ослаблены)
- Depression economy
- Memento mori atmosphere

**Death timer mechanic:**
```rust
- Days remaining: 60-90 (configurable)
- HP degradation: -10% max HP per week
- Critical state: Last 10 days (ограничена мобильность, hallucinations)
- Permadeath: Timer expires = game over
```

**Legacy win conditions (выбираешь в начале):**
1. **Wealth** — Накопи 1M credits, передай наследнику
2. **Sacrifice** — Героический акт (спаси станцию/планету)
3. **Knowledge** — Найди ancient artifact, передай учёным
4. **Blood** — Убей финальную цель (express revenge)
5. **Fade Away** — Sandbox (no goal, просто живи)

**Endgame:** НЕТ (это feature!)
- Epilogue cutscene (impact твоего legacy)
- Memorial в других кампаниях (easter egg)
- Unlock cosmetics для других кампаний

**Уникальные механики:**
- Real-time death countdown
- Health degradation system
- Time extension options (expensive medical treatment)
- Final days events (goodbyes, closure)

---

## Система предысторий (универсальная)

**6 backgrounds для всех кампаний:**

| Background | Stats | Reputation | Perk |
|------------|-------|------------|------|
| **Military Veteran** | +2 combat, +1 leadership | Neutral military, -1 pirates | "Tactical Mind" (+10% damage when outnumbered) |
| **Ex-Criminal** | +2 stealth, +1 hacking | +1 underworld, -2 corps | "Street Smart" (better black market prices) |
| **Scientist** | +2 tech, +1 perception | +1 research factions | "Analytical" (faster crafting) |
| **Diplomat** | +2 charisma, +1 intelligence | +1 all major factions | "Silver Tongue" (unique dialogues) |
| **Mercenary** | +1 combat, +1 piloting, +500 credits | True neutral | "Mercenary Code" (+10% rewards) |
| **Nobody** | Balanced | True neutral | "Underdog" (+15% XP) |

**Starting situations различаются по кампаниям:**
- Last Hope: 100% HP, standard gear, faction contacts
- Blood Debt: 50-70% HP, damaged gear, alone
- Final Dawn: 100% HP, timer active, decent gear

См. детали: [Background Starting Situations](#background-starting-situations)

---

## Sandbox Configuration (Deep Mode)

**26+ параметров** для максимальной кастомизации.

### A. World Generation (6 параметров)

1. **Seed value** (u64) — RNG seed
2. **Galaxy size** — Tiny (50 systems) / Small (100) / Medium (200) / Large (500) / Huge (1000)
3. **World density** (0.0-1.0) — станции на систему
4. **Resource abundance** (0.0-2.0) — loot, minerals
5. **Anomaly frequency** (0.0-1.0) — странные события
6. **Starting system type** — Core (safe) / Border / Frontier (dangerous) / Random

### B. Faction & Politics (5 параметров)

7. **Faction count** (3-10) — количество major factions
8. **Power balance** — Balanced / Hegemony / Bipolar / Fractured / Anarchy
9. **Diplomacy state** — Peace / Cold War / Hot War / Chaos
10. **AI aggression** (0.0-1.0) — насколько активно фракции воюют
11. **Ideology mix** — Corporate / Military / Scientific / Pirates / Mixed

### C. Economy (4 параметра)

12. **Economy state** — Booming / Stable / Recession / Depression
13. **Price volatility** (0.0-1.0) — динамика цен
14. **Black market strength** (0.0-1.0) — доступность контрабанды
15. **Starting credits** (0.1-5.0x) — множитель стартового капитала

### D. Danger & Conflict (4 параметра)

16. **Piracy level** (0.0-1.0) — частота пиратских атак
17. **NPC aggression** (0.0-1.0) — как быстро NPCs атакуют
18. **Random event frequency** (0.0-2.0) — dynamic events
19. **Hazard level** (0.0-1.0) — radiation, asteroids, etc.

### E. Technology & Progression (3 параметра)

20. **Tech level** — Primitive / Standard / Advanced / Mixed
21. **Gear progression speed** (0.5-2.0x) — unlock rate
22. **Research availability** (0.0-1.0) — сколько tech доступно

### F. Game Rules (3+ параметра)

23. **Permadeath** (bool)
24. **Ironman mode** (bool) — no manual saves
25. **Injury severity** (0.5-2.0x) — опасность ранений
26. **Time scale** (0.5-2.0x) — скорость in-game time

### Campaign-Specific Configs

**Galactic Threat (7 параметров):**
1. Threat type (Alien / AI / Horror / Civil War)
2. Threat strength (0.0-1.0)
3. Progression speed (0.5-2.0x)
4. Time limit (days, optional)
5. Allow extensions (bool)
6. Faction unity (Very Low → High)
7. Victory condition (Eliminate / Contain / Survive)

**Revenge Arc (8 параметров):**
1. Target faction (specific or random)
2. Chain length (3-10 targets)
3. Betrayal severity (Personal / Professional / Ideological)
4. Starting resources (Desperate / Survivor / Prepared)
5. Intel difficulty (Easy / Normal / Hard / Extreme)
6. Target protection (0.5-2.0x)
7. Endgame options (какие пути доступны)
8. Takeover requirements (для faction leadership)

**Final Dawn (9 параметров):**
1. Days until death (15-180)
2. Degradation speed (5-35% HP/week)
3. Death cause (cosmetic: Radiation / Disease / Injury / etc.)
4. Allow extensions (bool)
5. Extension methods (Medical / Cybernetics / Drugs)
6. Max extensions (0-5)
7. Available legacies (какие win conditions)
8. Legacy difficulty (0.5-2.0x)
9. Post-death mode (Permadeath / Checkpoint / Spectator)

**Полные детали:** См. [docs/design/sandbox-configuration.md](sandbox-configuration.md) (TODO)

---

## Seed Sharing System

**Core feature для community.**

### Encoding Format

**Compact Code:**
```
Format: VR-[SEED]-[CAMPAIGN]-[HASH]
Example: VR-42-GT60-A7F3

- VR = VoidRun identifier
- 42 = seed value
- GT60 = Galactic Threat, 60% difficulty
- A7F3 = config checksum
```

**Full Config (JSON):**
```json
{
  "version": "1.0",
  "seed": 42,
  "campaign": { "type": "GalacticThreat", "config": {...} },
  "world": {...},
  "checksum": "a7f3..."
}
```

### UI для Sharing

**Import:**
```
World Seed:
○ Generate Random
○ Enter Seed: [________]
○ Import Code: [VR-42-GT60-A7F3]
○ Import File: [Browse...]

Quick Presets:
[Balanced] [Hardcore] [Sandbox] [Community]
```

**Export:**
```
Your seed code: VR-1337-RA5-B4D3

[Copy to Clipboard]
[Export Full Config]
[Share on Workshop] (optional)
```

### Community Features (опционально)

- Leaderboards (по кампаниям)
- Workshop integration (Steam/etc.)
- Challenge seeds (community-curated)

---

## Endgame Systems

**Все кампании (кроме Final Dawn) имеют endgame freeplay.**

### Emergent Systems (всегда активны)

1. **Faction Wars** — фракции автономно воюют за территории
2. **Economic Simulation** — supply/demand, trade routes
3. **Reputation Propagation** — твои действия влияют на репутацию
4. **Dynamic Events** — pirate raids, sieges, crises

### Procedural Quests (radiant system)

Генерируются на основе world state:
- Post-war rebuilding (Last Hope)
- Faction war participation (Blood Debt - Walk Away)
- Territory defense (Blood Debt - Take Control)

Шаблоны: Escort, Eliminate, Investigate, Trade, Rescue

### Hand-Crafted Questlines (5-10 per campaign)

**Last Hope endgame:**
- "The Aftermath" — политические последствия
- "War Crimes Tribunal" — судить преступников?
- "New Threat" — небольшая угроза

**Blood Debt endgame (Take Control):**
- "The Usurper" — внутренний coup attempt
- "Old Debts" — наследие старого лидера
- "Alliance or War" — forge peace или crush rivals

**Blood Debt endgame (Walk Away):**
- "Ghost of the Past" — кто-то ищет тебя
- "The Aftermath" — consequences твоей мести
- "New Purpose" — find meaning

### Faction Management (Blood Debt - Take Control)

```rust
struct FactionLeadership {
    territory: Vec<Station>,
    income: CreditsPerDay,
    lieutenants: Vec<NPC>,
    reputation: FactionReputation,
    internal_stability: f32, // 0.0-1.0
}
```

**Mechanics:**
- Appoint/dismiss lieutenants
- Set economic policies (taxes, trade routes)
- Declare wars/alliances
- Handle internal rebellions
- Expand territory

**Win conditions (optional):**
- Stabilize faction (stability > 0.7 для 30 дней)
- Become dominant (control 40%+ galaxy)
- Create legacy (achieve faction-specific goals)

**Детали:** См. [docs/design/endgame-systems.md](endgame-systems.md)

---

## Technical Architecture

### Campaign State Machine

```rust
enum CampaignState {
    LastHope { act: Act, threat_level: u8 },
    BloodDebt { target_chain: Vec<RevengeTarget>, current: usize },
    FinalDawn { legacy: LegacyGoal, days_remaining: u8 },
    Endgame { mode: EndgameMode },
    Freeplay,
}

enum EndgameMode {
    PostWar,        // Last Hope
    FactionLeader,  // Blood Debt - Take Control
    Wanderer,       // Blood Debt - Walk Away
    None,           // Final Dawn (no endgame)
}
```

### World Configuration Struct

```rust
struct WorldSeed {
    seed_value: u64,
    campaign_config: Option<CampaignConfig>,
    world_config: WorldConfig,
}

struct WorldConfig {
    // 26 параметров (см. выше)
    faction_balance: FactionBalance,
    economy: EconomyDifficulty,
    piracy_level: f32,
    // ... etc
}
```

### Procedural Target Generation (Revenge Arc)

```rust
fn generate_revenge_chain(
    player_background: Background,
    faction: Faction,
    chain_length: u8,
) -> Vec<RevengeTarget> {
    let mut targets = vec![];

    for rank in [FieldAgent, Lieutenant, Commander, InnerCircle] {
        let npc = faction.select_npc_by_rank(rank);
        targets.push(RevengeTarget {
            rank,
            location: npc.home_station,
            betrayal_context: generate_motive(player_background, rank),
            security_level: rank as u8,
            intel_requirements: generate_intel(rank),
        });
    }

    targets.push(faction.leader); // Final target (hand-crafted)
    targets
}
```

**Детали:** См. [docs/design/procedural-narrative.md](procedural-narrative.md)

---

## Development Roadmap (Sandbox-First)

### Phase 1: Core Sandbox Systems (Foundation)

1. **World Generation** — seed-based chunk/faction/economy generation
2. **Configuration System** — WorldConfig, CampaignConfig structs + UI
3. **Freeplay Mode** — no campaign, pure sandbox

### Phase 2: Campaign Systems (Mechanics)

4. **Campaign Framework** — state machine, objective tracking
5. **Procedural Systems** — target generation, quest templates, events
6. **Endgame Systems** — faction management, post-campaign freeplay

### Phase 3: Content & Polish (Story Mode)

7. **Curated Campaigns** — 3 predefined seeds, hand-crafted NPCs/quests
8. **Community Features** — seed sharing UI, workshop integration

**Детали:** См. обновлённый [docs/roadmap.md](../roadmap.md)

---

## Background Starting Situations

**Как предыстории различаются по кампаниям:**

### Military Veteran

**Last Hope:**
- Локация: Military base (border)
- Состояние: 100% HP, standard gear, contacts
- Narrative: Призван обратно для борьбы с угрозой
- Quest: "Report to Command"

**Blood Debt:**
- Локация: Abandoned outpost
- Состояние: 50% HP, damaged gear, alone
- Narrative: Отряд предан и уничтожен
- Quest: "Find the Traitor"

**Final Dawn:**
- Локация: Medical station
- Состояние: 100% HP, timer active
- Narrative: Radiation exposure в последней миссии
- Quest: "Choose Your Legacy"

*Аналогично для остальных 5 backgrounds.*

---

## UI/UX Flow

### Main Menu

```
╔════════════════════════════════╗
║       VOIDRUN                  ║
╠════════════════════════════════╣
║  [STORY MODE]                  ║
║  [SANDBOX MODE]                ║
║  [CONTINUE]                    ║
║  [SETTINGS]                    ║
║  [QUIT]                        ║
╚════════════════════════════════╝
```

### Story Mode Flow

```
Campaign Selection → Background Selection → START
        ↓
 (seed auto-selected)
```

### Sandbox Mode Flow

```
Seed Input → Campaign Type → Config → Background → START
```

---

## Примеры конфигураций

### "Impossible Revenge" (hardcore challenge)

```
Seed: VR-666-RA7-PER-CHA-DEP
- Revenge Arc, 7 targets
- Desperate start (50% HP, minimal gear)
- Chaotic factions (no alliances)
- Depression economy
- Hard intel difficulty
- Permadeath enabled
```

### "God Mode Sandbox" (relaxed exploration)

```
Seed: VR-9999-FP-EST-BAL-BOOM
- Freeplay (no campaign)
- Established start (100k credits, good rep)
- Balanced factions
- Booming economy
- Low piracy
```

### "Quick Death Run" (speedrun challenge)

```
Seed: VR-2077-FD30-RAP-RAD
- Final Dawn
- 30 days до смерти
- Rapid degradation (25% HP/week)
- No extensions
- Permadeath forced
```

---

## Следующие шаги

1. **Implement WorldConfig system** — structs, serialization, UI
2. **Procedural generation** — seed-based world/faction/NPC generation
3. **Campaign framework** — state machines, objective tracking
4. **Seed encoding** — compact format + JSON export/import

См. обновлённый roadmap для детального плана.

---

## Связанные документы

- [Procedural Narrative System](procedural-narrative.md) — генерация targets/quests/events
- [Endgame Systems](endgame-systems.md) — faction management, emergent gameplay
- [Sandbox Configuration Reference](sandbox-configuration.md) — полный список параметров (TODO)
- [Roadmap](../roadmap.md) — development phases

**Версия:** 1.0
**Обновлено:** 2025-01-23
