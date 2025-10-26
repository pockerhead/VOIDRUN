//! Tests for FSM AI components.

#[cfg(test)]
mod tests {
    use super::super::fsm::{AIState, AIConfig};

    #[test]
    fn test_ai_state_default() {
        let state = AIState::default();
        assert!(matches!(state, AIState::Idle));
    }

    #[test]
    fn test_ai_config_default() {
        let config = AIConfig::default();
        assert_eq!(config.retreat_stamina_threshold, 0.3);
        assert_eq!(config.retreat_health_threshold, 0.2);
        assert_eq!(config.retreat_duration, 2.0);
        assert_eq!(config.patrol_direction_change_interval, 10.0);
    }

    #[test]
    fn test_retreat_timer_logic() {
        let mut timer = 2.0;
        let delta = 0.5;

        timer -= delta;
        assert_eq!(timer, 1.5);

        timer -= delta;
        assert_eq!(timer, 1.0);

        timer -= delta;
        assert_eq!(timer, 0.5);

        timer -= delta;
        assert_eq!(timer, 0.0);
        assert!(timer <= 0.0); // Retreat завершен
    }
}
