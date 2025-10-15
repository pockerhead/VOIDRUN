//! DEPRECATED: AI combat decision system (Godot tactical layer).
//!
//! **This file has been REPLACED by `ai_combat_decision.rs`**
//!
//! # Why deprecated?
//!
//! **Problem:** Race condition between separate attack/parry decision systems:
//! - `ai_melee_attack_intent` (in melee.rs) generated attacks
//! - `ai_react_to_incoming_attacks_main_thread` (this file) generated parries
//! → Result: Actor could start attack windup THEN decide to parry (conflicting states)
//!
//! **Solution:** Unified decision system in `ai_combat_decision.rs`:
//! - Single system evaluates ALL actions (attack, parry, wait)
//! - Chooses best action by priority
//! - Cancels conflicting actions (e.g. interrupt windup to parry)
//!
//! # Migration
//!
//! All functionality moved to `ai_combat_decision_main_thread()` system.
//! Helper functions (`is_facing_attacker`, facing/distance validation) are now inline.
//!
//! This file kept as documentation of the old architecture.
//! Can be deleted after confirming new system works.
