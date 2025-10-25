# Procedural Narrative System

**Version:** 1.0
**Created:** 2025-01-23
**Status:** Design Phase

---

## Обзор

Система процедурной генерации нарративного контента для VOIDRUN. Используется в Sandbox Mode и частично в Story Mode.

**Ключевые компоненты:**
1. Procedural Target Generation (Revenge Arc)
2. Procedural Quest System (Radiant quests)
3. Dynamic Event Generation (Emergent storytelling)
4. Betrayal Motive Generation (Narrative flavor)

---

## 1. Procedural Target Generation (Revenge Arc)

### Концепт

В кампании Blood Debt игрок охотится на цепочку targets (3-10 NPC) от исполнителя до faction leader.

**Target chain structure:**
```
Target #1 (Field Agent)
  ↓ betrayed by
Target #2 (Lieutenant)
  ↓ ordered by
Target #3 (Commander)
  ↓ sanctioned by
Target #4 (Inner Circle)
  ↓ approved by
Target #5 (FACTION LEADER) — всегда hand-crafted
```

### Генерация Chain

```rust
struct RevengeTarget {
    rank: FactionRank,
    npc: NpcId,  // Выбран из существующих NPCs фракции
    location: StationId,
    betrayal_context: String,  // Как этот NPC связан с предательством
    security_level: u8,  // 1-5
    intel_requirements: Vec<IntelSource>,
}

fn generate_revenge_chain(
    player_background: Background,
    faction: &Faction,
    config: &RevengeConfig,
) -> Vec<RevengeTarget> {
    let chain_length = config.chain_length;
    let mut targets = Vec::new();

    // Определяем ранги для chain (без leader)
    let ranks = select_ranks_for_chain(chain_length - 1);

    // Генерируем промежуточные targets
    for (index, rank) in ranks.iter().enumerate() {
        // Выбираем NPC из фракции по рангу
        let npc = faction.select_npc_by_rank(*rank)
            .expect("Faction должна иметь NPCs всех рангов");

        // Генерируем контекст предательства
        let betrayal_context = generate_betrayal_context(
            player_background,
            config.betrayal_severity,
            *rank,
            index,
        );

        // Определяем требования intel
        let intel_requirements = generate_intel_requirements(
            *rank,
            config.intel_difficulty,
        );

        targets.push(RevengeTarget {
            rank: *rank,
            npc: npc.id,
            location: npc.home_station,
            betrayal_context,
            security_level: rank_to_security_level(*rank),
            intel_requirements,
        });
    }

    // Добавляем faction leader как финальную цель (hand-crafted NPC)
    targets.push(create_leader_target(faction.leader_id, player_background));

    targets
}

fn select_ranks_for_chain(count: u8) -> Vec<FactionRank> {
    use FactionRank::*;

    match count {
        2 => vec![FieldAgent, Lieutenant],
        3 => vec![FieldAgent, Lieutenant, Commander],
        4 => vec![FieldAgent, Lieutenant, Commander, InnerCircle],
        5 => vec![FieldAgent, FieldAgent, Lieutenant, Commander, InnerCircle],
        // ... и т.д. до 9 промежуточных targets
        _ => panic!("Invalid chain length"),
    }
}
```

### Betrayal Context Generation

**Система генерирует мотив предательства на основе:**
- Player background (кто ты был)
- Betrayal severity (насколько личное)
- Target rank (уровень ответственности)
- Position in chain (первый узнал или orchestrated)

```rust
fn generate_betrayal_context(
    background: Background,
    severity: BetrayalType,
    rank: FactionRank,
    chain_position: usize,
) -> String {
    use Background::*;
    use BetrayalType::*;
    use FactionRank::*;

    // Базовая матрица мотивов
    let base_motive = match (background, severity) {
        (MilitaryVeteran, Personal) => "unit_massacre",
        (MilitaryVeteran, Professional) => "court_martial_setup",
        (MilitaryVeteran, Ideological) => "war_crime_cover_up",

        (ExCriminal, Personal) => "family_killed",
        (ExCriminal, Professional) => "framed_for_crime",
        (ExCriminal, Ideological) => "gang_betrayed_code",

        (Scientist, Personal) => "research_stolen_colleague_killed",
        (Scientist, Professional) => "patent_theft",
        (Scientist, Ideological) => "research_weaponized",

        // ... остальные backgrounds
        _ => "generic_betrayal",
    };

    // Адаптация к рангу
    let role_in_betrayal = match (rank, chain_position) {
        (FieldAgent, 0) => "executed_the_order",
        (Lieutenant, _) => "coordinated_the_operation",
        (Commander, _) => "authorized_the_mission",
        (InnerCircle, _) => "orchestrated_from_shadows",
        _ => "was_involved_in",
    };

    // Генерация финального текста
    format_betrayal_text(base_motive, role_in_betrayal, rank)
}

fn format_betrayal_text(
    motive: &str,
    role: &str,
    rank: FactionRank,
) -> String {
    match motive {
        "unit_massacre" => format!(
            "{} {} the ambush that killed your squad. \
            They left you for dead.",
            rank.title(),
            role
        ),

        "family_killed" => format!(
            "{} {} the hit on your family. \
            They made it look like a rival gang.",
            rank.title(),
            role
        ),

        // ... остальные варианты
        _ => format!("{} {} your betrayal.", rank.title(), role),
    }
}
```

### Intel Requirements

**Что нужно узнать перед убийством target:**

```rust
enum IntelSource {
    Location,        // Где находится target
    Schedule,        // Когда он vulnerable
    Weakness,        // Security слабости
    Motivation,      // Почему он участвовал
    NextTarget,      // Зацепка на следующего в chain
}

fn generate_intel_requirements(
    rank: FactionRank,
    difficulty: IntelDifficulty,
) -> Vec<IntelSource> {
    use IntelSource::*;

    let base_requirements = match rank {
        FactionRank::FieldAgent => vec![Location],
        FactionRank::Lieutenant => vec![Location, Schedule],
        FactionRank::Commander => vec![Location, Schedule, Weakness],
        FactionRank::InnerCircle => vec![Location, Schedule, Weakness, Motivation],
    };

    // Усложняем на основе difficulty
    let additional = match difficulty {
        IntelDifficulty::Easy => vec![],
        IntelDifficulty::Normal => vec![NextTarget],
        IntelDifficulty::Hard => vec![NextTarget, Motivation],
        IntelDifficulty::Extreme => vec![NextTarget, Motivation, Schedule],
    };

    [base_requirements, additional].concat()
}
```

### Intel Acquisition Methods

**Как получить intel:**

1. **Interrogation** — допросить предыдущего target перед убийством
2. **Data hack** — взломать терминалы фракции
3. **Witness** — найти свидетеля (NPC кто знает)
4. **Documents** — найти physical evidence
5. **Tracker** — подсадить tracking device

```rust
struct IntelAcquisition {
    source_type: IntelSource,
    methods: Vec<AcquisitionMethod>,
    current_progress: f32, // 0.0-1.0
}

enum AcquisitionMethod {
    Interrogate(NpcId),
    HackTerminal(StationId),
    FindWitness { location: ChunkCoord, hint: String },
    StealDocuments(StationId),
    PlantTracker { on_npc: NpcId },
}
```

---

## 2. Procedural Quest System (Radiant)

### Концепт

Бесконечные процедурно генерируемые квесты для endgame freeplay.

**Quest Templates:**
- Escort (защитить convoy)
- Eliminate (убить target NPC)
- Investigate (explore location, find evidence)
- Trade (deliver goods)
- Rescue (save NPC from pirates/etc.)
- Defend (hold station against attack)

### Quest Generation

```rust
struct ProceduralQuest {
    template: QuestTemplate,
    parameters: QuestParameters,
    rewards: Rewards,
    time_limit: Option<Duration>,
    faction: Option<FactionId>,
}

fn generate_quest(world: &World, player: &Player) -> ProceduralQuest {
    // Выбираем template на основе world state
    let template = select_template_for_world_state(world);

    // Генерируем параметры
    let parameters = match template {
        QuestTemplate::Escort => generate_escort_params(world, player),
        QuestTemplate::Eliminate => generate_eliminate_params(world, player),
        QuestTemplate::Investigate => generate_investigate_params(world, player),
        // ... остальные
    };

    // Генерируем награды
    let rewards = calculate_rewards(&template, &parameters, player.level);

    ProceduralQuest {
        template,
        parameters,
        rewards,
        time_limit: template.default_time_limit(),
        faction: parameters.faction,
    }
}
```

### Template-Specific Generation

**Escort Quest:**
```rust
struct EscortParameters {
    convoy_size: u8,  // 1-5 ships
    start_station: StationId,
    end_station: StationId,
    route_danger: f32,  // 0.0-1.0
    cargo_value: u32,
    expected_threats: Vec<ThreatType>, // Pirates, Rival faction, etc.
}

fn generate_escort_params(world: &World, player: &Player) -> EscortParameters {
    // Выбираем start/end stations
    let start = world.select_random_station_near_player(player);
    let end = world.select_station_at_distance(start, 3..8); // 3-8 chunks away

    // Определяем опасность маршрута
    let route = world.pathfinding.find_path(start, end);
    let route_danger = calculate_route_danger(&route, world);

    // Генерируем convoy на основе опасности
    let convoy_size = if route_danger > 0.7 { 5 } else { 1 + (route_danger * 4.0) as u8 };

    EscortParameters {
        convoy_size,
        start_station: start,
        end_station: end,
        route_danger,
        cargo_value: (route_danger * 10000.0) as u32,
        expected_threats: generate_threats_for_route(&route, world),
    }
}
```

**Eliminate Quest:**
```rust
struct EliminateParameters {
    target_npc: NpcId,
    target_location: StationId,
    difficulty: f32,  // 0.0-1.0
    reason: EliminationReason,
    time_limit: Option<Duration>,
}

enum EliminationReason {
    Bounty,        // Criminal с наградой
    Assassination, // Заказ от фракции
    Revenge,       // Personal (NPC кто-то обидел)
    Threat,        // NPC представляет угрозу
}

fn generate_eliminate_params(world: &World, player: &Player) -> EliminateParameters {
    // Выбираем target NPC
    let target = world.select_random_hostile_npc(player);

    // Определяем причину
    let reason = if target.has_bounty() {
        EliminationReason::Bounty
    } else if world.factions.any_hostile_to_player() {
        EliminationReason::Assassination
    } else {
        EliminationReason::Threat
    };

    // Сложность = уровень target
    let difficulty = (target.level as f32) / (player.level as f32);

    EliminateParameters {
        target_npc: target.id,
        target_location: target.current_location,
        difficulty,
        reason,
        time_limit: if reason == EliminationReason::Bounty {
            Some(Duration::from_days(7))
        } else {
            None
        },
    }
}
```

### World State-Based Selection

**Квесты генерируются на основе состояния мира:**

```rust
fn select_template_for_world_state(world: &World) -> QuestTemplate {
    match world.current_state() {
        WorldState::PostWar => {
            // После войны → rebuilding quests
            weighted_choice([
                (QuestTemplate::Escort, 0.4),    // Supply runs
                (QuestTemplate::Defend, 0.3),    // Protect stations
                (QuestTemplate::Trade, 0.3),     // Economic recovery
            ])
        }

        WorldState::CivilWar => {
            // Гражданская война → combat quests
            weighted_choice([
                (QuestTemplate::Eliminate, 0.4),
                (QuestTemplate::Defend, 0.3),
                (QuestTemplate::Investigate, 0.3), // Intel missions
            ])
        }

        WorldState::Stable => {
            // Мирное время → diverse quests
            weighted_choice([
                (QuestTemplate::Trade, 0.3),
                (QuestTemplate::Investigate, 0.2),
                (QuestTemplate::Rescue, 0.2),
                (QuestTemplate::Escort, 0.2),
                (QuestTemplate::Eliminate, 0.1), // Bounties
            ])
        }
    }
}
```

---

## 3. Dynamic Event Generation

### Концепт

Процедурные события которые происходят в мире независимо от игрока.

**Event Types:**
- Pirate raid (атака пиратов на станцию)
- Faction war escalation (фракция захватывает территорию)
- Economic crisis (станция нуждается в supplies)
- Refugee crisis (беженцы прибывают на станцию)
- Anomaly discovered (странная локация найдена)

### Event Generation

```rust
struct DynamicEvent {
    event_type: EventType,
    location: ChunkCoord,
    participants: Vec<NpcId>,
    duration: Duration,
    player_can_intervene: bool,
}

fn generate_dynamic_event(world: &World, frequency: f32) -> Option<DynamicEvent> {
    // Roll для генерации события
    if !should_generate_event(frequency) {
        return None;
    }

    // Выбираем тип на основе world state
    let event_type = select_event_type(world);

    // Генерируем параметры
    match event_type {
        EventType::PirateRaid => generate_pirate_raid(world),
        EventType::FactionWarEscalation => generate_war_event(world),
        EventType::EconomicCrisis => generate_crisis(world),
        // ... остальные
    }
}

fn generate_pirate_raid(world: &World) -> Option<DynamicEvent> {
    // Выбираем цель (станцию)
    let target_station = world.select_vulnerable_station();

    // Генерируем пиратов
    let pirate_count = rand::range(3..10);
    let pirates = world.spawn_pirate_npcs(pirate_count, near: target_station);

    Some(DynamicEvent {
        event_type: EventType::PirateRaid,
        location: target_station.chunk,
        participants: pirates,
        duration: Duration::from_hours(2),
        player_can_intervene: true,
    })
}
```

### Player Intervention

**Игрок может вмешаться в события:**

```rust
enum InterventionOption {
    DefendStation {
        reward: Rewards,
        reputation_gain: i32,
    },

    JoinPirates {
        loot_share: f32,
        reputation_loss: i32,
    },

    Ignore,
}

fn get_intervention_options(event: &DynamicEvent, player: &Player) -> Vec<InterventionOption> {
    match &event.event_type {
        EventType::PirateRaid => vec![
            InterventionOption::DefendStation {
                reward: calculate_defense_reward(event),
                reputation_gain: 10,
            },
            InterventionOption::JoinPirates {
                loot_share: 0.3,
                reputation_loss: -20,
            },
            InterventionOption::Ignore,
        ],

        // ... остальные event types
    }
}
```

---

## 4. Betrayal Motive Matrix (Reference)

**Полная матрица для генерации мотивов предательства.**

### Military Veteran

| Betrayal Type | Field Agent | Lieutenant | Commander | Inner Circle |
|---------------|-------------|------------|-----------|--------------|
| **Personal** | Executed ambush order | Coordinated the trap | Authorized the betrayal | Orchestrated for promotion |
| **Professional** | Falsified evidence | Covered up war crime | Court-martialed you | Political purge |
| **Ideological** | Followed illegal order | Suppressed dissent | Betrayed allies for power | Sold out to enemy |

### Ex-Criminal

| Betrayal Type | Field Agent | Lieutenant | Commander | Inner Circle |
|---------------|-------------|------------|-----------|--------------|
| **Personal** | Killed your family | Ordered the hit | Approved elimination | Blood feud resolution |
| **Professional** | Framed you | Set up the job | Took credit for heist | Sold you to cops |
| **Ideological** | Broke gang code | Betrayed crew | Sold out syndicate | Went corporate |

### Scientist

| Betrayal Type | Field Agent | Lieutenant | Commander | Inner Circle |
|---------------|-------------|------------|-----------|--------------|
| **Personal** | Sabotaged experiment | Killed colleague | Stole research, murdered team | Destroyed your life's work |
| **Professional** | Falsified data | Stole patent | Fired you, took credit | Corporate espionage |
| **Ideological** | Weaponized research | Sold to military | Used for unethical ends | Created WMD from your work |

*Аналогично для Diplomat, Mercenary, Nobody.*

---

## 5. Implementation Priorities

### Phase 1: Foundation

1. **RevengeTarget struct** — базовая структура
2. **Target chain generation** — простая версия (fixed 5 targets)
3. **Betrayal context system** — базовая матрица мотивов

### Phase 2: Procedural Quests

4. **Quest templates** — 3 базовых (Escort, Eliminate, Trade)
5. **Quest generation** — простая параметрическая генерация
6. **World state awareness** — квесты адаптируются к миру

### Phase 3: Dynamic Events

7. **Event types** — 2-3 базовых события (Pirate Raid, Economic Crisis)
8. **Event generation** — frequency-based spawning
9. **Player intervention** — интеракция с событиями

### Phase 4: Polish

10. **Advanced betrayal context** — все 6 backgrounds × 3 types
11. **Intel system** — acquisition methods, UI
12. **Complex events** — faction wars, cascading crises

---

## Связанные документы

- [Campaign & Sandbox System](campaign-sandbox-system.md) — общая архитектура
- [Endgame Systems](endgame-systems.md) — где используются эти системы
- [Bevy ECS Design](../architecture/bevy-ecs-design.md) — как интегрировать в ECS

**Версия:** 1.0
**Обновлено:** 2025-01-23
