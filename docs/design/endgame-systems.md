# Endgame Systems Design

**Version:** 1.0
**Created:** 2025-01-23
**Status:** Design Phase

---

## Обзор

Endgame systems обеспечивают бесконечный контент после завершения main story campaign.

**Применимость:**
- ✅ The Last Hope (Post-War Galaxy)
- ✅ Blood Debt (Faction Leader / Wanderer paths)
- ❌ Final Dawn (no endgame — это feature)

**Ключевые компоненты:**
1. Emergent Gameplay Systems
2. Faction Management (Blood Debt - Take Control)
3. Procedural Quest System
4. Hand-Crafted Post-Game Questlines
5. Sandbox Tools

---

## 1. Emergent Gameplay Systems

**Философия:** Мир живёт сам, игрок — участник, не центр вселенной.

### A. Faction War Simulation

**Концепт:** Фракции автономно воюют за территории.

```rust
struct FactionWarState {
    active_wars: Vec<War>,
    faction_strengths: HashMap<FactionId, f32>,
    territory_control: HashMap<StationId, FactionId>,
}

struct War {
    attacker: FactionId,
    defender: FactionId,
    contested_stations: Vec<StationId>,
    war_exhaustion: f32,  // 0.0-1.0
    start_date: GameTime,
}

impl FactionWarState {
    fn daily_update(&mut self, world: &mut World) {
        for war in &mut self.active_wars {
            // Симулируем битвы
            for station in &war.contested_stations {
                self.simulate_battle(station, war, world);
            }

            // Проверяем условия окончания войны
            war.war_exhaustion += 0.01; // Растёт со временем
            if war.war_exhaustion > 0.8 || self.check_victory_conditions(war) {
                self.end_war(war, world);
            }
        }

        // Генерируем новые войны
        if rand::random::<f32>() < 0.05 { // 5% chance daily
            self.try_start_new_war(world);
        }
    }

    fn simulate_battle(&mut self, station: &StationId, war: &mut War, world: &World) {
        let attacker_strength = self.faction_strengths[&war.attacker];
        let defender_strength = self.faction_strengths[&war.defender];

        // Простая симуляция: сравниваем силы
        let battle_outcome = if attacker_strength > defender_strength * 1.2 {
            BattleOutcome::AttackerWins
        } else if defender_strength > attacker_strength * 1.2 {
            BattleOutcome::DefenderWins
        } else {
            BattleOutcome::Stalemate
        };

        // Применяем результат
        match battle_outcome {
            BattleOutcome::AttackerWins => {
                self.territory_control.insert(*station, war.attacker);
                war.war_exhaustion += 0.1; // Победа снижает exhaustion
            }
            BattleOutcome::DefenderWins => {
                war.war_exhaustion += 0.15; // Поражение увеличивает
            }
            BattleOutcome::Stalemate => {
                war.war_exhaustion += 0.05; // Затяжной конфликт
            }
        }
    }
}
```

**Player Intervention:**

Игрок может вмешаться в войну:
- **Join attacker** — помочь захватить станцию (missions, rewards)
- **Join defender** — защитить (higher reputation gain)
- **Sabotage both** — chaos agent (mercenary playstyle)

```rust
enum WarIntervention {
    JoinAttacker { faction: FactionId, station: StationId },
    JoinDefender { faction: FactionId, station: StationId },
    Sabotage { target: FactionId },
}

fn player_intervenes(intervention: WarIntervention, player: &mut Player, world: &mut World) {
    match intervention {
        WarIntervention::JoinAttacker { faction, station } => {
            // Trigger assault mission
            let mission = create_assault_mission(station, faction);
            if player.complete_mission(mission) {
                world.faction_wars.capture_station(station, faction);
                player.reputation.modify(faction, +20);
            }
        }

        // ... остальные варианты
    }
}
```

---

### B. Economic Simulation

**Концепт:** Цены зависят от supply/demand, торговые пути динамические.

```rust
struct EconomicState {
    station_inventories: HashMap<StationId, Inventory>,
    trade_routes: Vec<TradeRoute>,
    price_multipliers: HashMap<(StationId, ItemId), f32>,
}

struct TradeRoute {
    from: StationId,
    to: StationId,
    goods: Vec<ItemId>,
    frequency: Duration,  // Как часто NPCs торгуют
    active: bool,
}

impl EconomicState {
    fn daily_update(&mut self, world: &World) {
        // Обновляем supply/demand
        for (station_id, inventory) in &mut self.station_inventories {
            self.update_demand(station_id, inventory, world);
        }

        // NPCs торгуют по маршрутам
        for route in &self.trade_routes {
            if route.active && should_trade_today(route.frequency) {
                self.execute_npc_trade(route);
            }
        }

        // Пересчитываем цены
        self.recalculate_prices();
    }

    fn update_demand(
        &mut self,
        station: &StationId,
        inventory: &mut Inventory,
        world: &World,
    ) {
        let population = world.stations[station].population;

        // Потребление ресурсов населением
        inventory.consume(ItemId::Food, population / 100);
        inventory.consume(ItemId::Water, population / 50);

        // Производство (если есть facilities)
        if world.stations[station].has_facility(FacilityType::Farm) {
            inventory.produce(ItemId::Food, 100);
        }
    }

    fn recalculate_prices(&mut self) {
        for (station_id, inventory) in &self.station_inventories {
            for item in inventory.items() {
                let supply = inventory.quantity(item);
                let demand = calculate_demand(station_id, item);

                // Supply/demand price multiplier
                let multiplier = if supply > demand {
                    0.5 + (demand as f32 / supply as f32) * 0.5 // Низкие цены
                } else {
                    1.0 + ((supply as f32 / demand as f32) - 1.0) * 2.0 // Высокие цены
                };

                self.price_multipliers.insert((*station_id, item), multiplier);
            }
        }
    }
}
```

**Player Impact:**

Действия игрока влияют на экономику:
- **Trade routes disruption** — убил NPC-трейдеров → дефицит товаров
- **Station destruction** — уничтожил производство → цены растут
- **Faction wars** — блокировка маршрутов → дефляция/инфляция

---

### C. Reputation Propagation

**Концепт:** Твои действия распространяются через NPCs (gossip system).

```rust
struct ReputationSystem {
    faction_rep: HashMap<FactionId, i32>,  // -100 to +100
    personal_rep: HashMap<NpcId, i32>,
    legendary_status: LegendaryStatus,
}

enum LegendaryStatus {
    Unknown,
    Known,            // NPCs знают твоё имя
    Famous,           // Твои деяния обсуждаются
    Legendary,        // Unlock unique encounters
    Infamous,         // Feared/hated
}

impl ReputationSystem {
    fn propagate_action(&mut self, action: PlayerAction, witnesses: &[NpcId], world: &World) {
        // Свидетели распространяют информацию
        for witness in witnesses {
            let gossip_reach = calculate_gossip_reach(witness, world);

            for npc in gossip_reach {
                self.modify_npc_opinion(npc, action);
            }
        }

        // Обновляем legendary status
        self.update_legendary_status(action);
    }

    fn update_legendary_status(&mut self, action: PlayerAction) {
        use PlayerAction::*;

        match action {
            KilledFactionLeader(_) => {
                self.legendary_status = LegendaryStatus::Famous;
            }

            SavedStation { population } if population > 1000 => {
                self.legendary_status = LegendaryStatus::Legendary;
            }

            MassacredCivilians { count } if count > 50 => {
                self.legendary_status = LegendaryStatus::Infamous;
            }

            _ => {}
        }
    }
}

fn calculate_gossip_reach(npc: &NpcId, world: &World) -> Vec<NpcId> {
    // NPCs рассказывают друзьям/коллегам
    let npc_data = &world.npcs[npc];

    let mut reach = vec![];

    // Друзья
    reach.extend(&npc_data.friends);

    // Коллеги (та же станция)
    reach.extend(world.npcs_at_station(npc_data.location));

    // Faction members (если NPC в фракции)
    if let Some(faction) = npc_data.faction {
        reach.extend(world.factions[&faction].members.iter().take(10)); // Sample
    }

    reach
}
```

**Legendary Status Effects:**

- **Famous** → Unique dialogues, NPCs recognize you
- **Legendary** → Special quests unlock, vendors offer rare items
- **Infamous** → Bounty hunters, assassination attempts

---

## 2. Faction Management (Blood Debt - Take Control)

**Применяется только если игрок стал главой фракции.**

### Core Structure

```rust
struct FactionLeadership {
    faction_id: FactionId,
    controlled_stations: Vec<StationId>,
    lieutenants: HashMap<StationId, NpcId>,  // По одному на станцию
    daily_income: i32,
    stability: f32,  // 0.0-1.0
    expansion_targets: Vec<StationId>,
    internal_threats: Vec<InternalThreat>,
}

enum InternalThreat {
    Rebellion {
        leader: NpcId,
        supporters: Vec<NpcId>,
        grievance: String,
    },

    Corruption {
        lieutenant: NpcId,
        embezzled_funds: u32,
    },

    Coup {
        conspirators: Vec<NpcId>,
        progress: f32,  // 0.0-1.0 до execution
    },
}
```

### Daily Management Loop

```rust
impl FactionLeadership {
    fn daily_update(&mut self, world: &mut World, player: &Player) {
        // 1. Собираем доход
        self.collect_income(world);

        // 2. Проверяем стабильность
        self.update_stability(world);

        // 3. Генерируем угрозы
        if self.stability < 0.5 {
            self.generate_internal_threat();
        }

        // 4. Обрабатываем внешние угрозы (войны)
        self.handle_external_threats(world);

        // 5. Lieutenants делают decisions (автономно)
        for (station, lieutenant) in &self.lieutenants {
            self.lieutenant_daily_action(station, lieutenant, world);
        }
    }

    fn collect_income(&mut self, world: &World) {
        self.daily_income = 0;

        for station in &self.controlled_stations {
            let station_data = &world.stations[station];

            // Доход = население × tax rate × economic state
            let base_income = station_data.population * 10;
            let tax_multiplier = self.tax_rate;
            let economic_multiplier = world.economy.get_multiplier(station);

            let income = (base_income as f32 * tax_multiplier * economic_multiplier) as i32;

            self.daily_income += income;
        }
    }

    fn update_stability(&mut self, world: &World) {
        // Факторы стабильности
        let mut stability_modifiers = 0.0;

        // Положительные факторы
        if self.daily_income > 10000 {
            stability_modifiers += 0.01; // Процветание
        }

        if self.lieutenants.len() == self.controlled_stations.len() {
            stability_modifiers += 0.01; // Все станции управляемы
        }

        // Негативные факторы
        for threat in &self.internal_threats {
            stability_modifiers -= 0.02; // Внутренние угрозы
        }

        if world.faction_wars.faction_at_war(&self.faction_id) {
            stability_modifiers -= 0.01; // Война истощает
        }

        // Применяем
        self.stability = (self.stability + stability_modifiers).clamp(0.0, 1.0);
    }

    fn generate_internal_threat(&mut self) {
        use InternalThreat::*;

        // Выбираем тип угрозы
        let threat = match rand::range(0..3) {
            0 => self.generate_rebellion(),
            1 => self.generate_corruption(),
            2 => self.generate_coup(),
            _ => unreachable!(),
        };

        self.internal_threats.push(threat);
    }

    fn generate_rebellion(&self) -> InternalThreat {
        // Кто-то из lieutenants недоволен
        let disloyal_lieutenant = self.find_disloyal_lieutenant();

        InternalThreat::Rebellion {
            leader: disloyal_lieutenant.id,
            supporters: disloyal_lieutenant.gather_supporters(),
            grievance: generate_grievance(disloyal_lieutenant),
        }
    }
}
```

### Player Actions (Management UI)

```rust
enum ManagementAction {
    // Lieutenants
    AppointLieutenant { station: StationId, npc: NpcId },
    DismissLieutenant { station: StationId },

    // Economy
    SetTaxRate { rate: f32 },  // 0.0-1.0
    InvestInInfrastructure { station: StationId, amount: u32 },

    // Military
    DeclareWar { target_faction: FactionId },
    ProposeAlliance { target_faction: FactionId },
    LaunchAssault { target_station: StationId },

    // Internal
    SuppressRebellion { threat_id: ThreatId },
    NegotiateWithRebels { threat_id: ThreatId },
    InvestigateCorruption { lieutenant: NpcId },
}

impl Player {
    fn execute_management_action(&mut self, action: ManagementAction, leadership: &mut FactionLeadership) {
        match action {
            ManagementAction::AppointLieutenant { station, npc } => {
                leadership.lieutenants.insert(station, npc);
                leadership.stability += 0.05; // Управление улучшено
            }

            ManagementAction::SetTaxRate { rate } => {
                leadership.tax_rate = rate;

                // Высокие налоги → больше дохода, меньше стабильность
                if rate > 0.7 {
                    leadership.stability -= 0.1;
                }
            }

            ManagementAction::SuppressRebellion { threat_id } => {
                // Триггерит combat mission
                let rebellion = leadership.find_threat(threat_id);
                self.start_suppression_mission(rebellion);
            }

            // ... остальные действия
        }
    }
}
```

### Win Conditions (Faction Management)

**Optional endgame goals:**

1. **Stability Master** — Maintain stability > 0.7 for 30 days
2. **Galactic Dominance** — Control 40%+ of galaxy territories
3. **Economic Empire** — Achieve 100k daily income
4. **Peaceful Unification** — Form alliances with all major factions

---

## 3. Procedural Quest System

**См. детали в [procedural-narrative.md](procedural-narrative.md).**

**Краткое описание:**

Radiant quests генерируются бесконечно на основе:
- World state (война, мир, кризис)
- Player reputation
- Faction relationships

**Templates:**
- Escort, Eliminate, Investigate, Trade, Rescue, Defend

**Rewards scale с player level.**

---

## 4. Hand-Crafted Post-Game Questlines

**5-10 questlines на кампанию** для narrative closure после main story.

### The Last Hope Post-Game Quests

**1. "The Aftermath" (3-5 missions)**
- Тема: Политические последствия победы
- Trigger: 7 days после победы
- Content: Фракции спорят кто заслужил какие территории
- Player choice: Support one faction OR mediate neutral solution
- Outcome: Shapes post-war galaxy

**2. "War Crimes Tribunal" (2-3 missions)**
- Тема: Судить врагов или показать милосердие?
- Trigger: Захват вражеских leaders
- Content: Interrogations, evidence gathering, trial
- Player choice: Execute, imprison, exile, или pardon
- Outcome: Affects reputation с фракциями

**3. "New Threat" (4-6 missions)**
- Тема: Небольшая угроза (не endgame-scale)
- Trigger: Random (30+ days после победы)
- Content: Pirate warlord OR rogue AI OR remnants of old threat
- Outcome: Combat challenge для endgame builds

---

### Blood Debt Post-Game Quests (Take Control)

**1. "The Usurper" (5-7 missions)**
- Тема: Внутренний coup attempt
- Trigger: Stability < 0.4
- Content: Identify conspirators, gather evidence, confront traitor
- Player choice: Execute, exile, OR recruit (high-risk/high-reward)
- Outcome: Major stability swing

**2. "Old Debts" (3-5 missions)**
- Тема: Наследие убитого leader haunts you
- Trigger: Random (15+ days после takeover)
- Content: Hidden caches, secret alliances, blackmail
- Outcome: Unlock secrets about old regime

**3. "Alliance or War" (branching questline)**
- Тема: Forge peace с rival faction OR crush them
- Trigger: Player chooses (diplomatic или military path)
- Content:
  - Peace path: Negotiations, compromises, mutual enemies
  - War path: Strategic strikes, resource denial, final assault
- Outcome: Galaxy map changes dramatically

---

### Blood Debt Post-Game Quests (Walk Away)

**1. "Ghost of the Past" (3-4 missions)**
- Тема: Кто-то из старой фракции ищет тебя
- Trigger: Random (10+ days после ухода)
- Content: Old ally needs help OR old enemy seeks revenge
- Outcome: Closure на relationships

**2. "The Aftermath" (2-3 missions)**
- Тема: Consequences твоей мести
- Trigger: Return to faction territory
- Content: Witness civil war, refugees, chaos you caused
- Player choice: Help rebuild OR walk away (again)
- Outcome: Morality moment

**3. "New Purpose" (open-ended)**
- Тема: Find meaning после revenge
- Trigger: Player-initiated (visit specific location)
- Content: Multiple mini-quests (exploration, helping NPCs, building legacy)
- Outcome: Personal closure

---

## 5. Sandbox Tools

**Опционально: творческие инструменты для endgame.**

### A. Base Building

```rust
struct PlayerBase {
    location: ChunkCoord,
    modules: Vec<BaseModule>,
    crew: Vec<NpcId>,
    storage: Inventory,
}

enum BaseModule {
    LivingQuarters { capacity: u8 },
    Workshop { crafting_speed: f32 },
    Hangar { ship_capacity: u8 },
    Farm { food_production: u32 },
    DefenseTurret { damage: f32 },
}
```

**Почему:** Endgame sink для credits, personalization, home base для crew.

---

### B. Ship Customization

```rust
struct Ship {
    hull: HullType,
    weapons: Vec<WeaponSlot>,
    engines: EngineType,
    shields: ShieldType,
    cosmetics: CosmeticLoadout,
}
```

**Почему:** Endgame progression через ship upgrades вместо character levels.

---

### C. Crew System

```rust
struct Crew {
    members: Vec<NpcId>,
    roles: HashMap<NpcId, CrewRole>,
    morale: f32,
}

enum CrewRole {
    Pilot,
    Engineer,
    Gunner,
    Medic,
    Trader,
}
```

**Почему:** RPG companions для narrative depth, tactical advantages.

---

## Implementation Priorities

### Phase 1: Foundation (Emergent Systems)

1. **Faction war simulation** — базовая версия (2 фракции воюют)
2. **Economic simulation** — supply/demand, цены
3. **Reputation propagation** — gossip system

**Estimated effort:** 2-3 недели

---

### Phase 2: Faction Management

4. **Leadership struct** — доход, stability, lieutenants
5. **Internal threats** — rebellion, corruption, coup
6. **Management UI** — назначение lieutenants, policies

**Estimated effort:** 3-4 недели

---

### Phase 3: Content

7. **Procedural quests** — 3 templates (Escort, Eliminate, Trade)
8. **Hand-crafted questlines** — 2-3 per campaign (total 6-9 quests)
9. **Polish** — balancing, testing, bug fixes

**Estimated effort:** 4-6 недель

---

### Phase 4: Sandbox Tools (Optional)

10. **Base building** — если время есть
11. **Ship customization** — advanced version
12. **Crew system** — если нужно для narrative

**Estimated effort:** 2-4 недели (low priority)

---

## Testing Strategy

### Emergent Systems Testing

- **Faction wars:** Симулировать 100 days, проверить balance
- **Economy:** Crash test (destroy production → цены растут?)
- **Reputation:** Verify gossip spreads correctly

### Faction Management Testing

- **Stability:** Тестировать edge cases (0% stability, 100% stability)
- **Rebellions:** Verify threats генерируются при низкой стабильности
- **Income:** Balance check (не слишком легко стать богатым?)

### Quest Testing

- **Generation:** 100 quests генерируются без ошибок?
- **Rewards:** Scaling правильный для player level?
- **Completion:** No softlocks

---

## Связанные документы

- [Campaign & Sandbox System](campaign-sandbox-system.md) — общая архитектура
- [Procedural Narrative](procedural-narrative.md) — quest generation details
- [Roadmap](../roadmap.md) — development schedule

**Версия:** 1.0
**Обновлено:** 2025-01-23
